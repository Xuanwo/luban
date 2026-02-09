use futures::{SinkExt as _, StreamExt as _};
use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tokio_tungstenite::tungstenite::Message;

static ENV_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

struct EnvGuard {
    _lock: std::sync::MutexGuard<'static, ()>,
    prev: Vec<(&'static str, Option<std::ffi::OsString>)>,
}

impl EnvGuard {
    fn lock(keys: Vec<&'static str>) -> Self {
        let lock = ENV_LOCK.lock().expect("env lock poisoned");
        let mut prev = Vec::with_capacity(keys.len());
        for key in keys {
            prev.push((key, std::env::var_os(key)));
        }
        Self { _lock: lock, prev }
    }

    fn set(&self, key: &'static str, value: &std::path::Path) {
        unsafe {
            std::env::set_var(key, value);
        }
    }

    fn set_str(&self, key: &'static str, value: &str) {
        unsafe {
            std::env::set_var(key, value);
        }
    }
}

impl Drop for EnvGuard {
    fn drop(&mut self) {
        for (key, prev) in self.prev.drain(..) {
            if let Some(prev) = prev {
                unsafe {
                    std::env::set_var(key, prev);
                }
            } else {
                unsafe {
                    std::env::remove_var(key);
                }
            }
        }
    }
}

async fn recv_ws_msg(
    socket: &mut tokio_tungstenite::WebSocketStream<
        tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>,
    >,
    timeout: Duration,
) -> luban_api::WsServerMessage {
    let next = tokio::time::timeout(timeout, socket.next())
        .await
        .expect("timed out waiting for ws message")
        .expect("websocket stream ended")
        .expect("websocket recv failed");
    let Message::Text(text) = next else {
        panic!("expected text ws message");
    };
    serde_json::from_str(&text).expect("failed to parse ws server message")
}

fn run_git(dir: &PathBuf, args: &[&str]) {
    let status = std::process::Command::new("git")
        .args(args)
        .current_dir(dir)
        .status()
        .expect("spawn git");
    assert!(status.success(), "git command failed: {args:?}");
}

fn create_git_project() -> PathBuf {
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let dir = std::env::temp_dir().join(format!(
        "luban-contracts-http-project-{}-{}",
        std::process::id(),
        unique
    ));
    std::fs::create_dir_all(&dir).expect("create temp project dir");

    run_git(&dir, &["init"]);
    run_git(&dir, &["config", "user.email", "contracts@example.com"]);
    run_git(&dir, &["config", "user.name", "luban-contracts"]);
    run_git(&dir, &["checkout", "-b", "main"]);
    std::fs::write(dir.join("README.md"), "contracts http test\n").expect("write README.md");
    run_git(&dir, &["add", "."]);
    run_git(&dir, &["commit", "-m", "init"]);

    dir
}

fn create_git_project_with_github_remote(owner: &str, repo: &str) -> PathBuf {
    let dir = create_git_project();
    let remote = format!("https://github.com/{owner}/{repo}.git");
    run_git(&dir, &["remote", "add", "origin", &remote]);
    dir
}

struct StartedTestServer {
    addr: SocketAddr,
    handle: Option<tokio::task::JoinHandle<()>>,
}

impl Drop for StartedTestServer {
    fn drop(&mut self) {
        if let Some(handle) = self.handle.take() {
            handle.abort();
        }
    }
}

async fn start_avatar_upstream(counter: Arc<AtomicUsize>) -> StartedTestServer {
    const PNG_1X1: &[u8] = &[
        0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a, 0x00, 0x00, 0x00, 0x0d, 0x49, 0x48, 0x44,
        0x52, 0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x01, 0x08, 0x06, 0x00, 0x00, 0x00, 0x1f,
        0x15, 0xc4, 0x89, 0x00, 0x00, 0x00, 0x0a, 0x49, 0x44, 0x41, 0x54, 0x78, 0x9c, 0x63, 0x60,
        0x00, 0x00, 0x00, 0x02, 0x00, 0x01, 0xe2, 0x21, 0xbc, 0x33, 0x00, 0x00, 0x00, 0x00, 0x49,
        0x45, 0x4e, 0x44, 0xae, 0x42, 0x60, 0x82,
    ];

    #[derive(Clone)]
    struct AppState {
        counter: Arc<AtomicUsize>,
    }

    async fn avatar(
        axum::extract::State(state): axum::extract::State<AppState>,
        axum::extract::Path(_owner): axum::extract::Path<String>,
    ) -> impl axum::response::IntoResponse {
        state.counter.fetch_add(1, Ordering::SeqCst);
        (
            [
                (axum::http::header::CONTENT_TYPE, "image/png"),
                (axum::http::header::CACHE_CONTROL, "no-store"),
            ],
            PNG_1X1.to_vec(),
        )
    }

    let app = axum::Router::new()
        .route("/{owner}", axum::routing::get(avatar))
        .with_state(AppState { counter });

    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind avatar upstream");
    let addr = listener.local_addr().expect("avatar local addr");
    let handle = tokio::spawn(async move {
        axum::serve(listener, app).await.expect("avatar serve");
    });

    StartedTestServer {
        addr,
        handle: Some(handle),
    }
}

async fn create_workdir_via_ws(server_addr: SocketAddr, project_path: &str) -> (u64, String) {
    let url = format!("ws://{}/api/events", server_addr);
    let (mut socket, _) = tokio_tungstenite::connect_async(url)
        .await
        .expect("connect websocket");

    let first = recv_ws_msg(&mut socket, Duration::from_secs(2)).await;
    assert!(matches!(first, luban_api::WsServerMessage::Hello { .. }));

    let hello = luban_api::WsClientMessage::Hello {
        protocol_version: luban_api::PROTOCOL_VERSION,
        last_seen_rev: None,
    };
    socket
        .send(Message::Text(
            serde_json::to_string(&hello)
                .expect("serialize hello")
                .into(),
        ))
        .await
        .expect("send hello");

    let mut saw_resync = false;
    for _ in 0..20 {
        let msg = recv_ws_msg(&mut socket, Duration::from_secs(2)).await;
        if let luban_api::WsServerMessage::Event { event, .. } = msg
            && matches!(*event, luban_api::ServerEvent::AppChanged { .. })
        {
            saw_resync = true;
            break;
        }
    }
    assert!(
        saw_resync,
        "expected an AppChanged resync event after hello"
    );

    let action = luban_api::WsClientMessage::Action {
        request_id: "req-add-project-and-open".to_owned(),
        action: Box::new(luban_api::ClientAction::AddProjectAndOpen {
            path: project_path.to_owned(),
        }),
    };
    socket
        .send(Message::Text(
            serde_json::to_string(&action)
                .expect("serialize add_project_and_open action")
                .into(),
        ))
        .await
        .expect("send add_project_and_open action");

    let mut saw_ack = false;
    let mut out: Option<(u64, String)> = None;
    for _ in 0..120 {
        let msg = recv_ws_msg(&mut socket, Duration::from_secs(2)).await;
        match msg {
            luban_api::WsServerMessage::Ack { request_id, .. } => {
                if request_id == "req-add-project-and-open" {
                    saw_ack = true;
                }
            }
            luban_api::WsServerMessage::Event { event, .. } => {
                if let luban_api::ServerEvent::AddProjectAndOpenReady {
                    request_id,
                    project_id,
                    workspace_id,
                } = *event
                    && request_id == "req-add-project-and-open"
                {
                    out = Some((workspace_id.0, project_id.0));
                }
            }
            _ => {}
        }

        if saw_ack && out.is_some() {
            break;
        }
    }

    assert!(saw_ack, "expected ack for add_project_and_open");
    out.expect("expected AddProjectAndOpenReady")
}

async fn ensure_task_via_ws(server_addr: SocketAddr, workdir_id: u64) {
    let url = format!("ws://{}/api/events", server_addr);
    let (mut socket, _) = tokio_tungstenite::connect_async(url)
        .await
        .expect("connect websocket");

    let first = recv_ws_msg(&mut socket, Duration::from_secs(2)).await;
    assert!(matches!(first, luban_api::WsServerMessage::Hello { .. }));

    let hello = luban_api::WsClientMessage::Hello {
        protocol_version: luban_api::PROTOCOL_VERSION,
        last_seen_rev: None,
    };
    socket
        .send(Message::Text(
            serde_json::to_string(&hello)
                .expect("serialize hello")
                .into(),
        ))
        .await
        .expect("send hello");

    let mut saw_resync = false;
    for _ in 0..20 {
        let msg = recv_ws_msg(&mut socket, Duration::from_secs(2)).await;
        if let luban_api::WsServerMessage::Event { event, .. } = msg
            && matches!(*event, luban_api::ServerEvent::AppChanged { .. })
        {
            saw_resync = true;
            break;
        }
    }
    assert!(
        saw_resync,
        "expected an AppChanged resync event after hello"
    );

    let action = luban_api::WsClientMessage::Action {
        request_id: "req-create-task".to_owned(),
        action: Box::new(luban_api::ClientAction::CreateWorkspaceThread {
            workspace_id: luban_api::WorkspaceId(workdir_id),
            model_id: None,
            thinking_effort: None,
        }),
    };
    socket
        .send(Message::Text(
            serde_json::to_string(&action)
                .expect("serialize create_task action")
                .into(),
        ))
        .await
        .expect("send create_task action");

    let mut saw_ack = false;
    for _ in 0..60 {
        let msg = recv_ws_msg(&mut socket, Duration::from_secs(2)).await;
        match msg {
            luban_api::WsServerMessage::Ack { request_id, .. } => {
                if request_id == "req-create-task" {
                    saw_ack = true;
                    break;
                }
            }
            luban_api::WsServerMessage::Error { message, .. } => {
                panic!("create_task error: {message}");
            }
            _ => {}
        }
    }
    assert!(saw_ack, "expected ack for create_task");
}

async fn set_task_star_via_ws(
    server_addr: SocketAddr,
    workdir_id: u64,
    task_id: u64,
    starred: bool,
) {
    let url = format!("ws://{}/api/events", server_addr);
    let (mut socket, _) = tokio_tungstenite::connect_async(url)
        .await
        .expect("connect websocket");

    let first = recv_ws_msg(&mut socket, Duration::from_secs(2)).await;
    assert!(matches!(first, luban_api::WsServerMessage::Hello { .. }));

    let hello = luban_api::WsClientMessage::Hello {
        protocol_version: luban_api::PROTOCOL_VERSION,
        last_seen_rev: None,
    };
    socket
        .send(Message::Text(
            serde_json::to_string(&hello)
                .expect("serialize hello")
                .into(),
        ))
        .await
        .expect("send hello");

    let mut saw_resync = false;
    for _ in 0..20 {
        let msg = recv_ws_msg(&mut socket, Duration::from_secs(2)).await;
        if let luban_api::WsServerMessage::Event { event, .. } = msg
            && matches!(*event, luban_api::ServerEvent::AppChanged { .. })
        {
            saw_resync = true;
            break;
        }
    }
    assert!(
        saw_resync,
        "expected an AppChanged resync event after hello"
    );

    let action = luban_api::WsClientMessage::Action {
        request_id: "req-task-star".to_owned(),
        action: Box::new(luban_api::ClientAction::TaskStarSet {
            workspace_id: luban_api::WorkspaceId(workdir_id),
            thread_id: luban_api::WorkspaceThreadId(task_id),
            starred,
        }),
    };
    socket
        .send(Message::Text(
            serde_json::to_string(&action)
                .expect("serialize task_star_set action")
                .into(),
        ))
        .await
        .expect("send task_star_set action");

    let mut saw_ack = false;
    let mut saw_summaries_changed = false;
    for _ in 0..60 {
        let msg = recv_ws_msg(&mut socket, Duration::from_secs(2)).await;
        match msg {
            luban_api::WsServerMessage::Ack { request_id, .. } => {
                if request_id == "req-task-star" {
                    saw_ack = true;
                }
            }
            luban_api::WsServerMessage::Event { event, .. } => {
                if let luban_api::ServerEvent::TaskSummariesChanged {
                    workspace_id,
                    tasks,
                    ..
                } = *event
                {
                    if workspace_id.0 != workdir_id {
                        continue;
                    }
                    if let Some(task) = tasks.iter().find(|t| t.thread_id.0 == task_id)
                        && task.is_starred == starred
                    {
                        saw_summaries_changed = true;
                    }
                }
            }
            luban_api::WsServerMessage::Error { message, .. } => {
                panic!("task_star_set error: {message}");
            }
            _ => {}
        }
        if saw_ack && saw_summaries_changed {
            break;
        }
    }
    assert!(saw_ack, "expected ack for task_star_set");
    assert!(
        saw_summaries_changed,
        "expected a TaskSummariesChanged event for task_star_set"
    );
}

async fn set_task_status_via_ws(
    server_addr: SocketAddr,
    workdir_id: u64,
    task_id: u64,
    task_status: luban_api::TaskStatus,
) {
    let url = format!("ws://{}/api/events", server_addr);
    let (mut socket, _) = tokio_tungstenite::connect_async(url)
        .await
        .expect("connect websocket");

    let first = recv_ws_msg(&mut socket, Duration::from_secs(2)).await;
    assert!(matches!(first, luban_api::WsServerMessage::Hello { .. }));

    let hello = luban_api::WsClientMessage::Hello {
        protocol_version: luban_api::PROTOCOL_VERSION,
        last_seen_rev: None,
    };
    socket
        .send(Message::Text(
            serde_json::to_string(&hello)
                .expect("serialize hello")
                .into(),
        ))
        .await
        .expect("send hello");

    let mut saw_resync = false;
    for _ in 0..20 {
        let msg = recv_ws_msg(&mut socket, Duration::from_secs(2)).await;
        if let luban_api::WsServerMessage::Event { event, .. } = msg
            && matches!(*event, luban_api::ServerEvent::AppChanged { .. })
        {
            saw_resync = true;
            break;
        }
    }
    assert!(
        saw_resync,
        "expected an AppChanged resync event after hello"
    );

    let action = luban_api::WsClientMessage::Action {
        request_id: "req-task-status".to_owned(),
        action: Box::new(luban_api::ClientAction::TaskStatusSet {
            workspace_id: luban_api::WorkspaceId(workdir_id),
            thread_id: luban_api::WorkspaceThreadId(task_id),
            task_status,
        }),
    };
    socket
        .send(Message::Text(
            serde_json::to_string(&action)
                .expect("serialize task_status_set action")
                .into(),
        ))
        .await
        .expect("send task_status_set action");

    let mut saw_ack = false;
    for _ in 0..60 {
        let msg = recv_ws_msg(&mut socket, Duration::from_secs(2)).await;
        if matches!(
            msg,
            luban_api::WsServerMessage::Ack { ref request_id, .. }
                if request_id == "req-task-status"
        ) {
            saw_ack = true;
            break;
        }
    }
    assert!(saw_ack, "expected ack for task_status_set");
}

async fn archive_workdir_via_ws(server_addr: SocketAddr, workdir_id: u64) {
    let url = format!("ws://{}/api/events", server_addr);
    let (mut socket, _) = tokio_tungstenite::connect_async(url)
        .await
        .expect("connect websocket");

    let first = recv_ws_msg(&mut socket, Duration::from_secs(2)).await;
    assert!(matches!(first, luban_api::WsServerMessage::Hello { .. }));

    let hello = luban_api::WsClientMessage::Hello {
        protocol_version: luban_api::PROTOCOL_VERSION,
        last_seen_rev: None,
    };
    socket
        .send(Message::Text(
            serde_json::to_string(&hello)
                .expect("serialize hello")
                .into(),
        ))
        .await
        .expect("send hello");

    let mut saw_resync = false;
    for _ in 0..20 {
        let msg = recv_ws_msg(&mut socket, Duration::from_secs(2)).await;
        if let luban_api::WsServerMessage::Event { event, .. } = msg
            && matches!(*event, luban_api::ServerEvent::AppChanged { .. })
        {
            saw_resync = true;
            break;
        }
    }
    assert!(
        saw_resync,
        "expected an AppChanged resync event after hello"
    );

    let action = luban_api::WsClientMessage::Action {
        request_id: "req-archive-workdir".to_owned(),
        action: Box::new(luban_api::ClientAction::ArchiveWorkspace {
            workspace_id: luban_api::WorkspaceId(workdir_id),
        }),
    };
    socket
        .send(Message::Text(
            serde_json::to_string(&action)
                .expect("serialize archive_workdir action")
                .into(),
        ))
        .await
        .expect("send archive_workdir action");

    let mut saw_ack = false;
    for _ in 0..120 {
        let msg = recv_ws_msg(&mut socket, Duration::from_secs(2)).await;
        match msg {
            luban_api::WsServerMessage::Ack { request_id, .. } => {
                if request_id == "req-archive-workdir" {
                    saw_ack = true;
                    break;
                }
            }
            luban_api::WsServerMessage::Error { message, .. } => {
                panic!("archive_workdir error: {message}");
            }
            _ => {}
        }
    }
    assert!(saw_ack, "expected ack for archive_workdir");
}

async fn create_workdir_for_project_via_ws(server_addr: SocketAddr, project_id: &str) {
    let url = format!("ws://{}/api/events", server_addr);
    let (mut socket, _) = tokio_tungstenite::connect_async(url)
        .await
        .expect("connect websocket");

    let first = recv_ws_msg(&mut socket, Duration::from_secs(2)).await;
    assert!(matches!(first, luban_api::WsServerMessage::Hello { .. }));

    let hello = luban_api::WsClientMessage::Hello {
        protocol_version: luban_api::PROTOCOL_VERSION,
        last_seen_rev: None,
    };
    socket
        .send(Message::Text(
            serde_json::to_string(&hello)
                .expect("serialize hello")
                .into(),
        ))
        .await
        .expect("send hello");

    let mut saw_resync = false;
    for _ in 0..20 {
        let msg = recv_ws_msg(&mut socket, Duration::from_secs(2)).await;
        if let luban_api::WsServerMessage::Event { event, .. } = msg
            && matches!(*event, luban_api::ServerEvent::AppChanged { .. })
        {
            saw_resync = true;
            break;
        }
    }
    assert!(
        saw_resync,
        "expected an AppChanged resync event after hello"
    );

    let action = luban_api::WsClientMessage::Action {
        request_id: "req-create-workdir".to_owned(),
        action: Box::new(luban_api::ClientAction::CreateWorkspace {
            project_id: luban_api::ProjectId(project_id.to_owned()),
        }),
    };
    socket
        .send(Message::Text(
            serde_json::to_string(&action)
                .expect("serialize create_workdir action")
                .into(),
        ))
        .await
        .expect("send create_workdir action");

    let mut saw_ack = false;
    for _ in 0..120 {
        let msg = recv_ws_msg(&mut socket, Duration::from_secs(2)).await;
        match msg {
            luban_api::WsServerMessage::Ack { request_id, .. } => {
                if request_id == "req-create-workdir" {
                    saw_ack = true;
                    break;
                }
            }
            luban_api::WsServerMessage::Error { message, .. } => {
                panic!("create_workdir error: {message}");
            }
            _ => {}
        }
    }
    assert!(saw_ack, "expected ack for create_workdir");
}

async fn upload_text_attachment(
    client: &reqwest::Client,
    base: &str,
    workdir_id: u64,
    idempotency_key: &str,
    bytes: Vec<u8>,
) -> luban_api::AttachmentRef {
    let form = reqwest::multipart::Form::new().text("kind", "text").part(
        "file",
        reqwest::multipart::Part::bytes(bytes)
            .file_name("hello.txt")
            .mime_str("text/plain")
            .expect("mime"),
    );

    client
        .post(format!("{base}/api/workdirs/{workdir_id}/attachments"))
        .header("Idempotency-Key", idempotency_key)
        .multipart(form)
        .send()
        .await
        .expect("POST /attachments")
        .error_for_status()
        .expect("upload status")
        .json::<luban_api::AttachmentRef>()
        .await
        .expect("upload json")
}

#[tokio::test]
async fn http_contracts_smoke() {
    let env = EnvGuard::lock(vec![
        luban_domain::paths::LUBAN_CODEX_ROOT_ENV,
        luban_domain::paths::LUBAN_ROOT_ENV,
        "LUBAN_GITHUB_AVATAR_BASE_URL",
    ]);
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();

    let luban_root = std::env::temp_dir().join(format!(
        "luban-contracts-http-root-{}-{}",
        std::process::id(),
        unique
    ));
    env.set(luban_domain::paths::LUBAN_ROOT_ENV, &luban_root);

    let codex_root = std::env::temp_dir().join(format!(
        "luban-contracts-http-codex-root-{}-{}",
        std::process::id(),
        unique
    ));
    let prompts_dir = codex_root.join("prompts");
    std::fs::create_dir_all(&prompts_dir).expect("create prompts dir");
    std::fs::write(
        prompts_dir.join("review.md"),
        ["Review a change locally.", "", "- Inspect diffs.", ""].join("\n"),
    )
    .expect("write prompt");
    env.set(luban_domain::paths::LUBAN_CODEX_ROOT_ENV, &codex_root);

    let avatar_hits = Arc::new(AtomicUsize::new(0));
    let avatar_upstream = start_avatar_upstream(avatar_hits.clone()).await;
    let avatar_base_url = format!("http://{}", avatar_upstream.addr);
    env.set_str("LUBAN_GITHUB_AVATAR_BASE_URL", &avatar_base_url);

    let addr: SocketAddr = "127.0.0.1:0".parse().unwrap();
    let server =
        luban_server::start_server_with_config(addr, luban_server::ServerConfig::default())
            .await
            .unwrap();

    let base = format!("http://{}", server.addr);
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(10))
        .build()
        .expect("reqwest client");

    // C-HTTP-HEALTH
    {
        let res = client
            .get(format!("{base}/api/health"))
            .send()
            .await
            .expect("GET /api/health");
        assert!(res.status().is_success());
        let body = res.text().await.expect("health body");
        assert_eq!(body, "ok");
    }

    // C-HTTP-APP
    {
        let res = client
            .get(format!("{base}/api/app"))
            .send()
            .await
            .expect("GET /api/app");
        assert!(res.status().is_success());
        let _snapshot: luban_api::AppSnapshot = res.json().await.expect("app snapshot json");
    }

    // C-HTTP-CODEX-PROMPTS
    {
        let res = client
            .get(format!("{base}/api/codex/prompts"))
            .send()
            .await
            .expect("GET /api/codex/prompts");
        assert!(res.status().is_success());
        let prompts: Vec<luban_api::CodexCustomPromptSnapshot> =
            res.json().await.expect("codex prompts json");
        assert!(
            !prompts.is_empty(),
            "expected codex prompts to be discovered under LUBAN_CODEX_ROOT"
        );
    }

    let project_dir = create_git_project_with_github_remote("octocat", "hello-world");
    let project_path = project_dir.to_string_lossy().to_string();
    let (workdir_id, project_id) = create_workdir_via_ws(server.addr, &project_path).await;

    // C-HTTP-PROJECTS-AVATAR
    {
        let url = reqwest::Url::parse_with_params(
            &format!("{base}/api/projects/avatar"),
            [("project_id", project_id.clone())],
        )
        .expect("avatar url");

        let res = client
            .get(url.clone())
            .send()
            .await
            .expect("GET /api/projects/avatar");
        assert_eq!(res.status(), reqwest::StatusCode::OK);
        assert_eq!(
            res.headers()
                .get(reqwest::header::CONTENT_TYPE)
                .and_then(|v| v.to_str().ok()),
            Some("image/png")
        );
        let first = res.bytes().await.expect("avatar bytes").to_vec();
        assert!(!first.is_empty(), "expected avatar bytes to be non-empty");

        let res = client
            .get(url)
            .send()
            .await
            .expect("GET /api/projects/avatar (cached)");
        assert_eq!(res.status(), reqwest::StatusCode::OK);
        let second = res.bytes().await.expect("avatar bytes").to_vec();
        assert_eq!(second, first, "expected cached avatar to match");
        assert_eq!(
            avatar_hits.load(Ordering::SeqCst),
            1,
            "expected avatar to be fetched once and then cached"
        );
    }

    // C-HTTP-WORKDIR-TASKS
    let mut threads: luban_api::ThreadsSnapshot = client
        .get(format!("{base}/api/workdirs/{workdir_id}/tasks"))
        .send()
        .await
        .expect("GET /threads")
        .error_for_status()
        .expect("threads status")
        .json()
        .await
        .expect("threads json");

    if threads.threads.is_empty() {
        ensure_task_via_ws(server.addr, workdir_id).await;
        threads = client
            .get(format!("{base}/api/workdirs/{workdir_id}/tasks"))
            .send()
            .await
            .expect("GET /tasks")
            .error_for_status()
            .expect("tasks status")
            .json()
            .await
            .expect("tasks json");
    }

    let task_id = threads.tabs.active_tab.0;
    for meta in &threads.threads {
        assert!(
            meta.created_at_unix_seconds > 0,
            "expected ThreadMeta.created_at_unix_seconds to be present"
        );
        assert!(
            meta.created_at_unix_seconds <= meta.updated_at_unix_seconds,
            "expected ThreadMeta.created_at_unix_seconds <= updated_at_unix_seconds"
        );
    }

    // C-HTTP-TASKS
    {
        let snap: luban_api::TasksSnapshot = client
            .get(format!("{base}/api/tasks"))
            .send()
            .await
            .expect("GET /api/tasks")
            .error_for_status()
            .expect("tasks status")
            .json()
            .await
            .expect("tasks json");
        assert!(
            snap.tasks.iter().any(|t| t.workspace_id.0 == workdir_id),
            "expected tasks to include the active workdir"
        );

        let active = snap
            .tasks
            .iter()
            .find(|t| t.workspace_id.0 == workdir_id && t.thread_id.0 == task_id)
            .expect("task should exist in tasks snapshot");
        assert!(
            active.created_at_unix_seconds > 0,
            "expected TaskSummarySnapshot.created_at_unix_seconds to be present"
        );
        assert!(
            active.created_at_unix_seconds <= active.updated_at_unix_seconds,
            "expected TaskSummarySnapshot.created_at_unix_seconds <= updated_at_unix_seconds"
        );
        assert!(
            !active.is_starred,
            "expected tasks to be unstarred by default"
        );
    }

    // C-HTTP-NEW-TASK-DRAFTS / C-HTTP-NEW-TASK-DRAFT
    {
        let snap: luban_api::NewTaskDraftsSnapshot = client
            .get(format!("{base}/api/new_task/drafts"))
            .send()
            .await
            .expect("GET /api/new_task/drafts")
            .error_for_status()
            .expect("drafts status")
            .json()
            .await
            .expect("drafts json");
        assert!(snap.drafts.is_empty(), "expected drafts to start empty");

        #[derive(serde::Serialize)]
        struct UpsertDraft<'a> {
            text: &'a str,
            project_id: &'a str,
            workdir_id: u64,
        }

        let created: luban_api::NewTaskDraftSnapshot = client
            .post(format!("{base}/api/new_task/drafts"))
            .json(&UpsertDraft {
                text: "draft-1",
                project_id: &project_id,
                workdir_id,
            })
            .send()
            .await
            .expect("POST /api/new_task/drafts")
            .error_for_status()
            .expect("create draft status")
            .json()
            .await
            .expect("create draft json");

        assert_eq!(created.text, "draft-1");
        assert_eq!(
            created.project_id,
            Some(luban_api::ProjectId(project_id.clone()))
        );
        assert_eq!(
            created.workspace_id,
            Some(luban_api::WorkspaceId(workdir_id))
        );
        assert!(created.created_at_unix_ms > 0);
        assert!(created.updated_at_unix_ms >= created.created_at_unix_ms);

        let updated: luban_api::NewTaskDraftSnapshot = client
            .put(format!("{base}/api/new_task/drafts/{}", created.id))
            .json(&UpsertDraft {
                text: "draft-2",
                project_id: &project_id,
                workdir_id,
            })
            .send()
            .await
            .expect("PUT /api/new_task/drafts/{id}")
            .error_for_status()
            .expect("update draft status")
            .json()
            .await
            .expect("update draft json");
        assert_eq!(updated.id, created.id);
        assert_eq!(updated.text, "draft-2");
        assert!(updated.updated_at_unix_ms >= created.updated_at_unix_ms);

        let res = client
            .delete(format!("{base}/api/new_task/drafts/{}", created.id))
            .send()
            .await
            .expect("DELETE /api/new_task/drafts/{id}");
        assert_eq!(res.status(), reqwest::StatusCode::NO_CONTENT);

        let snap: luban_api::NewTaskDraftsSnapshot = client
            .get(format!("{base}/api/new_task/drafts"))
            .send()
            .await
            .expect("GET /api/new_task/drafts (after delete)")
            .error_for_status()
            .expect("drafts status")
            .json()
            .await
            .expect("drafts json");
        assert!(
            snap.drafts.iter().all(|d| d.id != updated.id),
            "expected deleted draft to be removed"
        );
    }

    // C-HTTP-NEW-TASK-STASH
    {
        let initial: luban_api::NewTaskStashResponse = client
            .get(format!("{base}/api/new_task/stash"))
            .send()
            .await
            .expect("GET /api/new_task/stash")
            .error_for_status()
            .expect("stash status")
            .json()
            .await
            .expect("stash json");
        assert!(initial.stash.is_none(), "expected stash to start empty");

        #[derive(serde::Serialize)]
        struct PutStash<'a> {
            text: &'a str,
            project_id: &'a str,
            workdir_id: u64,
            editing_draft_id: &'a str,
        }

        let res = client
            .put(format!("{base}/api/new_task/stash"))
            .json(&PutStash {
                text: "stash-1",
                project_id: &project_id,
                workdir_id,
                editing_draft_id: "draft-id",
            })
            .send()
            .await
            .expect("PUT /api/new_task/stash");
        assert_eq!(res.status(), reqwest::StatusCode::NO_CONTENT);

        let loaded: luban_api::NewTaskStashResponse = client
            .get(format!("{base}/api/new_task/stash"))
            .send()
            .await
            .expect("GET /api/new_task/stash (after put)")
            .error_for_status()
            .expect("stash status")
            .json()
            .await
            .expect("stash json");
        let stash = loaded.stash.expect("stash should exist");
        assert_eq!(stash.text, "stash-1");
        assert_eq!(
            stash.project_id,
            Some(luban_api::ProjectId(project_id.clone()))
        );
        assert_eq!(stash.workspace_id, Some(luban_api::WorkspaceId(workdir_id)));
        assert_eq!(stash.editing_draft_id.as_deref(), Some("draft-id"));
        assert!(stash.updated_at_unix_ms > 0);

        let res = client
            .delete(format!("{base}/api/new_task/stash"))
            .send()
            .await
            .expect("DELETE /api/new_task/stash");
        assert_eq!(res.status(), reqwest::StatusCode::NO_CONTENT);

        let cleared: luban_api::NewTaskStashResponse = client
            .get(format!("{base}/api/new_task/stash"))
            .send()
            .await
            .expect("GET /api/new_task/stash (after delete)")
            .error_for_status()
            .expect("stash status")
            .json()
            .await
            .expect("stash json");
        assert!(cleared.stash.is_none(), "expected stash to be cleared");
    }

    set_task_star_via_ws(server.addr, workdir_id, task_id, true).await;

    // C-HTTP-TASKS (starred)
    {
        let snap: luban_api::TasksSnapshot = client
            .get(format!("{base}/api/tasks"))
            .send()
            .await
            .expect("GET /api/tasks")
            .error_for_status()
            .expect("tasks status")
            .json()
            .await
            .expect("tasks json");

        let active = snap
            .tasks
            .iter()
            .find(|t| t.workspace_id.0 == workdir_id && t.thread_id.0 == task_id)
            .expect("task should exist in tasks snapshot");
        assert!(active.is_starred, "expected task to be starred");
    }

    set_task_status_via_ws(
        server.addr,
        workdir_id,
        task_id,
        luban_api::TaskStatus::Iterating,
    )
    .await;

    // C-HTTP-TASKS (task_status)
    {
        let mut saw_updated = false;
        for _ in 0..20 {
            let snap: luban_api::TasksSnapshot = client
                .get(format!("{base}/api/tasks"))
                .send()
                .await
                .expect("GET /api/tasks")
                .error_for_status()
                .expect("tasks status")
                .json()
                .await
                .expect("tasks json");

            let active = snap
                .tasks
                .iter()
                .find(|t| t.workspace_id.0 == workdir_id && t.thread_id.0 == task_id)
                .expect("task should exist in tasks snapshot");

            if active.task_status == luban_api::TaskStatus::Iterating {
                saw_updated = true;
                break;
            }
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
        assert!(saw_updated, "expected task_status to be updated");
    }

    // C-HTTP-TASKS (task_status query)
    {
        ensure_task_via_ws(server.addr, workdir_id).await;

        let mut backlog_task_id: Option<u64> = None;
        for _ in 0..60 {
            let threads: luban_api::ThreadsSnapshot = client
                .get(format!("{base}/api/workdirs/{workdir_id}/tasks"))
                .send()
                .await
                .expect("GET /threads (backlog task poll)")
                .error_for_status()
                .expect("threads status (backlog poll)")
                .json()
                .await
                .expect("threads json (backlog poll)");

            if let Some(other) = threads
                .threads
                .iter()
                .find(|t| t.thread_id.0 != task_id)
                .map(|t| t.thread_id.0)
            {
                backlog_task_id = Some(other);
                break;
            }
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
        let backlog_task_id = backlog_task_id.expect("expected a second task to exist");

        set_task_status_via_ws(
            server.addr,
            workdir_id,
            backlog_task_id,
            luban_api::TaskStatus::Backlog,
        )
        .await;

        let url_backlog = reqwest::Url::parse_with_params(
            &format!("{base}/api/tasks"),
            [
                ("project_id", project_id.clone()),
                ("task_status", "backlog".to_owned()),
            ],
        )
        .expect("tasks url (task_status=backlog)");
        let snap_backlog: luban_api::TasksSnapshot = client
            .get(url_backlog)
            .send()
            .await
            .expect("GET /api/tasks (task_status=backlog)")
            .error_for_status()
            .expect("tasks status (task_status=backlog)")
            .json()
            .await
            .expect("tasks json (task_status=backlog)");
        assert!(
            snap_backlog
                .tasks
                .iter()
                .all(|t| t.task_status == luban_api::TaskStatus::Backlog),
            "expected task_status=backlog to filter results"
        );
        assert!(
            snap_backlog
                .tasks
                .iter()
                .any(|t| t.workspace_id.0 == workdir_id && t.thread_id.0 == backlog_task_id),
            "expected backlog task to be present in filtered results"
        );

        let url_iterating = reqwest::Url::parse_with_params(
            &format!("{base}/api/tasks"),
            [
                ("project_id", project_id.clone()),
                ("task_status", "iterating".to_owned()),
            ],
        )
        .expect("tasks url (task_status=iterating)");
        let snap_iterating: luban_api::TasksSnapshot = client
            .get(url_iterating)
            .send()
            .await
            .expect("GET /api/tasks (task_status=iterating)")
            .error_for_status()
            .expect("tasks status (task_status=iterating)")
            .json()
            .await
            .expect("tasks json (task_status=iterating)");
        assert!(
            snap_iterating
                .tasks
                .iter()
                .all(|t| t.task_status == luban_api::TaskStatus::Iterating),
            "expected task_status=iterating to filter results"
        );
        assert!(
            snap_iterating
                .tasks
                .iter()
                .any(|t| t.workspace_id.0 == workdir_id && t.thread_id.0 == task_id),
            "expected iterating task to be present in filtered results"
        );
    }

    // C-HTTP-CONVERSATION (pagination)
    {
        let convo: luban_api::ConversationSnapshot = client
            .get(format!(
                "{base}/api/workdirs/{workdir_id}/conversations/{task_id}?limit=10"
            ))
            .send()
            .await
            .expect("GET /conversation")
            .error_for_status()
            .expect("conversation status")
            .json()
            .await
            .expect("conversation json");

        assert_eq!(convo.workspace_id.0, workdir_id);
        assert_eq!(convo.thread_id.0, task_id);
        assert!(
            convo.entries.len() <= 10,
            "expected limit=10 to clamp entries"
        );
        assert!(
            convo.entries_total >= convo.entries.len() as u64,
            "expected entries_total to be consistent with page size"
        );
        assert!(convo.entries_start <= convo.entries_total);
        assert!(
            convo
                .entries
                .iter()
                .any(|entry| matches!(entry, luban_api::ConversationEntry::SystemEvent(_))),
            "expected conversation to include system events"
        );
        for entry in &convo.entries {
            match entry {
                luban_api::ConversationEntry::SystemEvent(ev) => {
                    assert!(ev.created_at_unix_ms > 0);
                }
                luban_api::ConversationEntry::UserEvent(ev) => {
                    assert!(ev.created_at_unix_ms > 0);
                }
                luban_api::ConversationEntry::AgentEvent(ev) => {
                    assert!(ev.created_at_unix_ms > 0);
                }
            }
        }
    }

    // C-HTTP-CHANGES / C-HTTP-DIFF
    {
        let _changes: luban_api::WorkspaceChangesSnapshot = client
            .get(format!("{base}/api/workdirs/{workdir_id}/changes"))
            .send()
            .await
            .expect("GET /changes")
            .error_for_status()
            .expect("changes status")
            .json()
            .await
            .expect("changes json");

        let _diff: luban_api::WorkspaceDiffSnapshot = client
            .get(format!("{base}/api/workdirs/{workdir_id}/diff"))
            .send()
            .await
            .expect("GET /diff")
            .error_for_status()
            .expect("diff status")
            .json()
            .await
            .expect("diff json");
    }

    // C-HTTP-MENTIONS
    {
        let res = client
            .get(format!("{base}/api/workdirs/{workdir_id}/mentions?q=read"))
            .send()
            .await
            .expect("GET /mentions")
            .error_for_status()
            .expect("mentions status");
        let _items: Vec<luban_api::MentionItemSnapshot> = res.json().await.expect("mentions json");
    }

    // C-HTTP-ATTACHMENTS-UPLOAD / C-HTTP-ATTACHMENTS-DOWNLOAD / C-HTTP-CONTEXT / C-HTTP-CONTEXT-DELETE
    {
        let bytes = b"hello contracts\n".to_vec();
        let idempotency_key = format!(
            "contracts-http-upload-{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_nanos()
        );

        let att1 =
            upload_text_attachment(&client, &base, workdir_id, &idempotency_key, bytes.clone())
                .await;
        let att2 =
            upload_text_attachment(&client, &base, workdir_id, &idempotency_key, bytes.clone())
                .await;
        assert_eq!(
            att1.id, att2.id,
            "expected Idempotency-Key to dedupe attachment upload"
        );

        let res = client
            .get(format!(
                "{base}/api/workdirs/{workdir_id}/attachments/{}?ext={}",
                att1.id, att1.extension
            ))
            .send()
            .await
            .expect("GET /attachments/{id}")
            .error_for_status()
            .expect("download status");
        let downloaded = res.bytes().await.expect("download bytes").to_vec();
        assert_eq!(downloaded, bytes);

        let ctx: luban_api::ContextSnapshot = client
            .get(format!("{base}/api/workdirs/{workdir_id}/context"))
            .send()
            .await
            .expect("GET /context")
            .error_for_status()
            .expect("context status")
            .json()
            .await
            .expect("context json");
        let item = ctx
            .items
            .iter()
            .find(|i| i.attachment.id == att1.id)
            .expect("expected uploaded attachment to be present in context items");

        let del = client
            .delete(format!(
                "{base}/api/workdirs/{workdir_id}/context/{}",
                item.context_id
            ))
            .send()
            .await
            .expect("DELETE /context/{id}");
        assert_eq!(del.status(), reqwest::StatusCode::NO_CONTENT);

        let ctx2: luban_api::ContextSnapshot = client
            .get(format!("{base}/api/workdirs/{workdir_id}/context"))
            .send()
            .await
            .expect("GET /context")
            .error_for_status()
            .expect("context status")
            .json()
            .await
            .expect("context json");
        assert!(
            ctx2.items.iter().all(|i| i.context_id != item.context_id),
            "expected deleted context item to be removed"
        );
    }

    // C-HTTP-TASKS (workdir_status query)
    {
        // The project used earlier in this test has a GitHub `origin`, and workspace creation
        // (`create_workdir`) fetches `origin/main`. To keep this contract test independent from
        // network access, create a second git project whose `origin` points at a local upstream.
        let upstream_dir = create_git_project();
        let archive_project_dir = create_git_project();
        let origin = upstream_dir.to_string_lossy().to_string();
        run_git(&archive_project_dir, &["remote", "add", "origin", &origin]);

        let archive_project_path = archive_project_dir.to_string_lossy().to_string();
        let (_main_workdir_id, archive_project_id) =
            create_workdir_via_ws(server.addr, &archive_project_path).await;

        create_workdir_for_project_via_ws(server.addr, &archive_project_id).await;

        let mut workdir_to_archive: Option<u64> = None;
        for _ in 0..600 {
            let app: luban_api::AppSnapshot = client
                .get(format!("{base}/api/app"))
                .send()
                .await
                .expect("GET /api/app (create workdir poll)")
                .error_for_status()
                .expect("app status")
                .json()
                .await
                .expect("app json");

            let project = app
                .projects
                .iter()
                .find(|p| p.id.0 == archive_project_id)
                .expect("expected archive project to exist in app snapshot");

            workdir_to_archive = project
                .workspaces
                .iter()
                .find(|w| w.workspace_name != "main")
                .map(|w| w.id.0);
            if workdir_to_archive.is_some() {
                break;
            }
            tokio::time::sleep(Duration::from_millis(200)).await;
        }

        let workdir_to_archive =
            workdir_to_archive.expect("expected create_workdir to create a non-main workdir");

        let threads: luban_api::ThreadsSnapshot = client
            .get(format!("{base}/api/workdirs/{workdir_to_archive}/tasks"))
            .send()
            .await
            .expect("GET /threads (archive target)")
            .error_for_status()
            .expect("threads status")
            .json()
            .await
            .expect("threads json");
        if threads.threads.is_empty() {
            ensure_task_via_ws(server.addr, workdir_to_archive).await;
        }

        archive_workdir_via_ws(server.addr, workdir_to_archive).await;

        let mut saw_archived = false;
        for _ in 0..300 {
            let app: luban_api::AppSnapshot = client
                .get(format!("{base}/api/app"))
                .send()
                .await
                .expect("GET /api/app (archive poll)")
                .error_for_status()
                .expect("app status")
                .json()
                .await
                .expect("app json");

            let status = app.projects.iter().find_map(|p| {
                p.workspaces
                    .iter()
                    .find(|w| w.id.0 == workdir_to_archive)
                    .map(|w| w.status)
            });
            if status == Some(luban_api::WorkspaceStatus::Archived) {
                saw_archived = true;
                break;
            }
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
        assert!(
            saw_archived,
            "expected workdir to transition to archived after archive_workdir"
        );

        let url_active = reqwest::Url::parse_with_params(
            &format!("{base}/api/tasks"),
            [("project_id", archive_project_id.clone())],
        )
        .expect("tasks url (active)");
        let snap_active: luban_api::TasksSnapshot = client
            .get(url_active)
            .send()
            .await
            .expect("GET /api/tasks (after archive, default active)")
            .error_for_status()
            .expect("tasks status (active)")
            .json()
            .await
            .expect("tasks json (active)");
        assert!(
            snap_active
                .tasks
                .iter()
                .all(|t| t.workspace_id.0 != workdir_to_archive),
            "expected default /api/tasks to exclude archived workdirs"
        );

        let url_all = reqwest::Url::parse_with_params(
            &format!("{base}/api/tasks"),
            [
                ("project_id", archive_project_id.clone()),
                ("workdir_status", "all".to_owned()),
            ],
        )
        .expect("tasks url (all)");
        let snap_all: luban_api::TasksSnapshot = client
            .get(url_all)
            .send()
            .await
            .expect("GET /api/tasks (after archive, workdir_status=all)")
            .error_for_status()
            .expect("tasks status (all)")
            .json()
            .await
            .expect("tasks json (all)");
        assert!(
            snap_all
                .tasks
                .iter()
                .any(|t| t.workspace_id.0 == workdir_to_archive),
            "expected workdir_status=all to include archived workdirs"
        );

        let _ = std::fs::remove_dir_all(&upstream_dir);
        let _ = std::fs::remove_dir_all(&archive_project_dir);
    }

    let _ = std::fs::remove_dir_all(&project_dir);
    let _ = std::fs::remove_dir_all(&codex_root);
}
