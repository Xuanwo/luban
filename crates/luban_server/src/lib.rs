use anyhow::Context as _;
use axum::Router;
use std::net::SocketAddr;

mod auth;
mod branch_watch;
pub mod engine;
mod git_changes;
mod idempotency;
mod mentions;
mod project_avatars;
pub mod pty;
pub mod server;
mod task_document_watch;
mod telegram;

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
        let mode = mode.trim();
        out.auth.mode = if mode.eq_ignore_ascii_case("single_user")
            || mode.eq_ignore_ascii_case("single-user")
            || mode.eq_ignore_ascii_case("singleuser")
        {
            AuthMode::SingleUser
        } else {
            AuthMode::Disabled
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    static ENV_LOCK: Mutex<()> = Mutex::new(());

    struct EnvGuard {
        _lock: std::sync::MutexGuard<'static, ()>,
        prev: Vec<(&'static str, Option<String>)>,
    }

    impl EnvGuard {
        fn lock(keys: Vec<&'static str>) -> Self {
            let lock = ENV_LOCK.lock().expect("env lock poisoned");

            let mut prev = Vec::with_capacity(keys.len());
            for key in keys {
                prev.push((key, std::env::var(key).ok()));
            }

            Self { _lock: lock, prev }
        }

        fn set(&self, key: &'static str, value: &str) {
            unsafe {
                std::env::set_var(key, value);
            }
        }

        fn remove(&self, key: &'static str) {
            unsafe {
                std::env::remove_var(key);
            }
        }
    }

    impl Drop for EnvGuard {
        fn drop(&mut self) {
            for (key, value) in self.prev.drain(..) {
                match value {
                    Some(value) => unsafe {
                        std::env::set_var(key, value);
                    },
                    None => unsafe {
                        std::env::remove_var(key);
                    },
                }
            }
        }
    }

    #[test]
    fn server_config_from_env_defaults_to_disabled() {
        let env = EnvGuard::lock(vec!["LUBAN_AUTH_MODE", "LUBAN_AUTH_BOOTSTRAP_TOKEN"]);
        env.remove("LUBAN_AUTH_MODE");
        env.remove("LUBAN_AUTH_BOOTSTRAP_TOKEN");

        let cfg = ServerConfig::from_env();
        assert_eq!(cfg.auth.mode, AuthMode::Disabled);
        assert_eq!(cfg.auth.bootstrap_token, None);
    }

    #[test]
    fn server_config_from_env_parses_auth_mode_single_user() {
        let env = EnvGuard::lock(vec!["LUBAN_AUTH_MODE"]);

        for value in [
            "single_user",
            "SINGLE_USER",
            " Single-User ",
            "singleuser",
            "SINGLEUSER",
        ] {
            env.set("LUBAN_AUTH_MODE", value);
            let cfg = ServerConfig::from_env();
            assert_eq!(cfg.auth.mode, AuthMode::SingleUser, "value={value:?}");
        }
    }

    #[test]
    fn server_config_from_env_trims_bootstrap_token() {
        let env = EnvGuard::lock(vec!["LUBAN_AUTH_BOOTSTRAP_TOKEN"]);

        env.set("LUBAN_AUTH_BOOTSTRAP_TOKEN", "  abc  ");
        let cfg = ServerConfig::from_env();
        assert_eq!(cfg.auth.bootstrap_token.as_deref(), Some("abc"));

        env.set("LUBAN_AUTH_BOOTSTRAP_TOKEN", "   ");
        let cfg = ServerConfig::from_env();
        assert_eq!(cfg.auth.bootstrap_token, None);
    }
}
