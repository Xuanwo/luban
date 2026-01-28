use axum::http::header::{CACHE_CONTROL, CONTENT_TYPE};
use axum::http::header::{COOKIE, SET_COOKIE};
use axum::http::{HeaderMap, HeaderValue, StatusCode};
use axum::middleware::Next;
use axum::response::{IntoResponse, Response};
use axum::{Router, routing::get};
use axum::{extract::Query, extract::State};
use tokio::sync::{Mutex, RwLock};

static SESSION_COOKIE_NAME: &str = "luban_session";

#[derive(Clone)]
pub(crate) struct AuthState {
    mode: crate::AuthMode,
    bootstrap_token: std::sync::Arc<Mutex<Option<String>>>,
    session_token: std::sync::Arc<RwLock<Option<String>>>,
}

impl AuthState {
    pub(crate) fn new(config: crate::AuthConfig) -> Self {
        Self {
            mode: config.mode,
            bootstrap_token: std::sync::Arc::new(Mutex::new(config.bootstrap_token)),
            session_token: std::sync::Arc::new(RwLock::new(None)),
        }
    }

    pub(crate) fn enabled(&self) -> bool {
        self.mode != crate::AuthMode::Disabled
    }

    async fn is_authorized(&self, headers: &HeaderMap) -> bool {
        if !self.enabled() {
            return true;
        }

        let Some(cookie) = headers.get(COOKIE).and_then(|h| h.to_str().ok()) else {
            return false;
        };
        let Some(found) = cookie_value(cookie, SESSION_COOKIE_NAME) else {
            return false;
        };

        let session = self.session_token.read().await;
        session.as_deref() == Some(found)
    }

    async fn consume_bootstrap_token(&self, token: &str) -> bool {
        if !self.enabled() {
            return false;
        }

        {
            let session = self.session_token.read().await;
            if session.as_deref() == Some(token) {
                return true;
            }
        }

        let mut bootstrap = self.bootstrap_token.lock().await;
        let Some(expected) = bootstrap.as_deref() else {
            drop(bootstrap);
            let session = self.session_token.read().await;
            return session.as_deref() == Some(token);
        };
        if expected != token {
            return false;
        }

        let mut session = self.session_token.write().await;
        *session = Some(token.to_owned());
        *bootstrap = None;
        true
    }
}

fn cookie_value<'a>(cookie_header: &'a str, name: &str) -> Option<&'a str> {
    for part in cookie_header.split(';') {
        let trimmed = part.trim();
        let Some((k, v)) = trimmed.split_once('=') else {
            continue;
        };
        if k.trim() == name {
            return Some(v.trim());
        }
    }
    None
}

pub(crate) async fn require_session(
    State(state): State<crate::server::AppStateHolder>,
    req: axum::http::Request<axum::body::Body>,
    next: Next,
) -> Response {
    if state.auth.is_authorized(req.headers()).await {
        return next.run(req).await;
    }
    (StatusCode::UNAUTHORIZED, "unauthorized").into_response()
}

#[derive(serde::Deserialize)]
pub(crate) struct AuthBootstrapQuery {
    pub(crate) token: String,
}

pub(crate) async fn auth_bootstrap(
    State(state): State<crate::server::AppStateHolder>,
    Query(query): Query<AuthBootstrapQuery>,
) -> impl IntoResponse {
    if !state.auth.enabled() {
        return (StatusCode::NOT_FOUND, "not found").into_response();
    }
    if !state.auth.consume_bootstrap_token(query.token.trim()).await {
        return (StatusCode::UNAUTHORIZED, "unauthorized").into_response();
    }

    let cookie = format!(
        "{name}={token}; Path=/; HttpOnly; SameSite=Lax",
        name = SESSION_COOKIE_NAME,
        token = query.token.trim(),
    );

    let body = r#"<!doctype html>
<html>
  <head>
    <meta charset="utf-8" />
    <meta name="referrer" content="no-referrer" />
    <title>Luban</title>
  </head>
  <body>
    <script>
      window.history.replaceState(null, "", "/");
      window.location.replace("/");
    </script>
  </body>
</html>
"#;

    let mut resp = (
        StatusCode::OK,
        [
            (CONTENT_TYPE, "text/html; charset=utf-8"),
            (CACHE_CONTROL, "no-store"),
        ],
        body,
    )
        .into_response();
    if let Ok(value) = HeaderValue::from_str(&cookie) {
        resp.headers_mut().append(SET_COOKIE, value);
    }
    resp
}

pub(crate) fn router() -> Router<crate::server::AppStateHolder> {
    Router::new().route("/auth", get(auth_bootstrap))
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::http::header::COOKIE;
    use axum::http::{HeaderMap, HeaderValue};
    use tokio::task::JoinSet;

    #[test]
    fn cookie_value_parses_standard_cookie_header() {
        assert_eq!(cookie_value("a=1; b=2", "a"), Some("1"));
        assert_eq!(cookie_value("a=1; b=2", "b"), Some("2"));
        assert_eq!(cookie_value("a=1; b=2", "c"), None);
    }

    #[test]
    fn cookie_value_trims_spaces() {
        assert_eq!(cookie_value("  a = 1  ;  b= 2 ", "a"), Some("1"));
        assert_eq!(cookie_value("  a = 1  ;  b= 2 ", "b"), Some("2"));
    }

    #[tokio::test]
    async fn consume_bootstrap_token_is_idempotent_for_same_token() {
        let state = AuthState::new(crate::AuthConfig {
            mode: crate::AuthMode::SingleUser,
            bootstrap_token: Some("t".to_owned()),
        });

        let mut set = JoinSet::new();
        for _ in 0..2 {
            let state = state.clone();
            set.spawn(async move { state.consume_bootstrap_token("t").await });
        }

        let mut oks = 0usize;
        while let Some(result) = set.join_next().await {
            if result.expect("task panicked") {
                oks += 1;
            }
        }
        assert_eq!(oks, 2);
    }

    #[tokio::test]
    async fn consume_bootstrap_token_rejects_wrong_token() {
        let state = AuthState::new(crate::AuthConfig {
            mode: crate::AuthMode::SingleUser,
            bootstrap_token: Some("t".to_owned()),
        });

        assert!(!state.consume_bootstrap_token("wrong").await);
        assert!(state.consume_bootstrap_token("t").await);
        assert!(!state.consume_bootstrap_token("wrong").await);
    }

    #[tokio::test]
    async fn is_authorized_accepts_session_cookie_after_bootstrap() {
        let state = AuthState::new(crate::AuthConfig {
            mode: crate::AuthMode::SingleUser,
            bootstrap_token: Some("t".to_owned()),
        });

        assert!(state.consume_bootstrap_token("t").await);

        let mut headers = HeaderMap::new();
        headers.insert(COOKIE, HeaderValue::from_static("luban_session=t"));
        assert!(state.is_authorized(&headers).await);
    }
}
