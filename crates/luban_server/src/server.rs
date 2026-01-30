use crate::auth;
use crate::engine::{Engine, EngineHandle, new_default_services};
use crate::idempotency::{Begin, IdempotencyStore};
use crate::mentions;
use crate::pty::PtyManager;
use anyhow::Context as _;
use axum::middleware;
use axum::{
    Json, Router,
    extract::{Multipart, Path, Query, State, ws::WebSocketUpgrade},
    response::IntoResponse,
    routing::{delete, get, post},
};
use luban_api::AppSnapshot;
use luban_api::{
    CodexCustomPromptSnapshot, PROTOCOL_VERSION, WorkspaceChangesSnapshot, WorkspaceDiffSnapshot,
    WsClientMessage, WsServerMessage,
};
use luban_domain::paths;
use luban_domain::{ContextImage, ProjectWorkspaceService};
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};
use tokio::sync::broadcast;
use tower_http::services::{ServeDir, ServeFile};

pub async fn router(config: crate::ServerConfig) -> anyhow::Result<Router> {
    let services = new_default_services()?;
    let (engine, events) = Engine::start(services.clone());

    let state = AppStateHolder {
        engine,
        events,
        pty: PtyManager::new(),
        services,
        auth: auth::AuthState::new(config.auth),
        idempotency_attachments: IdempotencyStore::new(
            std::time::Duration::from_secs(10 * 60),
            256,
        ),
    };

    let api_public = Router::new().route("/health", get(health));

    let api_protected = Router::new()
        .route("/app", get(get_app))
        .route("/codex/prompts", get(get_codex_prompts))
        .route("/workspaces/{workspace_id}/threads", get(get_threads))
        .route(
            "/workspaces/{workspace_id}/conversations/{thread_id}",
            get(get_conversation),
        )
        .route(
            "/workspaces/{workspace_id}/attachments",
            post(upload_attachment),
        )
        .route(
            "/workspaces/{workspace_id}/attachments/{attachment_id}",
            get(download_attachment),
        )
        .route("/workspaces/{workspace_id}/changes", get(get_changes))
        .route("/workspaces/{workspace_id}/diff", get(get_diff))
        .route("/workspaces/{workspace_id}/context", get(get_context))
        .route(
            "/workspaces/{workspace_id}/mentions",
            get(get_workspace_mentions),
        )
        .route(
            "/workspaces/{workspace_id}/context/{context_id}",
            delete(delete_context_item),
        )
        .route("/events", get(ws_events))
        .route("/pty/{workspace_id}/{thread_id}", get(ws_pty))
        .route_layer(middleware::from_fn_with_state(
            state.clone(),
            auth::require_session,
        ));

    let api = api_public.merge(api_protected);

    let web_dist = std::env::var_os("LUBAN_WEB_DIST_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("web/out"));
    let web_index = web_dist.join("index.html");
    let web = ServeDir::new(web_dist).not_found_service(ServeFile::new(web_index));

    Ok(Router::new()
        .merge(auth::router())
        .nest("/api", api)
        .fallback_service(web)
        .with_state(state))
}

async fn health() -> &'static str {
    "ok"
}

fn resolve_codex_root() -> anyhow::Result<PathBuf> {
    if let Some(root) = std::env::var_os(paths::LUBAN_CODEX_ROOT_ENV) {
        let root = root.to_string_lossy();
        let trimmed = root.trim();
        if trimmed.is_empty() {
            anyhow::bail!("{} is set but empty", paths::LUBAN_CODEX_ROOT_ENV);
        }
        return Ok(PathBuf::from(trimmed));
    }

    let home = std::env::var_os("HOME").ok_or_else(|| anyhow::anyhow!("HOME is not set"))?;
    Ok(PathBuf::from(home).join(".codex"))
}

fn resolve_luban_root() -> anyhow::Result<PathBuf> {
    if let Some(root) = std::env::var_os(luban_domain::paths::LUBAN_ROOT_ENV) {
        let root = root.to_string_lossy();
        let trimmed = root.trim();
        if trimmed.is_empty() {
            anyhow::bail!("{} is set but empty", luban_domain::paths::LUBAN_ROOT_ENV);
        }
        return Ok(PathBuf::from(trimmed));
    }

    let home = std::env::var_os("HOME").ok_or_else(|| anyhow::anyhow!("HOME is not set"))?;
    Ok(PathBuf::from(home).join("luban"))
}

#[derive(Clone)]
pub(crate) struct AppStateHolder {
    engine: EngineHandle,
    events: broadcast::Sender<WsServerMessage>,
    pty: PtyManager,
    services: std::sync::Arc<dyn ProjectWorkspaceService>,
    pub(crate) auth: auth::AuthState,
    idempotency_attachments: IdempotencyStore<luban_api::AttachmentRef>,
}

async fn get_app(State(state): State<AppStateHolder>) -> impl IntoResponse {
    match state.engine.app_snapshot().await {
        Ok(snapshot) => Json(snapshot).into_response(),
        Err(err) => (
            axum::http::StatusCode::INTERNAL_SERVER_ERROR,
            err.to_string(),
        )
            .into_response(),
    }
}

fn prompt_description(contents: &str) -> String {
    for line in contents.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        let trimmed = trimmed.trim_start_matches('#').trim();
        if trimmed.is_empty() {
            continue;
        }
        return trimmed.chars().take(160).collect();
    }
    String::new()
}

fn codex_prompt_id(prompts_dir: &std::path::Path, path: &std::path::Path) -> String {
    let rel = path.strip_prefix(prompts_dir).unwrap_or(path);
    let rel = rel.to_string_lossy().replace('\\', "/");
    let rel = rel.strip_prefix('/').unwrap_or(&rel);
    if let Some(stripped) = rel.strip_suffix(".md") {
        stripped.to_owned()
    } else if let Some(stripped) = rel.strip_suffix(".txt") {
        stripped.to_owned()
    } else {
        rel.to_owned()
    }
}

fn load_codex_prompts() -> anyhow::Result<Vec<CodexCustomPromptSnapshot>> {
    let root = resolve_codex_root()?;
    let prompts_dir = root.join("prompts");
    if !prompts_dir.exists() {
        return Ok(Vec::new());
    }

    let mut out = Vec::new();
    let mut stack = vec![prompts_dir.clone()];
    while let Some(dir) = stack.pop() {
        let entries =
            std::fs::read_dir(&dir).with_context(|| format!("failed to read {}", dir.display()))?;
        for entry in entries {
            let entry =
                entry.with_context(|| format!("failed to read entry under {}", dir.display()))?;
            let path = entry.path();
            let ty = entry
                .file_type()
                .with_context(|| format!("failed to stat {}", path.display()))?;
            if ty.is_dir() {
                stack.push(path);
                continue;
            }
            if !ty.is_file() {
                continue;
            }
            let file_name = entry.file_name().to_string_lossy().to_string();
            if file_name == ".DS_Store" {
                continue;
            }

            let contents = std::fs::read_to_string(&path)
                .with_context(|| format!("failed to read {}", path.display()))?;
            let id = codex_prompt_id(&prompts_dir, &path);
            let label = id.clone();
            let description = prompt_description(&contents);
            out.push(CodexCustomPromptSnapshot {
                id,
                label,
                description,
                contents,
            });
        }
    }

    out.sort_by(|a, b| a.label.cmp(&b.label));
    Ok(out)
}

async fn get_codex_prompts() -> impl IntoResponse {
    let result = tokio::task::spawn_blocking(load_codex_prompts).await;
    match result {
        Ok(Ok(prompts)) => Json(prompts).into_response(),
        Ok(Err(err)) => (
            axum::http::StatusCode::INTERNAL_SERVER_ERROR,
            err.to_string(),
        )
            .into_response(),
        Err(err) => (
            axum::http::StatusCode::INTERNAL_SERVER_ERROR,
            format!("failed to join prompt task: {err}"),
        )
            .into_response(),
    }
}

async fn get_threads(
    State(state): State<AppStateHolder>,
    Path(workspace_id): Path<u64>,
) -> impl IntoResponse {
    match state
        .engine
        .threads_snapshot(luban_api::WorkspaceId(workspace_id))
        .await
    {
        Ok(snapshot) => Json(snapshot).into_response(),
        Err(err) => (axum::http::StatusCode::NOT_FOUND, err.to_string()).into_response(),
    }
}

#[derive(serde::Deserialize)]
struct MentionQuery {
    q: String,
}

async fn get_workspace_mentions(
    State(state): State<AppStateHolder>,
    Path(workspace_id): Path<u64>,
    Query(query): Query<MentionQuery>,
) -> impl IntoResponse {
    let worktree_path = match state
        .engine
        .workspace_worktree_path(luban_api::WorkspaceId(workspace_id))
        .await
    {
        Ok(Some(path)) => path,
        Ok(None) => {
            return (axum::http::StatusCode::NOT_FOUND, "workspace not found").into_response();
        }
        Err(err) => {
            return (axum::http::StatusCode::NOT_FOUND, err.to_string()).into_response();
        }
    };

    let q = query.q;
    let result = tokio::task::spawn_blocking(move || {
        mentions::search_workspace_mentions(&worktree_path, &q)
    })
    .await;
    match result {
        Ok(Ok(items)) => Json(items).into_response(),
        Ok(Err(err)) => (
            axum::http::StatusCode::INTERNAL_SERVER_ERROR,
            err.to_string(),
        )
            .into_response(),
        Err(err) => (
            axum::http::StatusCode::INTERNAL_SERVER_ERROR,
            format!("failed to join mention task: {err}"),
        )
            .into_response(),
    }
}

async fn get_conversation(
    State(state): State<AppStateHolder>,
    Path((workspace_id, thread_id)): Path<(u64, u64)>,
    Query(query): Query<ConversationQuery>,
) -> impl IntoResponse {
    match state
        .engine
        .conversation_snapshot(
            luban_api::WorkspaceId(workspace_id),
            luban_api::WorkspaceThreadId(thread_id),
            query.before,
            query.limit,
        )
        .await
    {
        Ok(snapshot) => Json(snapshot).into_response(),
        Err(err) => (axum::http::StatusCode::NOT_FOUND, err.to_string()).into_response(),
    }
}

#[derive(serde::Deserialize)]
struct ConversationQuery {
    before: Option<u64>,
    limit: Option<u64>,
}

async fn ws_events(ws: WebSocketUpgrade, State(state): State<AppStateHolder>) -> impl IntoResponse {
    ws.on_upgrade(move |socket| ws_events_task(socket, state))
}

async fn ws_events_task(mut socket: axum::extract::ws::WebSocket, state: AppStateHolder) {
    let mut rx = state.events.subscribe();
    let engine = state.engine.clone();

    let current_rev = engine.current_rev().await.unwrap_or(0);
    let _ = socket
        .send(json_text(&WsServerMessage::Hello {
            protocol_version: PROTOCOL_VERSION,
            current_rev,
        }))
        .await;

    loop {
        tokio::select! {
            incoming = socket.recv() => {
                let Some(Ok(msg)) = incoming else { break };
                if handle_ws_incoming(msg, &engine, &mut socket).await.is_err() {
                    break;
                }
            }
            outgoing = rx.recv() => {
                match outgoing {
                    Ok(outgoing) => {
                        if socket.send(json_text(&outgoing)).await.is_err() {
                            break;
                        }
                    }
                    Err(broadcast::error::RecvError::Lagged(_)) => {
                        if send_app_snapshot_if_needed(&engine, None, &mut socket).await.is_err() {
                            break;
                        }
                    }
                    Err(broadcast::error::RecvError::Closed) => break,
                }
            }
        }
    }
}

fn json_text<T: serde::Serialize>(value: &T) -> axum::extract::ws::Message {
    axum::extract::ws::Message::Text(serde_json::to_string(value).unwrap_or_default().into())
}

async fn handle_ws_incoming(
    msg: axum::extract::ws::Message,
    engine: &EngineHandle,
    socket: &mut axum::extract::ws::WebSocket,
) -> anyhow::Result<()> {
    let axum::extract::ws::Message::Text(text) = msg else {
        return Ok(());
    };

    let client: WsClientMessage = match serde_json::from_str(&text) {
        Ok(v) => v,
        Err(err) => {
            let _ = socket
                .send(json_text(&WsServerMessage::Error {
                    request_id: None,
                    message: format!("invalid ws message: {err}"),
                }))
                .await;
            return Ok(());
        }
    };

    match client {
        WsClientMessage::Hello { last_seen_rev, .. } => {
            send_app_snapshot_if_needed(engine, last_seen_rev, socket).await?;
            Ok(())
        }
        WsClientMessage::Ping => {
            socket.send(json_text(&WsServerMessage::Pong)).await?;
            Ok(())
        }
        WsClientMessage::Action { request_id, action } => {
            let ack = engine
                .apply_client_action(request_id.clone(), *action)
                .await;
            let msg = match ack {
                Ok(rev) => WsServerMessage::Ack { request_id, rev },
                Err(message) => WsServerMessage::Error {
                    request_id: Some(request_id),
                    message,
                },
            };
            socket.send(json_text(&msg)).await?;
            Ok(())
        }
    }
}

async fn send_app_snapshot_if_needed(
    engine: &EngineHandle,
    last_seen_rev: Option<u64>,
    socket: &mut axum::extract::ws::WebSocket,
) -> anyhow::Result<()> {
    let current_rev = engine.current_rev().await.unwrap_or(0);
    if last_seen_rev == Some(current_rev) {
        return Ok(());
    }

    let snapshot = engine.app_snapshot().await?;
    let msg = WsServerMessage::Event {
        rev: current_rev,
        event: Box::new(luban_api::ServerEvent::AppChanged {
            rev: current_rev,
            snapshot: Box::new(snapshot),
        }),
    };
    socket.send(json_text(&msg)).await?;
    Ok(())
}

async fn ws_pty(
    ws: WebSocketUpgrade,
    State(state): State<AppStateHolder>,
    Path((workspace_id, thread_id)): Path<(u64, u64)>,
    Query(query): Query<PtyQuery>,
) -> impl IntoResponse {
    ws.on_upgrade(move |socket| ws_pty_task(socket, state, workspace_id, thread_id, query))
}

#[derive(serde::Deserialize, Clone)]
struct PtyQuery {
    reconnect: Option<String>,
}

async fn ws_pty_task(
    socket: axum::extract::ws::WebSocket,
    state: AppStateHolder,
    workspace_id: u64,
    thread_id: u64,
    query: PtyQuery,
) {
    let cwd = match state
        .engine
        .workspace_worktree_path(luban_api::WorkspaceId(workspace_id))
        .await
    {
        Ok(Some(path)) => path,
        _ => std::env::current_dir().unwrap_or_default(),
    };

    let reconnect = query
        .reconnect
        .as_deref()
        .map(str::trim)
        .filter(|v| !v.is_empty())
        .map(str::to_owned)
        .unwrap_or_else(|| format!("thread-{thread_id}"));

    let session = match state.pty.get_or_create(workspace_id, reconnect, cwd) {
        Ok(session) => session,
        Err(err) => {
            tracing::error!(error = %err, "failed to create pty session");
            return;
        }
    };

    crate::pty::pty_ws_task(socket, session).await;
}

#[derive(serde::Deserialize)]
struct AttachmentQuery {
    ext: String,
}

fn is_safe_file_extension(ext: &str) -> bool {
    let ext = ext.trim();
    if ext.is_empty() || ext.len() > 16 {
        return false;
    }
    ext.chars().all(|c| c.is_ascii_alphanumeric())
}

fn is_safe_attachment_id(id: &str) -> bool {
    let id = id.trim();
    if id.is_empty() || id.len() > 128 {
        return false;
    }
    id.chars()
        .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
}

async fn download_attachment(
    State(state): State<AppStateHolder>,
    Path((workspace_id, attachment_id)): Path<(u64, String)>,
    Query(query): Query<AttachmentQuery>,
) -> impl IntoResponse {
    if !is_safe_file_extension(&query.ext) {
        return (
            axum::http::StatusCode::BAD_REQUEST,
            "invalid attachment extension",
        )
            .into_response();
    }
    if !is_safe_attachment_id(&attachment_id) {
        return (axum::http::StatusCode::BAD_REQUEST, "invalid attachment id").into_response();
    }

    let Some((project_slug, workspace_name)) =
        workspace_scope_from_snapshot(&state.engine.app_snapshot().await.ok(), workspace_id)
    else {
        return (axum::http::StatusCode::NOT_FOUND, "workspace not found").into_response();
    };

    let luban_root = match resolve_luban_root() {
        Ok(root) => root,
        Err(err) => {
            return (
                axum::http::StatusCode::INTERNAL_SERVER_ERROR,
                err.to_string(),
            )
                .into_response();
        }
    };
    let conversations_root = luban_domain::paths::conversations_root(&luban_root);
    let blob_path = conversations_root
        .join(project_slug)
        .join(workspace_name)
        .join("context")
        .join("blobs")
        .join(format!("{}.{}", attachment_id, query.ext));

    let bytes = match tokio::fs::read(&blob_path).await {
        Ok(bytes) => bytes,
        Err(_) => {
            return (axum::http::StatusCode::NOT_FOUND, "attachment not found").into_response();
        }
    };

    let content_type = match query.ext.as_str() {
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "webp" => "image/webp",
        "txt" | "md" => "text/plain; charset=utf-8",
        _ => "application/octet-stream",
    };

    ([(axum::http::header::CONTENT_TYPE, content_type)], bytes).into_response()
}

async fn get_changes(
    State(state): State<AppStateHolder>,
    Path(workspace_id): Path<u64>,
) -> impl IntoResponse {
    let Some((_project_slug, _workspace_name, worktree_path)) =
        workspace_info_from_snapshot(&state.engine.app_snapshot().await.ok(), workspace_id)
    else {
        return (axum::http::StatusCode::NOT_FOUND, "workspace not found").into_response();
    };

    let repo_path = PathBuf::from(worktree_path);
    let result =
        tokio::task::spawn_blocking(move || crate::git_changes::collect_changes(&repo_path)).await;

    match result {
        Ok(Ok(files)) => Json(WorkspaceChangesSnapshot {
            workspace_id: luban_api::WorkspaceId(workspace_id),
            files,
        })
        .into_response(),
        Ok(Err(err)) => (
            axum::http::StatusCode::INTERNAL_SERVER_ERROR,
            err.to_string(),
        )
            .into_response(),
        Err(err) => (
            axum::http::StatusCode::INTERNAL_SERVER_ERROR,
            format!("failed to run git: {err}"),
        )
            .into_response(),
    }
}

async fn get_diff(
    State(state): State<AppStateHolder>,
    Path(workspace_id): Path<u64>,
) -> impl IntoResponse {
    let Some((_project_slug, _workspace_name, worktree_path)) =
        workspace_info_from_snapshot(&state.engine.app_snapshot().await.ok(), workspace_id)
    else {
        return (axum::http::StatusCode::NOT_FOUND, "workspace not found").into_response();
    };

    let repo_path = PathBuf::from(worktree_path);
    let result =
        tokio::task::spawn_blocking(move || crate::git_changes::collect_diff(&repo_path)).await;

    match result {
        Ok(Ok(files)) => Json(WorkspaceDiffSnapshot {
            workspace_id: luban_api::WorkspaceId(workspace_id),
            files,
        })
        .into_response(),
        Ok(Err(err)) => (
            axum::http::StatusCode::INTERNAL_SERVER_ERROR,
            err.to_string(),
        )
            .into_response(),
        Err(err) => (
            axum::http::StatusCode::INTERNAL_SERVER_ERROR,
            format!("failed to run git: {err}"),
        )
            .into_response(),
    }
}

async fn get_context(
    State(state): State<AppStateHolder>,
    Path(workspace_id): Path<u64>,
) -> impl IntoResponse {
    let Some((project_slug, workspace_name)) =
        workspace_scope_from_snapshot(&state.engine.app_snapshot().await.ok(), workspace_id)
    else {
        return (axum::http::StatusCode::NOT_FOUND, "workspace not found").into_response();
    };

    match state
        .services
        .list_context_items(project_slug, workspace_name)
    {
        Ok(items) => {
            let api_items = items
                .into_iter()
                .map(|item| luban_api::ContextItemSnapshot {
                    context_id: item.id,
                    attachment: luban_api::AttachmentRef {
                        id: item.attachment.id,
                        kind: match item.attachment.kind {
                            luban_domain::AttachmentKind::Image => luban_api::AttachmentKind::Image,
                            luban_domain::AttachmentKind::Text => luban_api::AttachmentKind::Text,
                            luban_domain::AttachmentKind::File => luban_api::AttachmentKind::File,
                        },
                        name: item.attachment.name,
                        extension: item.attachment.extension,
                        mime: item.attachment.mime,
                        byte_len: item.attachment.byte_len,
                    },
                    created_at_unix_ms: item.created_at_unix_ms,
                })
                .collect();

            Json(luban_api::ContextSnapshot {
                workspace_id: luban_api::WorkspaceId(workspace_id),
                items: api_items,
            })
            .into_response()
        }
        Err(message) => (axum::http::StatusCode::INTERNAL_SERVER_ERROR, message).into_response(),
    }
}

async fn delete_context_item(
    State(state): State<AppStateHolder>,
    Path((workspace_id, context_id)): Path<(u64, u64)>,
) -> impl IntoResponse {
    let Some((project_slug, workspace_name)) =
        workspace_scope_from_snapshot(&state.engine.app_snapshot().await.ok(), workspace_id)
    else {
        return (axum::http::StatusCode::NOT_FOUND, "workspace not found").into_response();
    };

    match state
        .services
        .delete_context_item(project_slug, workspace_name, context_id)
    {
        Ok(()) => axum::http::StatusCode::NO_CONTENT.into_response(),
        Err(message) => (axum::http::StatusCode::INTERNAL_SERVER_ERROR, message).into_response(),
    }
}

async fn upload_attachment(
    State(state): State<AppStateHolder>,
    Path(workspace_id): Path<u64>,
    headers: axum::http::HeaderMap,
    mut multipart: Multipart,
) -> impl IntoResponse {
    let Some((project_slug, workspace_name)) =
        workspace_scope_from_snapshot(&state.engine.app_snapshot().await.ok(), workspace_id)
    else {
        return (axum::http::StatusCode::NOT_FOUND, "workspace not found").into_response();
    };

    let idempotency_key = headers
        .get("Idempotency-Key")
        .and_then(|v| v.to_str().ok())
        .map(|v| v.trim())
        .filter(|v| !v.is_empty())
        .map(|v| format!("upload_attachment:{workspace_id}:{v}"));

    let mut idempotency_owner = false;
    if let Some(key) = idempotency_key.clone() {
        match state.idempotency_attachments.begin(key.clone()).await {
            Begin::Done(value) => return Json(value).into_response(),
            Begin::Wait(rx) => match rx.await {
                Ok(Ok(value)) => return Json(value).into_response(),
                Ok(Err(message)) => {
                    return (axum::http::StatusCode::INTERNAL_SERVER_ERROR, message)
                        .into_response();
                }
                Err(_) => {
                    return (
                        axum::http::StatusCode::INTERNAL_SERVER_ERROR,
                        "failed to await idempotency result",
                    )
                        .into_response();
                }
            },
            Begin::Owner => idempotency_owner = true,
        }
    }

    let result: Result<luban_api::AttachmentRef, (axum::http::StatusCode, String)> = async {
        let mut file_bytes: Option<Vec<u8>> = None;
        let mut file_name: Option<String> = None;
        let mut content_type: Option<String> = None;
        let mut kind: Option<String> = None;

        while let Ok(Some(field)) = multipart.next_field().await {
            let name = field.name().unwrap_or("").to_owned();
            if name == "kind" {
                if let Ok(text) = field.text().await {
                    kind = Some(text);
                }
                continue;
            }
            if name != "file" {
                continue;
            }

            file_name = field.file_name().map(|s| s.to_owned());
            content_type = field.content_type().map(|m| m.to_string());
            file_bytes = field.bytes().await.ok().map(|b| b.to_vec());
        }

        let Some(bytes) = file_bytes else {
            return Err((
                axum::http::StatusCode::BAD_REQUEST,
                "missing multipart field: file".to_owned(),
            ));
        };

        let resolved_kind = kind
            .as_deref()
            .map(|s| s.trim().to_ascii_lowercase())
            .or_else(|| {
                content_type
                    .as_deref()
                    .map(|ct| ct.trim().to_ascii_lowercase())
            })
            .unwrap_or_else(|| "file".to_owned());

        let name = file_name.unwrap_or_else(|| "attachment".to_owned());
        let extension = name
            .rsplit_once('.')
            .map(|(_, ext)| ext.to_ascii_lowercase())
            .unwrap_or_else(|| "bin".to_owned());

        let uploaded_at_ms = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64;
        let display_name = append_timestamp_to_basename(&name, uploaded_at_ms);

        let stored = if resolved_kind.starts_with("image") {
            state.services.store_context_image(
                project_slug.clone(),
                workspace_name.clone(),
                ContextImage { extension, bytes },
            )
        } else if resolved_kind.starts_with("text") {
            let text = match String::from_utf8(bytes) {
                Ok(text) => text,
                Err(_) => {
                    return Err((
                        axum::http::StatusCode::BAD_REQUEST,
                        "text attachments must be valid UTF-8".to_owned(),
                    ));
                }
            };
            state.services.store_context_text(
                project_slug.clone(),
                workspace_name.clone(),
                text,
                extension,
            )
        } else {
            let unique = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_nanos();
            let tmp_dir = std::env::temp_dir().join(format!(
                "luban-upload-{}-{}",
                std::process::id(),
                unique
            ));
            if let Err(err) = std::fs::create_dir_all(&tmp_dir) {
                return Err((
                    axum::http::StatusCode::INTERNAL_SERVER_ERROR,
                    format!("failed to create tmp dir: {err}"),
                ));
            }

            let tmp_path = tmp_dir.join(display_name.clone());
            if let Err(err) = std::fs::write(&tmp_path, &bytes) {
                let _ = std::fs::remove_dir_all(&tmp_dir);
                return Err((
                    axum::http::StatusCode::INTERNAL_SERVER_ERROR,
                    format!("failed to write tmp file: {err}"),
                ));
            }

            let stored = state.services.store_context_file(
                project_slug.clone(),
                workspace_name.clone(),
                tmp_path,
            );
            let _ = std::fs::remove_dir_all(&tmp_dir);
            stored
        };

        match stored {
            Ok(mut att) => {
                att.name = display_name;
                if let Err(message) = state.services.record_context_item(
                    project_slug.clone(),
                    workspace_name.clone(),
                    att.clone(),
                    uploaded_at_ms,
                ) {
                    return Err((axum::http::StatusCode::INTERNAL_SERVER_ERROR, message));
                }

                let api = luban_api::AttachmentRef {
                    id: att.id,
                    kind: match att.kind {
                        luban_domain::AttachmentKind::Image => luban_api::AttachmentKind::Image,
                        luban_domain::AttachmentKind::Text => luban_api::AttachmentKind::Text,
                        luban_domain::AttachmentKind::File => luban_api::AttachmentKind::File,
                    },
                    name: att.name,
                    extension: att.extension,
                    mime: att.mime,
                    byte_len: att.byte_len,
                };
                Ok(api)
            }
            Err(message) => Err((axum::http::StatusCode::INTERNAL_SERVER_ERROR, message)),
        }
    }
    .await;

    if idempotency_owner && let Some(key) = idempotency_key.clone() {
        match &result {
            Ok(value) => {
                state
                    .idempotency_attachments
                    .complete(key, Ok(value.clone()))
                    .await
            }
            Err((_status, message)) => {
                state
                    .idempotency_attachments
                    .complete(key, Err(message.clone()))
                    .await
            }
        }
    }

    match result {
        Ok(value) => Json(value).into_response(),
        Err((status, message)) => (status, message).into_response(),
    }
}

fn append_timestamp_to_basename(name: &str, unix_ms: u64) -> String {
    let raw_name = std::path::Path::new(name)
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or(name)
        .trim();

    let raw_name = if raw_name.is_empty() {
        "file"
    } else {
        raw_name
    };

    let path = std::path::Path::new(raw_name);
    let stem = path
        .file_stem()
        .and_then(|s| s.to_str())
        .filter(|s| !s.trim().is_empty())
        .unwrap_or("file");
    let ext = path.extension().and_then(|s| s.to_str()).unwrap_or("");

    if ext.is_empty() {
        format!("{stem}-{unix_ms}")
    } else {
        format!("{stem}-{unix_ms}.{ext}")
    }
}

fn workspace_scope_from_snapshot(
    snapshot: &Option<AppSnapshot>,
    workspace_id: u64,
) -> Option<(String, String)> {
    let snapshot = snapshot.as_ref()?;
    for project in &snapshot.projects {
        for workspace in &project.workspaces {
            if workspace.id.0 == workspace_id {
                return Some((project.slug.clone(), workspace.workspace_name.clone()));
            }
        }
    }
    None
}

fn workspace_info_from_snapshot(
    snapshot: &Option<AppSnapshot>,
    workspace_id: u64,
) -> Option<(String, String, String)> {
    let snapshot = snapshot.as_ref()?;
    for project in &snapshot.projects {
        for workspace in &project.workspaces {
            if workspace.id.0 == workspace_id {
                return Some((
                    project.slug.clone(),
                    workspace.workspace_name.clone(),
                    workspace.worktree_path.clone(),
                ));
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::append_timestamp_to_basename;

    #[test]
    fn timestamp_appended_for_simple_names() {
        assert_eq!(
            append_timestamp_to_basename("report.txt", 123),
            "report-123.txt"
        );
        assert_eq!(append_timestamp_to_basename("archive", 456), "archive-456");
    }

    #[test]
    fn timestamp_uses_basename_only() {
        assert_eq!(
            append_timestamp_to_basename("../path/to/file.md", 42),
            "file-42.md"
        );
    }

    #[test]
    fn timestamp_handles_empty_names() {
        assert_eq!(append_timestamp_to_basename("", 9), "file-9");
        assert_eq!(append_timestamp_to_basename("   ", 9), "file-9");
    }
}
