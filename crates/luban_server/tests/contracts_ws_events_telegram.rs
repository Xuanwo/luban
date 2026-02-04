use axum::Json;
use futures::{SinkExt as _, StreamExt as _};
use serde_json::json;
use std::net::SocketAddr;
use std::time::Duration;
use tokio::sync::oneshot;
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

async fn start_fake_telegram_api(username: &'static str) -> (String, oneshot::Sender<()>) {
    let app = axum::Router::new()
        .route(
            "/{bot}/getMe",
            axum::routing::get(move || async move {
                Json(json!({
                    "ok": true,
                    "result": { "username": username },
                }))
            }),
        )
        .route(
            "/{bot}/getUpdates",
            axum::routing::get(|| async move {
                Json(json!({
                    "ok": true,
                    "result": [],
                }))
            }),
        )
        .route(
            "/{bot}/sendMessage",
            axum::routing::post(|| async move {
                Json(json!({
                    "ok": true,
                    "result": { "message_id": 1 },
                }))
            }),
        )
        .route(
            "/{bot}/answerCallbackQuery",
            axum::routing::post(|| async move { Json(json!({ "ok": true, "result": true })) }),
        );

    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind fake telegram api");
    let addr = listener.local_addr().expect("get addr");
    let (shutdown_tx, shutdown_rx) = oneshot::channel::<()>();
    tokio::spawn(async move {
        let server = axum::serve(listener, app).with_graceful_shutdown(async move {
            let _ = shutdown_rx.await;
        });
        let _ = server.await;
    });

    (format!("http://{addr}"), shutdown_tx)
}

#[tokio::test]
async fn ws_events_telegram_pair_start_emits_pair_ready() {
    let (api_base, shutdown) = start_fake_telegram_api("TestBot").await;
    let env = EnvGuard::lock(vec!["LUBAN_TELEGRAM_API_BASE_URL"]);
    env.set_str("LUBAN_TELEGRAM_API_BASE_URL", &api_base);

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

    let action = luban_api::WsClientMessage::Action {
        request_id: "req-telegram-token".to_owned(),
        action: Box::new(luban_api::ClientAction::TelegramBotTokenSet {
            token: "test-token".to_owned(),
        }),
    };
    socket
        .send(Message::Text(
            serde_json::to_string(&action)
                .expect("serialize telegram token set")
                .into(),
        ))
        .await
        .expect("send telegram token set");

    let mut saw_token_ack = false;
    for _ in 0..50 {
        let msg = recv_ws_msg(&mut socket, Duration::from_secs(2)).await;
        if let luban_api::WsServerMessage::Ack { request_id, .. } = msg
            && request_id == "req-telegram-token"
        {
            saw_token_ack = true;
            break;
        }
    }
    assert!(saw_token_ack, "expected ack for TelegramBotTokenSet");

    let action = luban_api::WsClientMessage::Action {
        request_id: "req-telegram-pair".to_owned(),
        action: Box::new(luban_api::ClientAction::TelegramPairStart),
    };
    socket
        .send(Message::Text(
            serde_json::to_string(&action)
                .expect("serialize telegram pair start")
                .into(),
        ))
        .await
        .expect("send telegram pair start");

    let mut saw_ack = false;
    let mut saw_ready = false;
    for _ in 0..80 {
        let msg = recv_ws_msg(&mut socket, Duration::from_secs(2)).await;
        match msg {
            luban_api::WsServerMessage::Ack { request_id, .. } => {
                if request_id == "req-telegram-pair" {
                    saw_ack = true;
                }
            }
            luban_api::WsServerMessage::Event { event, .. } => {
                if let luban_api::ServerEvent::TelegramPairReady { request_id, url } = *event
                    && request_id == "req-telegram-pair"
                {
                    let prefix = "https://t.me/TestBot?start=";
                    assert!(url.starts_with(prefix));
                    let code = &url[prefix.len()..];
                    assert_eq!(code.len(), 32);
                    assert!(code.chars().all(|c| matches!(c, '0'..='9' | 'a'..='f')));
                    saw_ready = true;
                }
            }
            _ => {}
        }
        if saw_ack && saw_ready {
            break;
        }
    }

    assert!(saw_ack, "expected ack for TelegramPairStart");
    assert!(saw_ready, "expected TelegramPairReady event");

    let _ = shutdown.send(());
    drop(env);
}
