use futures::{SinkExt as _, StreamExt as _};
use std::net::SocketAddr;
use std::time::Duration;
use tokio_tungstenite::tungstenite::Message;

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
async fn ws_events_claude_enabled_changed_emits_app_changed() {
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
        request_id: "req-claude-disable".to_owned(),
        action: Box::new(luban_api::ClientAction::ClaudeEnabledChanged { enabled: false }),
    };
    socket
        .send(Message::Text(
            serde_json::to_string(&action)
                .expect("serialize claude enabled action")
                .into(),
        ))
        .await
        .expect("send claude enabled action");

    let mut saw_ack = false;
    let mut saw_app_changed = false;
    for _ in 0..80 {
        let msg = recv_ws_msg(&mut socket, Duration::from_secs(2)).await;
        match msg {
            luban_api::WsServerMessage::Ack { request_id, .. } => {
                if request_id == "req-claude-disable" {
                    saw_ack = true;
                }
            }
            luban_api::WsServerMessage::Event { event, .. } => {
                if let luban_api::ServerEvent::AppChanged { snapshot, .. } = *event
                    && !snapshot.agent.claude_enabled
                {
                    saw_app_changed = true;
                }
            }
            _ => {}
        }
        if saw_ack && saw_app_changed {
            break;
        }
    }

    assert!(saw_ack, "expected ack for claude enabled action");
    assert!(
        saw_app_changed,
        "expected AppChanged with agent.claude_enabled=false"
    );
}
