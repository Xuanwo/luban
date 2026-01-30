use futures::{SinkExt as _, StreamExt as _};
use std::net::SocketAddr;
use std::path::PathBuf;
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

    fn run_git(dir: &PathBuf, args: &[&str]) {
        let status = std::process::Command::new("git")
            .args(args)
            .current_dir(dir)
            .status()
            .expect("spawn git");
        assert!(status.success(), "git command failed: {args:?}");
    }

    run_git(&dir, &["init"]);
    run_git(&dir, &["config", "user.email", "contracts@example.com"]);
    run_git(&dir, &["config", "user.name", "luban-contracts"]);
    run_git(&dir, &["checkout", "-b", "main"]);
    std::fs::write(dir.join("README.md"), "contracts http test\n").expect("write README.md");
    run_git(&dir, &["add", "."]);
    run_git(&dir, &["commit", "-m", "init"]);

    dir
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
    for _ in 0..60 {
        let msg = recv_ws_msg(&mut socket, Duration::from_secs(2)).await;
        if matches!(
            msg,
            luban_api::WsServerMessage::Ack { ref request_id, .. }
                if request_id == "req-task-star"
        ) {
            saw_ack = true;
            break;
        }
    }
    assert!(saw_ack, "expected ack for task_star_set");
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
    let env = EnvGuard::lock(vec![luban_domain::paths::LUBAN_CODEX_ROOT_ENV]);
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
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

    let project_dir = create_git_project();
    let project_path = project_dir.to_string_lossy().to_string();
    let (workdir_id, _project_id) = create_workdir_via_ws(server.addr, &project_path).await;

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
            !active.is_starred,
            "expected tasks to be unstarred by default"
        );
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

    // C-HTTP-CONVERSATION (pagination)
    {
        let convo: luban_api::ConversationSnapshot = client
            .get(format!(
                "{base}/api/workdirs/{workdir_id}/conversations/{task_id}?limit=1"
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
            convo.entries.len() <= 1,
            "expected limit=1 to clamp entries"
        );
        assert!(
            convo.entries_total >= convo.entries.len() as u64,
            "expected entries_total to be consistent with page size"
        );
        assert!(convo.entries_start <= convo.entries_total);
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

    let _ = std::fs::remove_dir_all(&project_dir);
    let _ = std::fs::remove_dir_all(&codex_root);
}
