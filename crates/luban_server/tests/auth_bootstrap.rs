use std::net::SocketAddr;

#[tokio::test]
async fn auth_bootstrap_sets_cookie_and_unlocks_api() {
    let token = "test_bootstrap_token".to_owned();

    let addr: SocketAddr = "127.0.0.1:0".parse().unwrap();
    let server = luban_server::start_server_with_config(
        addr,
        luban_server::ServerConfig {
            auth: luban_server::AuthConfig {
                mode: luban_server::AuthMode::SingleUser,
                bootstrap_token: Some(token.clone()),
            },
        },
    )
    .await
    .unwrap();

    let client = reqwest::Client::builder()
        .redirect(reqwest::redirect::Policy::none())
        .build()
        .unwrap();

    let health = client
        .get(format!("http://{}/api/health", server.addr))
        .send()
        .await
        .unwrap();
    assert_eq!(health.status(), reqwest::StatusCode::OK);

    let unauthorized = client
        .get(format!("http://{}/api/app", server.addr))
        .send()
        .await
        .unwrap();
    assert_eq!(unauthorized.status(), reqwest::StatusCode::UNAUTHORIZED);

    let bootstrap = client
        .get(format!("http://{}/auth?token={}", server.addr, token))
        .send()
        .await
        .unwrap();
    assert_eq!(bootstrap.status(), reqwest::StatusCode::OK);
    let set_cookie = bootstrap
        .headers()
        .get(reqwest::header::SET_COOKIE)
        .and_then(|v| v.to_str().ok())
        .unwrap_or_default()
        .to_owned();
    assert!(
        set_cookie.starts_with("luban_session="),
        "unexpected set-cookie: {set_cookie}"
    );

    let authorized = client
        .get(format!("http://{}/api/app", server.addr))
        .header(
            reqwest::header::COOKIE,
            "luban_session=test_bootstrap_token",
        )
        .send()
        .await
        .unwrap();
    assert_eq!(authorized.status(), reqwest::StatusCode::OK);
}
