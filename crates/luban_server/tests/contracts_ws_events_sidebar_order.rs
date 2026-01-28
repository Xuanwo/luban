use futures::{SinkExt as _, StreamExt as _};
use std::net::SocketAddr;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
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
async fn ws_events_sidebar_project_order_changed_emits_app_changed() {
    let addr: SocketAddr = "127.0.0.1:0".parse().unwrap();
    let server =
        luban_server::start_server_with_config(addr, luban_server::ServerConfig::default())
            .await
            .unwrap();

    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let project_a = std::env::temp_dir().join(format!(
        "luban-contracts-ws-events-sidebar-order-a-{}-{}",
        std::process::id(),
        unique
    ));
    let project_b = std::env::temp_dir().join(format!(
        "luban-contracts-ws-events-sidebar-order-b-{}-{}",
        std::process::id(),
        unique
    ));
    std::fs::create_dir_all(&project_a).expect("create temp project_a");
    std::fs::create_dir_all(&project_b).expect("create temp project_b");

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

    for (request_id, path) in [
        ("req-add-a", project_a.to_string_lossy().to_string()),
        ("req-add-b", project_b.to_string_lossy().to_string()),
    ] {
        let action = luban_api::WsClientMessage::Action {
            request_id: request_id.to_owned(),
            action: Box::new(luban_api::ClientAction::AddProject { path }),
        };
        socket
            .send(Message::Text(
                serde_json::to_string(&action)
                    .expect("serialize add_project action")
                    .into(),
            ))
            .await
            .expect("send add_project action");

        let mut saw_ack = false;
        for _ in 0..50 {
            let msg = recv_ws_msg(&mut socket, Duration::from_secs(2)).await;
            if let luban_api::WsServerMessage::Ack {
                request_id: got, ..
            } = msg
                && got == request_id
            {
                saw_ack = true;
                break;
            }
        }
        assert!(saw_ack, "expected ack for {request_id}");
    }

    let order = vec![
        luban_api::ProjectId(project_b.to_string_lossy().to_string()),
        luban_api::ProjectId(project_a.to_string_lossy().to_string()),
    ];
    let action = luban_api::WsClientMessage::Action {
        request_id: "req-order".to_owned(),
        action: Box::new(luban_api::ClientAction::SidebarProjectOrderChanged {
            project_ids: order.clone(),
        }),
    };
    socket
        .send(Message::Text(
            serde_json::to_string(&action)
                .expect("serialize sidebar order action")
                .into(),
        ))
        .await
        .expect("send sidebar order action");

    let mut saw_ack = false;
    let mut saw_app_changed = false;
    for _ in 0..80 {
        let msg = recv_ws_msg(&mut socket, Duration::from_secs(2)).await;
        match msg {
            luban_api::WsServerMessage::Ack { request_id, .. } => {
                if request_id == "req-order" {
                    saw_ack = true;
                }
            }
            luban_api::WsServerMessage::Event { event, .. } => {
                if let luban_api::ServerEvent::AppChanged { snapshot, .. } = *event
                    && snapshot.ui.sidebar_project_order == order
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
    assert!(saw_ack, "expected ack for sidebar order action");
    assert!(
        saw_app_changed,
        "expected AppChanged with sidebar_project_order"
    );

    let _ = std::fs::remove_dir_all(&project_a);
    let _ = std::fs::remove_dir_all(&project_b);
}
