use crate::auth;
use crate::engine::{Engine, EngineHandle, new_default_services};
use crate::idempotency::{Begin, IdempotencyStore};
use crate::mentions;
use crate::project_avatars;
use crate::pty::PtyManager;
use anyhow::Context as _;
use axum::middleware;
use axum::{
    Json, Router,
    extract::{Multipart, Path, Query, State, ws::WebSocketUpgrade},
    response::IntoResponse,
    routing::{delete, get, post, put},
};
use base64::Engine as _;
use luban_api::AppSnapshot;
use luban_api::{
    CodexCustomPromptSnapshot, PROTOCOL_VERSION, WorkspaceChangesSnapshot, WorkspaceDiffSnapshot,
    WsClientMessage, WsServerMessage,
};
use luban_domain::paths;
use luban_domain::{
    ContextImage, ProjectWorkspaceService, TaskDocumentKind as DomainTaskDocumentKind,
};
use rand::RngCore as _;
use std::path::{Path as FsPath, PathBuf};
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tokio::sync::broadcast;
use tower_http::services::{ServeDir, ServeFile};

pub async fn router(config: crate::ServerConfig) -> anyhow::Result<Router> {
    let services = new_default_services()?;
    let (engine, events) = Engine::start(services.clone());
    crate::telegram::start_gateway(engine.clone(), events.clone());

    let avatar_http = reqwest::Client::builder()
        .timeout(Duration::from_secs(10))
        .connect_timeout(Duration::from_secs(3))
        .redirect(reqwest::redirect::Policy::limited(10))
        .user_agent("luban")
        .build()
        .context("failed to build avatar http client")?;

    let state = AppStateHolder {
        engine,
        events,
        pty: PtyManager::new(),
        services,
        avatar_http,
        auth: auth::AuthState::new(config.auth),
        idempotency_attachments: IdempotencyStore::new(
            std::time::Duration::from_secs(10 * 60),
            256,
        ),
    };

    let api_public = Router::new().route("/health", get(health));

    let api_protected = Router::new()
        .route("/app", get(get_app))
        .route("/projects/avatar", get(get_project_avatar))
        .route("/codex/prompts", get(get_codex_prompts))
        .route("/tasks", get(get_tasks))
        .route(
            "/new_task/drafts",
            get(list_new_task_drafts).post(create_new_task_draft),
        )
        .route(
            "/new_task/drafts/{draft_id}",
            put(update_new_task_draft).delete(delete_new_task_draft),
        )
        .route(
            "/new_task/stash",
            get(get_new_task_stash)
                .put(save_new_task_stash)
                .delete(clear_new_task_stash),
        )
        .route("/workdirs/{workdir_id}/tasks", get(get_threads))
        .route(
            "/workdirs/{workdir_id}/conversations/{task_id}",
            get(get_conversation),
        )
        .route(
            "/workdirs/{workdir_id}/tasks/{task_id}/documents",
            get(get_task_documents),
        )
        .route(
            "/workdirs/{workdir_id}/tasks/{task_id}/documents/{kind}",
            get(get_task_document).put(update_task_document),
        )
        .route(
            "/workdirs/{workdir_id}/attachments",
            post(upload_attachment),
        )
        .route(
            "/workdirs/{workdir_id}/attachments/{attachment_id}",
            get(download_attachment),
        )
        .route("/workdirs/{workdir_id}/changes", get(get_changes))
        .route("/workdirs/{workdir_id}/diff", get(get_diff))
        .route("/workdirs/{workdir_id}/context", get(get_context))
        .route(
            "/workdirs/{workdir_id}/mentions",
            get(get_workspace_mentions),
        )
        .route(
            "/workdirs/{workdir_id}/context/{context_id}",
            delete(delete_context_item),
        )
        .route("/events", get(ws_events))
        .route("/pty/{workdir_id}/{task_id}", get(ws_pty))
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
    avatar_http: reqwest::Client,
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

#[derive(serde::Deserialize)]
struct ProjectAvatarQuery {
    project_id: String,
}

fn is_safe_project_id(id: &str) -> bool {
    let id = id.trim();
    if id.is_empty() || id.len() > 4096 {
        return false;
    }
    !id.chars().any(|c| c.is_control())
}

async fn get_project_avatar(
    State(state): State<AppStateHolder>,
    Query(query): Query<ProjectAvatarQuery>,
) -> impl IntoResponse {
    if !is_safe_project_id(&query.project_id) {
        return (axum::http::StatusCode::BAD_REQUEST, "invalid project_id").into_response();
    }

    let snapshot = match state.engine.app_snapshot().await {
        Ok(snapshot) => snapshot,
        Err(err) => {
            return (
                axum::http::StatusCode::INTERNAL_SERVER_ERROR,
                err.to_string(),
            )
                .into_response();
        }
    };
    let Some(project) = snapshot
        .projects
        .iter()
        .find(|p| p.id.0 == query.project_id)
    else {
        return (axum::http::StatusCode::NOT_FOUND, "project not found").into_response();
    };

    let project_path = PathBuf::from(&project.path);
    let services = state.services.clone();
    let identity =
        match tokio::task::spawn_blocking(move || services.project_identity(project_path)).await {
            Ok(Ok(identity)) => identity,
            Ok(Err(err)) => {
                tracing::warn!(error = %err, "failed to load project identity for avatar");
                return (axum::http::StatusCode::NOT_FOUND, "avatar not available").into_response();
            }
            Err(err) => {
                return (
                    axum::http::StatusCode::INTERNAL_SERVER_ERROR,
                    format!("failed to resolve project identity: {err}"),
                )
                    .into_response();
            }
        };

    let Some(github_repo) = identity.github_repo.as_deref() else {
        return (axum::http::StatusCode::NOT_FOUND, "avatar not available").into_response();
    };
    let Some(owner) = project_avatars::github_owner_from_repo_id(github_repo) else {
        return (axum::http::StatusCode::NOT_FOUND, "avatar not available").into_response();
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

    match project_avatars::get_or_fetch_owner_avatar_png(&state.avatar_http, &luban_root, owner)
        .await
    {
        Ok(Some(bytes)) => (
            [
                (axum::http::header::CONTENT_TYPE, "image/png"),
                (axum::http::header::CACHE_CONTROL, "private, max-age=86400"),
            ],
            bytes,
        )
            .into_response(),
        Ok(None) => (axum::http::StatusCode::NOT_FOUND, "avatar not available").into_response(),
        Err(err) => {
            tracing::warn!(error = %err, owner, "failed to load project avatar");
            (axum::http::StatusCode::BAD_GATEWAY, "avatar fetch failed").into_response()
        }
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

#[derive(serde::Deserialize)]
struct TasksQuery {
    project_id: Option<String>,
    workdir_status: Option<String>,
    task_status: Option<String>,
}

async fn get_tasks(
    State(state): State<AppStateHolder>,
    Query(query): Query<TasksQuery>,
) -> impl IntoResponse {
    let app = match state.engine.app_snapshot().await {
        Ok(snapshot) => snapshot,
        Err(err) => {
            return (
                axum::http::StatusCode::INTERNAL_SERVER_ERROR,
                err.to_string(),
            )
                .into_response();
        }
    };

    let starred = state
        .engine
        .starred_tasks_snapshot()
        .await
        .unwrap_or_default();

    let mut tasks = Vec::<luban_api::TaskSummarySnapshot>::new();
    let selected_project_id = query
        .project_id
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty());

    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    enum WorkdirStatusFilter {
        Active,
        Archived,
        All,
    }

    let workdir_status_filter = match query
        .workdir_status
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
    {
        None => WorkdirStatusFilter::Active,
        Some("active") => WorkdirStatusFilter::Active,
        Some("archived") => WorkdirStatusFilter::Archived,
        Some("all") => WorkdirStatusFilter::All,
        Some(other) => {
            return (
                axum::http::StatusCode::BAD_REQUEST,
                format!("invalid workdir_status: {other}"),
            )
                .into_response();
        }
    };

    fn parse_task_status(value: &str) -> Option<luban_api::TaskStatus> {
        match value {
            "backlog" => Some(luban_api::TaskStatus::Backlog),
            "todo" => Some(luban_api::TaskStatus::Todo),
            "iterating" | "in_progress" => Some(luban_api::TaskStatus::Iterating),
            "validating" | "in_review" => Some(luban_api::TaskStatus::Validating),
            "done" => Some(luban_api::TaskStatus::Done),
            "canceled" => Some(luban_api::TaskStatus::Canceled),
            _ => None,
        }
    }

    let task_status_filter: Option<Vec<luban_api::TaskStatus>> = match query
        .task_status
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
    {
        None => None,
        Some("all") => None,
        Some(raw) => {
            let mut out = Vec::new();
            for part in raw.split(',') {
                let trimmed = part.trim();
                if trimmed.is_empty() {
                    continue;
                }
                let Some(status) = parse_task_status(trimmed) else {
                    return (
                        axum::http::StatusCode::BAD_REQUEST,
                        format!("invalid task_status: {trimmed}"),
                    )
                        .into_response();
                };
                if !out.contains(&status) {
                    out.push(status);
                }
            }
            Some(out)
        }
    };

    for p in &app.projects {
        if let Some(selected) = selected_project_id
            && p.id.0 != selected
        {
            continue;
        }

        for w in &p.workspaces {
            match workdir_status_filter {
                WorkdirStatusFilter::Active => {
                    if w.status != luban_api::WorkspaceStatus::Active {
                        continue;
                    }
                }
                WorkdirStatusFilter::Archived => {
                    if w.status != luban_api::WorkspaceStatus::Archived {
                        continue;
                    }
                }
                WorkdirStatusFilter::All => {}
            }

            let snap = match state.engine.threads_snapshot(w.id).await {
                Ok(v) => v,
                Err(_) => continue,
            };

            let active_task_id = snap.tabs.active_tab;
            for t in snap.threads {
                if let Some(filter) = task_status_filter.as_ref()
                    && !filter.contains(&t.task_status)
                {
                    continue;
                }

                let agent_run_status = if t.thread_id == active_task_id
                    && w.agent_run_status == luban_api::OperationStatus::Running
                {
                    luban_api::OperationStatus::Running
                } else {
                    luban_api::OperationStatus::Idle
                };

                let has_unread_completion =
                    t.thread_id == active_task_id && w.has_unread_completion;

                tasks.push(luban_api::TaskSummarySnapshot {
                    project_id: p.id.clone(),
                    workspace_id: w.id,
                    thread_id: t.thread_id,
                    title: t.title,
                    created_at_unix_seconds: t.created_at_unix_seconds,
                    updated_at_unix_seconds: t.updated_at_unix_seconds,
                    branch_name: w.branch_name.clone(),
                    workspace_name: w.workspace_name.clone(),
                    agent_run_status,
                    has_unread_completion,
                    task_status: t.task_status,
                    turn_status: t.turn_status,
                    last_turn_result: t.last_turn_result,
                    is_starred: starred.contains(&(w.id.0, t.thread_id.0)),
                });
            }
        }
    }

    Json(luban_api::TasksSnapshot {
        rev: app.rev,
        tasks,
    })
    .into_response()
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
            return (axum::http::StatusCode::NOT_FOUND, "workdir not found").into_response();
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
struct UpdateTaskDocumentRequest {
    content: String,
}

const TASK_DOCUMENT_SCHEMA_VERSION: u32 = 1;

#[derive(serde::Serialize, serde::Deserialize)]
struct TaskDocumentIndexRecord {
    task_ulid: String,
    workspace_id: u64,
    task_id: u64,
}

#[derive(Clone, serde::Serialize, serde::Deserialize)]
struct TaskDocumentIdentityRecord {
    schema_version: u32,
    task_ulid: String,
    workspace_id: u64,
    task_id: u64,
    created_at_unix_ms: u64,
    updated_at_unix_ms: u64,
}

fn to_api_task_document_kind(kind: DomainTaskDocumentKind) -> luban_api::TaskDocumentKind {
    match kind {
        DomainTaskDocumentKind::Task => luban_api::TaskDocumentKind::Task,
        DomainTaskDocumentKind::Plan => luban_api::TaskDocumentKind::Plan,
        DomainTaskDocumentKind::Memory => luban_api::TaskDocumentKind::Memory,
    }
}

fn task_document_order() -> [DomainTaskDocumentKind; 3] {
    [
        DomainTaskDocumentKind::Task,
        DomainTaskDocumentKind::Plan,
        DomainTaskDocumentKind::Memory,
    ]
}

fn task_documents_v1_root(luban_root: &FsPath) -> PathBuf {
    luban_root.join("tasks").join("v1")
}

fn task_documents_tasks_root(luban_root: &FsPath) -> PathBuf {
    task_documents_v1_root(luban_root).join("tasks")
}

fn task_documents_index_root(luban_root: &FsPath) -> PathBuf {
    task_documents_v1_root(luban_root)
        .join("index")
        .join("workdir_task")
}

fn task_document_index_path(luban_root: &FsPath, workspace_id: u64, task_id: u64) -> PathBuf {
    task_documents_index_root(luban_root).join(format!("{workspace_id}-{task_id}.json"))
}

fn task_document_task_dir(luban_root: &FsPath, task_ulid: &str) -> PathBuf {
    task_documents_tasks_root(luban_root).join(task_ulid)
}

fn task_document_meta_path(luban_root: &FsPath, task_ulid: &str) -> PathBuf {
    task_document_task_dir(luban_root, task_ulid).join("task.json")
}

fn task_document_relative_path(task_ulid: &str, kind: DomainTaskDocumentKind) -> String {
    format!("tasks/v1/tasks/{task_ulid}/{}", kind.file_name())
}

fn fallback_task_ulid(workspace_id: u64, task_id: u64) -> String {
    format!("task-{workspace_id}-{task_id}")
}

fn file_updated_at_unix_ms(path: &FsPath) -> u64 {
    let from_meta = std::fs::metadata(path)
        .ok()
        .and_then(|meta| meta.modified().ok())
        .and_then(|time| time.duration_since(UNIX_EPOCH).ok())
        .map(|duration| duration.as_millis() as u64);
    from_meta.unwrap_or_else(now_unix_millis)
}

fn write_text_atomic(path: &FsPath, content: &str) -> anyhow::Result<()> {
    let parent = path
        .parent()
        .ok_or_else(|| anyhow::anyhow!("invalid task document path"))?;
    std::fs::create_dir_all(parent)
        .with_context(|| format!("failed to create {}", parent.display()))?;

    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let tmp = parent.join(format!(".tmp-{}-{unique}", std::process::id()));
    std::fs::write(&tmp, content.as_bytes())
        .with_context(|| format!("failed to write {}", tmp.display()))?;
    std::fs::rename(&tmp, path).with_context(|| format!("failed to replace {}", path.display()))?;
    Ok(())
}

fn document_hash(content: &str) -> String {
    blake3::hash(content.as_bytes()).to_hex().to_string()
}

fn build_task_document_snapshot(
    task_ulid: &str,
    kind: DomainTaskDocumentKind,
    content: String,
    content_hash: String,
    byte_len: u64,
    updated_at_unix_ms: u64,
) -> luban_api::TaskDocumentSnapshot {
    luban_api::TaskDocumentSnapshot {
        kind: to_api_task_document_kind(kind),
        rel_path: task_document_relative_path(task_ulid, kind),
        content,
        content_hash,
        byte_len,
        updated_at_unix_ms,
    }
}

fn read_task_document_identity(
    luban_root: &FsPath,
    workspace_id: u64,
    task_id: u64,
) -> anyhow::Result<Option<TaskDocumentIdentityRecord>> {
    let index_path = task_document_index_path(luban_root, workspace_id, task_id);
    if !index_path.exists() {
        return Ok(None);
    }

    let index_content = std::fs::read_to_string(&index_path)
        .with_context(|| format!("failed to read {}", index_path.display()))?;
    let index: TaskDocumentIndexRecord = serde_json::from_str(&index_content)
        .with_context(|| format!("failed to parse {}", index_path.display()))?;
    if index.workspace_id != workspace_id || index.task_id != task_id {
        return Ok(None);
    }

    let meta_path = task_document_meta_path(luban_root, &index.task_ulid);
    if !meta_path.exists() {
        return Ok(None);
    }
    let meta_content = std::fs::read_to_string(&meta_path)
        .with_context(|| format!("failed to read {}", meta_path.display()))?;
    let identity: TaskDocumentIdentityRecord = serde_json::from_str(&meta_content)
        .with_context(|| format!("failed to parse {}", meta_path.display()))?;
    if identity.workspace_id != workspace_id || identity.task_id != task_id {
        return Ok(None);
    }
    Ok(Some(identity))
}

fn write_task_document_identity(
    luban_root: &FsPath,
    identity: &TaskDocumentIdentityRecord,
) -> anyhow::Result<()> {
    let task_dir = task_document_task_dir(luban_root, &identity.task_ulid);
    std::fs::create_dir_all(&task_dir)
        .with_context(|| format!("failed to create {}", task_dir.display()))?;

    let meta_path = task_document_meta_path(luban_root, &identity.task_ulid);
    let meta_json = serde_json::to_string_pretty(identity)?;
    write_text_atomic(&meta_path, &meta_json)?;

    let index_path = task_document_index_path(luban_root, identity.workspace_id, identity.task_id);
    let index = TaskDocumentIndexRecord {
        task_ulid: identity.task_ulid.clone(),
        workspace_id: identity.workspace_id,
        task_id: identity.task_id,
    };
    let index_json = serde_json::to_string_pretty(&index)?;
    write_text_atomic(&index_path, &index_json)?;
    Ok(())
}

fn ensure_task_document_identity(
    luban_root: &FsPath,
    workspace_id: u64,
    task_id: u64,
) -> anyhow::Result<TaskDocumentIdentityRecord> {
    if let Some(identity) = read_task_document_identity(luban_root, workspace_id, task_id)? {
        return Ok(identity);
    }

    let now = now_unix_millis();
    let identity = TaskDocumentIdentityRecord {
        schema_version: TASK_DOCUMENT_SCHEMA_VERSION,
        task_ulid: ulid::Ulid::new().to_string(),
        workspace_id,
        task_id,
        created_at_unix_ms: now,
        updated_at_unix_ms: now,
    };
    write_task_document_identity(luban_root, &identity)?;
    Ok(identity)
}

fn touch_task_document_identity(
    luban_root: &FsPath,
    identity: &mut TaskDocumentIdentityRecord,
) -> anyhow::Result<()> {
    identity.updated_at_unix_ms = now_unix_millis();
    write_task_document_identity(luban_root, identity)
}

fn resolve_task_document_location(
    luban_root: &FsPath,
    workspace_id: u64,
    task_id: u64,
) -> anyhow::Result<(String, u64, PathBuf)> {
    let identity = read_task_document_identity(luban_root, workspace_id, task_id)?
        .map(|record| (record.task_ulid, record.updated_at_unix_ms));
    let (task_ulid, fallback_updated_at_unix_ms) =
        identity.unwrap_or_else(|| (fallback_task_ulid(workspace_id, task_id), now_unix_millis()));
    let root = task_document_task_dir(luban_root, &task_ulid);
    Ok((task_ulid, fallback_updated_at_unix_ms, root))
}

fn read_task_document_snapshot_at(
    task_ulid: &str,
    root: &FsPath,
    fallback_updated_at_unix_ms: u64,
    kind: DomainTaskDocumentKind,
) -> anyhow::Result<luban_api::TaskDocumentSnapshot> {
    let path = root.join(kind.file_name());
    let existed = path.exists();
    let content = if existed {
        std::fs::read_to_string(&path)
            .with_context(|| format!("failed to read {}", path.display()))?
    } else {
        String::new()
    };
    let updated_at_unix_ms = if existed {
        file_updated_at_unix_ms(&path)
    } else {
        fallback_updated_at_unix_ms
    };
    let byte_len = content.len() as u64;
    let content_hash = document_hash(&content);
    Ok(build_task_document_snapshot(
        task_ulid,
        kind,
        content,
        content_hash,
        byte_len,
        updated_at_unix_ms,
    ))
}

async fn get_task_document(
    State(_state): State<AppStateHolder>,
    Path((workspace_id, task_id, kind)): Path<(u64, u64, String)>,
) -> impl IntoResponse {
    let Some(kind) = DomainTaskDocumentKind::parse_key(&kind) else {
        return (
            axum::http::StatusCode::BAD_REQUEST,
            "invalid task document kind",
        )
            .into_response();
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

    let result = tokio::task::spawn_blocking(move || {
        let (task_ulid, fallback_updated_at_unix_ms, root) =
            resolve_task_document_location(FsPath::new(&luban_root), workspace_id, task_id)?;
        read_task_document_snapshot_at(&task_ulid, &root, fallback_updated_at_unix_ms, kind)
    })
    .await;

    match result {
        Ok(Ok(document)) => Json(document).into_response(),
        Ok(Err(err)) => (
            axum::http::StatusCode::INTERNAL_SERVER_ERROR,
            err.to_string(),
        )
            .into_response(),
        Err(err) => (
            axum::http::StatusCode::INTERNAL_SERVER_ERROR,
            format!("failed to join task document read task: {err}"),
        )
            .into_response(),
    }
}

async fn get_task_documents(
    State(_state): State<AppStateHolder>,
    Path((workspace_id, task_id)): Path<(u64, u64)>,
) -> impl IntoResponse {
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

    let result = tokio::task::spawn_blocking(move || {
        let (task_ulid, fallback_updated_at_unix_ms, root) =
            resolve_task_document_location(FsPath::new(&luban_root), workspace_id, task_id)?;
        let mut documents = Vec::new();
        for kind in task_document_order() {
            documents.push(read_task_document_snapshot_at(
                &task_ulid,
                &root,
                fallback_updated_at_unix_ms,
                kind,
            )?);
        }

        Ok::<Vec<luban_api::TaskDocumentSnapshot>, anyhow::Error>(documents)
    })
    .await;

    match result {
        Ok(Ok(documents)) => Json(luban_api::TaskDocumentsSnapshot {
            workspace_id: luban_api::WorkspaceId(workspace_id),
            thread_id: luban_api::WorkspaceThreadId(task_id),
            documents,
        })
        .into_response(),
        Ok(Err(err)) => (
            axum::http::StatusCode::INTERNAL_SERVER_ERROR,
            err.to_string(),
        )
            .into_response(),
        Err(err) => (
            axum::http::StatusCode::INTERNAL_SERVER_ERROR,
            format!("failed to join task document read task: {err}"),
        )
            .into_response(),
    }
}

async fn update_task_document(
    State(_state): State<AppStateHolder>,
    Path((workspace_id, task_id, kind)): Path<(u64, u64, String)>,
    Json(req): Json<UpdateTaskDocumentRequest>,
) -> impl IntoResponse {
    let Some(kind) = DomainTaskDocumentKind::parse_key(&kind) else {
        return (
            axum::http::StatusCode::BAD_REQUEST,
            "invalid task document kind",
        )
            .into_response();
    };
    if req.content.len() > 1_000_000 {
        return (
            axum::http::StatusCode::BAD_REQUEST,
            "document content too large",
        )
            .into_response();
    }

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

    let content = req.content;
    let result = tokio::task::spawn_blocking(move || {
        let mut identity =
            ensure_task_document_identity(FsPath::new(&luban_root), workspace_id, task_id)?;
        let root = task_document_task_dir(FsPath::new(&luban_root), &identity.task_ulid);
        std::fs::create_dir_all(&root)
            .with_context(|| format!("failed to create {}", root.display()))?;
        let path = root.join(kind.file_name());
        write_text_atomic(&path, &content)?;
        touch_task_document_identity(FsPath::new(&luban_root), &mut identity)?;

        let updated_at_unix_ms = file_updated_at_unix_ms(&path);
        let byte_len = content.len() as u64;
        let content_hash = document_hash(&content);
        Ok::<luban_api::TaskDocumentSnapshot, anyhow::Error>(build_task_document_snapshot(
            &identity.task_ulid,
            kind,
            content,
            content_hash,
            byte_len,
            updated_at_unix_ms,
        ))
    })
    .await;

    match result {
        Ok(Ok(document)) => Json(document).into_response(),
        Ok(Err(err)) => (
            axum::http::StatusCode::INTERNAL_SERVER_ERROR,
            err.to_string(),
        )
            .into_response(),
        Err(err) => (
            axum::http::StatusCode::INTERNAL_SERVER_ERROR,
            format!("failed to join task document update task: {err}"),
        )
            .into_response(),
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
                if handle_ws_incoming(msg, &state, &mut socket).await.is_err() {
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
    state: &AppStateHolder,
    socket: &mut axum::extract::ws::WebSocket,
) -> anyhow::Result<()> {
    let axum::extract::ws::Message::Text(text) = msg else {
        return Ok(());
    };

    let engine = &state.engine;

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
        WsClientMessage::Action { request_id, action } => match *action {
            luban_api::ClientAction::TerminalCommandStart {
                workspace_id,
                thread_id,
                command,
            } => {
                handle_terminal_command_start(
                    request_id,
                    workspace_id,
                    thread_id,
                    command,
                    state,
                    socket,
                )
                .await
            }
            other => {
                let ack = engine.apply_client_action(request_id.clone(), other).await;
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
        },
    }
}

async fn handle_terminal_command_start(
    request_id: String,
    workspace_id: luban_api::WorkspaceId,
    thread_id: luban_api::WorkspaceThreadId,
    command: String,
    state: &AppStateHolder,
    socket: &mut axum::extract::ws::WebSocket,
) -> anyhow::Result<()> {
    let command = command.trim().to_owned();
    if command.is_empty() {
        socket
            .send(json_text(&WsServerMessage::Error {
                request_id: Some(request_id),
                message: "command is empty".to_owned(),
            }))
            .await?;
        return Ok(());
    }

    let cwd = match state.engine.workspace_worktree_path(workspace_id).await {
        Ok(Some(path)) => path,
        _ => std::env::current_dir().unwrap_or_default(),
    };

    let mut id_bytes = [0u8; 16];
    rand::rngs::OsRng.fill_bytes(&mut id_bytes);
    let command_id = format!(
        "cmd_{}",
        base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(id_bytes)
    );

    let mut reconnect_bytes = [0u8; 16];
    rand::rngs::OsRng.fill_bytes(&mut reconnect_bytes);
    let reconnect = format!(
        "reconnect_{}",
        base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(reconnect_bytes)
    );

    state
        .engine
        .dispatch_domain_action(luban_domain::Action::TerminalCommandStarted {
            workspace_id: luban_domain::WorkspaceId::from_u64(workspace_id.0),
            thread_id: luban_domain::WorkspaceThreadId::from_u64(thread_id.0),
            command_id: command_id.clone(),
            command: command.clone(),
            reconnect: reconnect.clone(),
        })
        .await
        .map_err(|err| anyhow::anyhow!(err.to_string()))?;

    let session =
        match state
            .pty
            .spawn_command(workspace_id.0, reconnect.clone(), cwd, command.clone())
        {
            Ok(session) => session,
            Err(err) => {
                tracing::error!(error = %err, "failed to create terminal command pty session");
                let _ = state
                    .engine
                    .dispatch_domain_action(luban_domain::Action::TerminalCommandFinished {
                        workspace_id: luban_domain::WorkspaceId::from_u64(workspace_id.0),
                        thread_id: luban_domain::WorkspaceThreadId::from_u64(thread_id.0),
                        command_id: command_id.clone(),
                        command: command.clone(),
                        reconnect: reconnect.clone(),
                        output_base64: String::new(),
                        output_byte_len: 0,
                    })
                    .await;

                socket
                    .send(json_text(&WsServerMessage::Error {
                        request_id: Some(request_id),
                        message: "failed to create terminal session".to_owned(),
                    }))
                    .await?;
                return Ok(());
            }
        };

    let engine = state.engine.clone();
    tokio::spawn(async move {
        let mut terminated = session.subscribe_terminated();
        let _ = terminated.recv().await;
        let (bytes, output_byte_len) = session.output_snapshot();
        let output_base64 = if output_byte_len > 0 {
            base64::engine::general_purpose::STANDARD.encode(bytes)
        } else {
            String::new()
        };

        let _ = engine
            .dispatch_domain_action(luban_domain::Action::TerminalCommandFinished {
                workspace_id: luban_domain::WorkspaceId::from_u64(workspace_id.0),
                thread_id: luban_domain::WorkspaceThreadId::from_u64(thread_id.0),
                command_id,
                command,
                reconnect,
                output_base64,
                output_byte_len,
            })
            .await;
    });

    let rev = state.engine.current_rev().await.unwrap_or(0);
    socket
        .send(json_text(&WsServerMessage::Ack { request_id, rev }))
        .await?;
    Ok(())
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

fn now_unix_millis() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}

fn is_safe_new_task_draft_id(id: &str) -> bool {
    let id = id.trim();
    if id.is_empty() || id.len() > 128 {
        return false;
    }
    id.chars()
        .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
}

fn map_new_task_draft_snapshot(
    draft: luban_domain::NewTaskDraft,
) -> luban_api::NewTaskDraftSnapshot {
    luban_api::NewTaskDraftSnapshot {
        id: draft.id,
        text: draft.text,
        project_id: draft.project_id.map(luban_api::ProjectId),
        workspace_id: draft.workspace_id.map(luban_api::WorkspaceId),
        created_at_unix_ms: draft.created_at_unix_ms,
        updated_at_unix_ms: draft.updated_at_unix_ms,
    }
}

fn map_new_task_stash_snapshot(
    stash: luban_domain::NewTaskStash,
) -> luban_api::NewTaskStashSnapshot {
    luban_api::NewTaskStashSnapshot {
        text: stash.text,
        project_id: stash.project_id.map(luban_api::ProjectId),
        workspace_id: stash.workspace_id.map(luban_api::WorkspaceId),
        editing_draft_id: stash.editing_draft_id,
        updated_at_unix_ms: stash.updated_at_unix_ms,
    }
}

#[derive(serde::Deserialize)]
struct NewTaskDraftUpsertRequest {
    text: String,
    project_id: Option<String>,
    #[serde(default)]
    workdir_id: Option<u64>,
}

async fn list_new_task_drafts(State(state): State<AppStateHolder>) -> impl IntoResponse {
    match state.services.list_new_task_drafts() {
        Ok(drafts) => Json(luban_api::NewTaskDraftsSnapshot {
            drafts: drafts
                .into_iter()
                .map(map_new_task_draft_snapshot)
                .collect(),
        })
        .into_response(),
        Err(message) => (axum::http::StatusCode::INTERNAL_SERVER_ERROR, message).into_response(),
    }
}

async fn create_new_task_draft(
    State(state): State<AppStateHolder>,
    Json(req): Json<NewTaskDraftUpsertRequest>,
) -> impl IntoResponse {
    let text = req.text.trim();
    if text.is_empty() {
        return (axum::http::StatusCode::BAD_REQUEST, "text is required").into_response();
    }
    if req.text.len() > 100_000 {
        return (axum::http::StatusCode::BAD_REQUEST, "text too large").into_response();
    }

    match state
        .services
        .create_new_task_draft(req.text, req.project_id, req.workdir_id)
    {
        Ok(draft) => Json(map_new_task_draft_snapshot(draft)).into_response(),
        Err(message) => (axum::http::StatusCode::INTERNAL_SERVER_ERROR, message).into_response(),
    }
}

async fn update_new_task_draft(
    State(state): State<AppStateHolder>,
    Path(draft_id): Path<String>,
    Json(req): Json<NewTaskDraftUpsertRequest>,
) -> impl IntoResponse {
    if !is_safe_new_task_draft_id(&draft_id) {
        return (axum::http::StatusCode::BAD_REQUEST, "invalid draft id").into_response();
    }
    let text = req.text.trim();
    if text.is_empty() {
        return (axum::http::StatusCode::BAD_REQUEST, "text is required").into_response();
    }
    if req.text.len() > 100_000 {
        return (axum::http::StatusCode::BAD_REQUEST, "text too large").into_response();
    }

    match state
        .services
        .update_new_task_draft(draft_id, req.text, req.project_id, req.workdir_id)
    {
        Ok(draft) => Json(map_new_task_draft_snapshot(draft)).into_response(),
        Err(message) if message.contains("not found") => {
            (axum::http::StatusCode::NOT_FOUND, message).into_response()
        }
        Err(message) => (axum::http::StatusCode::INTERNAL_SERVER_ERROR, message).into_response(),
    }
}

async fn delete_new_task_draft(
    State(state): State<AppStateHolder>,
    Path(draft_id): Path<String>,
) -> impl IntoResponse {
    if !is_safe_new_task_draft_id(&draft_id) {
        return (axum::http::StatusCode::BAD_REQUEST, "invalid draft id").into_response();
    }

    match state.services.delete_new_task_draft(draft_id) {
        Ok(()) => axum::http::StatusCode::NO_CONTENT.into_response(),
        Err(message) => (axum::http::StatusCode::INTERNAL_SERVER_ERROR, message).into_response(),
    }
}

async fn get_new_task_stash(State(state): State<AppStateHolder>) -> impl IntoResponse {
    match state.services.load_new_task_stash() {
        Ok(stash) => Json(luban_api::NewTaskStashResponse {
            stash: stash.map(map_new_task_stash_snapshot),
        })
        .into_response(),
        Err(message) => (axum::http::StatusCode::INTERNAL_SERVER_ERROR, message).into_response(),
    }
}

#[derive(serde::Deserialize)]
struct NewTaskStashUpsertRequest {
    text: String,
    project_id: Option<String>,
    #[serde(default)]
    workdir_id: Option<u64>,
    #[serde(default)]
    editing_draft_id: Option<String>,
}

async fn save_new_task_stash(
    State(state): State<AppStateHolder>,
    Json(req): Json<NewTaskStashUpsertRequest>,
) -> impl IntoResponse {
    let text = req.text.trim();
    if text.is_empty() {
        return (axum::http::StatusCode::BAD_REQUEST, "text is required").into_response();
    }
    if req.text.len() > 100_000 {
        return (axum::http::StatusCode::BAD_REQUEST, "text too large").into_response();
    }

    let stash = luban_domain::NewTaskStash {
        text: req.text,
        project_id: req.project_id,
        workspace_id: req.workdir_id,
        editing_draft_id: req.editing_draft_id,
        updated_at_unix_ms: now_unix_millis(),
    };

    match state.services.save_new_task_stash(stash) {
        Ok(()) => axum::http::StatusCode::NO_CONTENT.into_response(),
        Err(message) => (axum::http::StatusCode::INTERNAL_SERVER_ERROR, message).into_response(),
    }
}

async fn clear_new_task_stash(State(state): State<AppStateHolder>) -> impl IntoResponse {
    match state.services.clear_new_task_stash() {
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
