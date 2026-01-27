use anyhow::Context as _;
use axum::Router;
use std::net::SocketAddr;

mod auth;
pub mod engine;
mod git_changes;
mod mentions;
pub mod pty;
pub mod server;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AuthMode {
    Disabled,
    SingleUser,
}

#[derive(Clone, Debug)]
pub struct AuthConfig {
    pub mode: AuthMode,
    pub bootstrap_token: Option<String>,
}

impl Default for AuthConfig {
    fn default() -> Self {
        Self {
            mode: AuthMode::Disabled,
            bootstrap_token: None,
        }
    }
}

#[derive(Clone, Debug, Default)]
pub struct ServerConfig {
    pub auth: AuthConfig,
}

impl ServerConfig {
    pub fn from_env() -> Self {
        let mut out = Self::default();

        let mode = std::env::var("LUBAN_AUTH_MODE").unwrap_or_default();
        out.auth.mode = match mode.trim().to_ascii_lowercase().as_str() {
            "single_user" | "single-user" | "singleuser" => AuthMode::SingleUser,
            _ => AuthMode::Disabled,
        };

        out.auth.bootstrap_token = std::env::var("LUBAN_AUTH_BOOTSTRAP_TOKEN")
            .ok()
            .map(|v| v.trim().to_owned())
            .filter(|v| !v.is_empty());

        out
    }
}

pub struct StartedServer {
    pub addr: SocketAddr,
    handle: Option<tokio::task::JoinHandle<anyhow::Result<()>>>,
}

impl StartedServer {
    pub async fn wait(self) -> anyhow::Result<()> {
        let mut this = self;
        let handle = this.handle.take().context("server task already consumed")?;

        handle
            .await
            .context("server task panicked")?
            .context("server failed")?;
        Ok(())
    }
}

impl Drop for StartedServer {
    fn drop(&mut self) {
        if let Some(handle) = self.handle.take() {
            handle.abort();
        }
    }
}

pub async fn start_server(addr: SocketAddr) -> anyhow::Result<StartedServer> {
    start_server_with_config(addr, ServerConfig::from_env()).await
}

pub async fn start_server_with_config(
    addr: SocketAddr,
    config: ServerConfig,
) -> anyhow::Result<StartedServer> {
    let app: Router = server::router(config).await?;

    let listener = tokio::net::TcpListener::bind(addr)
        .await
        .with_context(|| format!("failed to bind {addr}"))?;

    let actual = listener.local_addr().context("failed to read local addr")?;

    let handle = tokio::spawn(async move {
        axum::serve(listener, app).await.context("server failed")?;
        Ok(())
    });

    Ok(StartedServer {
        addr: actual,
        handle: Some(handle),
    })
}
