use futures::SinkExt as _;
use futures::StreamExt as _;
use tokio::time::Duration;
use tokio_tungstenite::tungstenite::Message;

async fn wait_for_output(
    ws: &mut tokio_tungstenite::WebSocketStream<
        tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>,
    >,
    needle: &[u8],
    timeout: Duration,
) -> Vec<u8> {
    let mut out = Vec::new();
    let deadline = tokio::time::sleep(timeout);
    tokio::pin!(deadline);

    loop {
        tokio::select! {
            _ = &mut deadline => {
                panic!("timed out waiting for output; captured {} bytes", out.len());
            }
            msg = ws.next() => {
                let Some(msg) = msg else {
                    panic!("websocket closed while waiting for output; captured {} bytes", out.len());
                };
                let msg = msg.expect("websocket recv failed");
                match msg {
                    Message::Binary(bytes) => {
                        out.extend_from_slice(&bytes);
                        if out.windows(needle.len()).any(|w| w == needle) {
                            return out;
                        }
                    }
                    Message::Text(_) => {}
                    Message::Close(_) => {
                        panic!("websocket closed while waiting for output; captured {} bytes", out.len());
                    }
                    _ => {}
                }
            }
        }
    }
}

#[tokio::test]
async fn ws_pty_reconnect_replays_history_for_same_token() {
    // Safety: tests run in a dedicated process in CI, and this env var is used
    // only to reduce shell-specific prompt noise for this test.
    unsafe {
        std::env::set_var("SHELL", "/bin/sh");
    }

    let addr = "127.0.0.1:0".parse().unwrap();
    let server = luban_server::start_server(addr).await.unwrap();

    let reconnect = "contracts-ws-pty-test-token";
    let url = format!("ws://{}/api/pty/0/1?reconnect={}", server.addr, reconnect);

    let (mut ws, _) = tokio_tungstenite::connect_async(url.clone())
        .await
        .expect("connect websocket");

    let marker = b"luban_ws_pty_contract_marker";
    let input = b"printf 'luban_ws_pty_contract_marker\\n'\n";
    ws.send(Message::Binary(input.to_vec().into()))
        .await
        .expect("send input");

    let _ = wait_for_output(&mut ws, marker, Duration::from_secs(5)).await;

    ws.close(None).await.ok();
    drop(ws);

    let (mut ws2, _) = tokio_tungstenite::connect_async(url)
        .await
        .expect("reconnect websocket");

    let _ = wait_for_output(&mut ws2, marker, Duration::from_secs(5)).await;
    ws2.close(None).await.ok();
}
