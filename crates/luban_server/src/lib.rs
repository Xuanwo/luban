use anyhow::Context as _;
use axum::Router;
use std::net::SocketAddr;

pub mod engine;
pub mod pty;
pub mod server;

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
    let app: Router = server::router().await?;

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
