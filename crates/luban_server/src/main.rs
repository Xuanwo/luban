use anyhow::Context as _;
use std::net::SocketAddr;
use tracing_subscriber::EnvFilter;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    if let Err(err) = luban_server::shell_env::apply_runtime_shell_env_defaults() {
        eprintln!("warning: failed to apply shell environment defaults: {err:#}");
    }

    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env())
        .init();

    let addr: SocketAddr = std::env::var("LUBAN_SERVER_ADDR")
        .unwrap_or_else(|_| "127.0.0.1:8421".to_owned())
        .parse()
        .context("invalid LUBAN_SERVER_ADDR")?;

    let server = luban_server::start_server(addr).await?;
    tracing::info!(addr = %server.addr, "luban_server listening");
    server.wait().await?;
    Ok(())
}
