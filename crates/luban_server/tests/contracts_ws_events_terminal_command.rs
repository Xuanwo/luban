use base64::Engine as _;
use futures::{SinkExt as _, StreamExt as _};
use std::net::SocketAddr;
use std::path::PathBuf;
use std::time::Duration;
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

    fn set_str(&self, key: &'static str, value: &str) {
        unsafe {
            std::env::set_var(key, value);
        }
    }

    fn set_path(&self, key: &'static str, value: &PathBuf) {
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

#[tokio::test]
async fn ws_events_terminal_command_start_emits_conversation_events_with_output() {
    let env = EnvGuard::lock(vec![luban_domain::paths::LUBAN_ROOT_ENV, "SHELL"]);

    let root = std::env::temp_dir().join(format!(
        "luban-contracts-ws-terminal-command-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos()
    ));
    std::fs::create_dir_all(&root).expect("create LUBAN_ROOT");
    env.set_path(luban_domain::paths::LUBAN_ROOT_ENV, &root);
    env.set_str("SHELL", "/bin/sh");

    let addr: SocketAddr = "127.0.0.1:0".parse().unwrap();
    let server =
        luban_server::start_server_with_config(addr, luban_server::ServerConfig::default())
            .await
            .unwrap();

    let url = format!("ws://{}/api/events", server.addr);
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

    let request_id = "req-terminal-command-start".to_owned();
    let marker = "luban_terminal_command_contract_marker";
    let cmd = format!("printf '{marker}\\n'");
    let action = luban_api::WsClientMessage::Action {
        request_id: request_id.clone(),
        action: Box::new(luban_api::ClientAction::TerminalCommandStart {
            workspace_id: luban_api::WorkspaceId(0),
            thread_id: luban_api::WorkspaceThreadId(1),
            command: cmd.clone(),
        }),
    };
    socket
        .send(Message::Text(
            serde_json::to_string(&action)
                .expect("serialize action")
                .into(),
        ))
        .await
        .expect("send action");

    let mut saw_ack = false;
    let mut started: Option<luban_api::TerminalCommandStarted> = None;
    let mut finished: Option<luban_api::TerminalCommandFinished> = None;
    let mut started_created_at_unix_ms: Option<u64> = None;
    let mut finished_created_at_unix_ms: Option<u64> = None;

    for _ in 0..200 {
        let msg = recv_ws_msg(&mut socket, Duration::from_secs(5)).await;
        match msg {
            luban_api::WsServerMessage::Ack {
                request_id: rid, ..
            } => {
                if rid == request_id {
                    saw_ack = true;
                }
            }
            luban_api::WsServerMessage::Event { event, .. } => {
                let luban_api::ServerEvent::ConversationChanged { snapshot } = *event else {
                    continue;
                };
                for entry in snapshot.entries {
                    let luban_api::ConversationEntry::UserEvent(user) = entry else {
                        continue;
                    };
                    assert!(
                        user.created_at_unix_ms > 0,
                        "expected created_at_unix_ms to be present on user event entries"
                    );
                    match user.event {
                        luban_api::UserEvent::TerminalCommandStarted(ev) => {
                            if ev.command == cmd {
                                started = Some(ev);
                                started_created_at_unix_ms = Some(user.created_at_unix_ms);
                            }
                        }
                        luban_api::UserEvent::TerminalCommandFinished(ev) => {
                            if ev.command == cmd {
                                finished = Some(ev);
                                finished_created_at_unix_ms = Some(user.created_at_unix_ms);
                            }
                        }
                        _ => {}
                    }
                }
            }
            _ => {}
        }
        if saw_ack && started.is_some() && finished.is_some() {
            break;
        }
    }

    assert!(saw_ack, "expected ack for terminal command action");
    let started = started.expect("expected TerminalCommandStarted user event");
    let finished = finished.expect("expected TerminalCommandFinished user event");
    let started_created_at_unix_ms =
        started_created_at_unix_ms.expect("expected created_at_unix_ms for started entry");
    let finished_created_at_unix_ms =
        finished_created_at_unix_ms.expect("expected created_at_unix_ms for finished entry");
    assert!(
        started_created_at_unix_ms <= finished_created_at_unix_ms,
        "expected started.created_at_unix_ms <= finished.created_at_unix_ms"
    );
    assert_eq!(started.id, finished.id, "start/finish should share id");
    assert_eq!(
        started.reconnect, finished.reconnect,
        "start/finish should share reconnect token"
    );
    assert!(finished.output_byte_len > 0, "expected non-empty output");
    assert!(
        !finished.output_base64.trim().is_empty(),
        "expected base64 output payload"
    );

    let bytes = base64::engine::general_purpose::STANDARD
        .decode(finished.output_base64.as_bytes())
        .expect("decode output_base64");
    assert_eq!(
        bytes.len() as u64,
        finished.output_byte_len,
        "output_byte_len should match decoded length"
    );
    let needle = marker.as_bytes();
    assert!(
        bytes.windows(needle.len()).any(|w| w == needle),
        "expected marker in output (decoded {} bytes)",
        bytes.len()
    );
}
