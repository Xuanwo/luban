use crate::engine::{Engine, EngineHandle, new_default_services};
use crate::pty::PtyManager;
use axum::{
    Json, Router,
    extract::{Multipart, Path, Query, State, ws::WebSocketUpgrade},
    response::IntoResponse,
    routing::{get, post},
};
use luban_api::AppSnapshot;
use luban_api::{PROTOCOL_VERSION, WsClientMessage, WsServerMessage};
use luban_domain::{ContextImage, ProjectWorkspaceService};
use std::path::PathBuf;
use tokio::sync::broadcast;
use tower_http::services::{ServeDir, ServeFile};

pub async fn router() -> anyhow::Result<Router> {
    let services = new_default_services()?;
    let (engine, events) = Engine::start(services.clone());

    let state = AppStateHolder {
        engine,
        events,
        pty: PtyManager::new(),
        services,
    };

    let api = Router::new()
        .route("/health", get(health))
        .route("/app", get(get_app))
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
        .route("/events", get(ws_events))
        .route("/pty/{workspace_id}/{thread_id}", get(ws_pty))
        .with_state(state);

    let web_dist = std::env::var_os("LUBAN_WEB_DIST_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("web/out"));
    let web_index = web_dist.join("index.html");
    let web = ServeDir::new(web_dist).not_found_service(ServeFile::new(web_index));

    Ok(Router::new().nest("/api", api).fallback_service(web))
}

async fn health() -> &'static str {
    "ok"
}

#[derive(Clone)]
struct AppStateHolder {
    engine: EngineHandle,
    events: broadcast::Sender<WsServerMessage>,
    pty: PtyManager,
    services: std::sync::Arc<dyn ProjectWorkspaceService>,
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

async fn get_conversation(
    State(state): State<AppStateHolder>,
    Path((workspace_id, thread_id)): Path<(u64, u64)>,
) -> impl IntoResponse {
    match state
        .engine
        .conversation_snapshot(
            luban_api::WorkspaceId(workspace_id),
            luban_api::WorkspaceThreadId(thread_id),
        )
        .await
    {
        Ok(snapshot) => Json(snapshot).into_response(),
        Err(err) => (axum::http::StatusCode::NOT_FOUND, err.to_string()).into_response(),
    }
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
                let Ok(outgoing) = outgoing else { break };
                if socket.send(json_text(&outgoing)).await.is_err() {
                    break;
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
        WsClientMessage::Hello { .. } => Ok(()),
        WsClientMessage::Ping => {
            socket.send(json_text(&WsServerMessage::Pong)).await?;
            Ok(())
        }
        WsClientMessage::Action { request_id, action } => {
            let ack = engine.apply_client_action(action).await;
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

async fn ws_pty(
    ws: WebSocketUpgrade,
    State(state): State<AppStateHolder>,
    Path((workspace_id, thread_id)): Path<(u64, u64)>,
) -> impl IntoResponse {
    ws.on_upgrade(move |socket| ws_pty_task(socket, state, workspace_id, thread_id))
}

async fn ws_pty_task(
    socket: axum::extract::ws::WebSocket,
    state: AppStateHolder,
    workspace_id: u64,
    thread_id: u64,
) {
    let cwd = match state
        .engine
        .workspace_worktree_path(luban_api::WorkspaceId(workspace_id))
        .await
    {
        Ok(Some(path)) => path,
        _ => std::env::current_dir().unwrap_or_default(),
    };

    let session = match state.pty.get_or_create(workspace_id, thread_id, cwd) {
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

async fn download_attachment(
    State(state): State<AppStateHolder>,
    Path((workspace_id, attachment_id)): Path<(u64, String)>,
    Query(query): Query<AttachmentQuery>,
) -> impl IntoResponse {
    let Some((project_slug, workspace_name)) =
        workspace_scope_from_snapshot(&state.engine.app_snapshot().await.ok(), workspace_id)
    else {
        return (axum::http::StatusCode::NOT_FOUND, "workspace not found").into_response();
    };

    let Some(home) = std::env::var_os("HOME") else {
        return (
            axum::http::StatusCode::INTERNAL_SERVER_ERROR,
            "HOME is not set",
        )
            .into_response();
    };
    let luban_root = PathBuf::from(home).join("luban");
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

async fn upload_attachment(
    State(state): State<AppStateHolder>,
    Path(workspace_id): Path<u64>,
    mut multipart: Multipart,
) -> impl IntoResponse {
    let Some((project_slug, workspace_name)) =
        workspace_scope_from_snapshot(&state.engine.app_snapshot().await.ok(), workspace_id)
    else {
        return (axum::http::StatusCode::NOT_FOUND, "workspace not found").into_response();
    };

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
        return (
            axum::http::StatusCode::BAD_REQUEST,
            "missing multipart field: file",
        )
            .into_response();
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

    let stored = if resolved_kind.starts_with("image") {
        state.services.store_context_image(
            project_slug,
            workspace_name,
            ContextImage { extension, bytes },
        )
    } else if resolved_kind.starts_with("text") {
        let text = match String::from_utf8(bytes) {
            Ok(text) => text,
            Err(_) => {
                return (
                    axum::http::StatusCode::BAD_REQUEST,
                    "text attachments must be valid UTF-8",
                )
                    .into_response();
            }
        };
        state
            .services
            .store_context_text(project_slug, workspace_name, text, extension)
    } else {
        return (
            axum::http::StatusCode::BAD_REQUEST,
            "unsupported attachment kind (use image/* or text/*)",
        )
            .into_response();
    };

    match stored {
        Ok(att) => {
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
            Json(api).into_response()
        }
        Err(message) => (axum::http::StatusCode::INTERNAL_SERVER_ERROR, message).into_response(),
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
