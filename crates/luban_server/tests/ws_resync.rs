use futures::{SinkExt as _, StreamExt as _};
use std::net::SocketAddr;
use std::time::Duration;
use tokio_tungstenite::tungstenite::Message;

#[tokio::test]
async fn ws_hello_triggers_app_changed_resync() {
    let addr: SocketAddr = "127.0.0.1:0".parse().unwrap();
    let server =
        luban_server::start_server_with_config(addr, luban_server::ServerConfig::default())
            .await
            .unwrap();

    let url = format!("ws://{}/api/events", server.addr);
    let (mut socket, _) = tokio_tungstenite::connect_async(url).await.unwrap();

    let first = tokio::time::timeout(Duration::from_secs(1), socket.next())
        .await
        .unwrap()
        .unwrap()
        .unwrap();
    let Message::Text(first_text) = first else {
        panic!("expected first message to be text");
    };
    let first_msg: luban_api::WsServerMessage = serde_json::from_str(&first_text).unwrap();
    assert!(matches!(
        first_msg,
        luban_api::WsServerMessage::Hello { .. }
    ));

    let hello = luban_api::WsClientMessage::Hello {
        protocol_version: luban_api::PROTOCOL_VERSION,
        last_seen_rev: None,
    };
    socket
        .send(Message::Text(serde_json::to_string(&hello).unwrap().into()))
        .await
        .unwrap();

    let mut saw_app_changed = false;
    for _ in 0..10 {
        let next = tokio::time::timeout(Duration::from_secs(1), socket.next())
            .await
            .unwrap()
            .unwrap()
            .unwrap();
        let Message::Text(text) = next else {
            continue;
        };
        let msg: luban_api::WsServerMessage = serde_json::from_str(&text).unwrap();
        if let luban_api::WsServerMessage::Event { event, .. } = msg
            && matches!(*event, luban_api::ServerEvent::AppChanged { .. })
        {
            saw_app_changed = true;
            break;
        }
    }
    assert!(
        saw_app_changed,
        "expected an AppChanged resync event after hello"
    );
}
