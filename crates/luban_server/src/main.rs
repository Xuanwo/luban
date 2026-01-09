use anyhow::Context as _;
use axum::Router;
use std::net::SocketAddr;
use tracing_subscriber::EnvFilter;

mod engine;
mod pty;
mod server;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env())
        .init();

    let addr: SocketAddr = std::env::var("LUBAN_SERVER_ADDR")
        .unwrap_or_else(|_| "127.0.0.1:8421".to_owned())
        .parse()
        .context("invalid LUBAN_SERVER_ADDR")?;

    let app: Router = server::router().await?;

    let listener = tokio::net::TcpListener::bind(addr)
        .await
        .with_context(|| format!("failed to bind {addr}"))?;

    tracing::info!(%addr, "luban_server listening");

    axum::serve(listener, app).await.context("server failed")?;
    Ok(())
}
