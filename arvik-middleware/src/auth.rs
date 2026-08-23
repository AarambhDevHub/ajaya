//! Authentication enforcement middleware.
//!
//! Provides several authentication strategies as Tower layers.
//! Returns `401 Unauthorized` with a `WWW-Authenticate` header on failure.
//!
//! # Example
//!
//! ```rust,ignore
//! use arvik_middleware::auth::RequireAuthorizationLayer;
//!
//! // Static bearer token
//! Router::new()
//!     .route("/admin", get(admin))
//!     .layer(RequireAuthorizationLayer::bearer("my-secret-token"));
//!
//! // HTTP Basic auth
//! Router::new()
//!     .layer(RequireAuthorizationLayer::basic("admin", "password"));
//!
//! // Custom async validator
//! Router::new()
//!     .layer(RequireAuthorizationLayer::custom(|req: &Request| {
//!         req.headers()
//!             .get("x-api-key")
//!             .and_then(|v| v.to_str().ok())
//!             .map(|k| k == "super-secret")
//!             .unwrap_or(false)
//!     }));
//! ```

use std::convert::Infallible;
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;
use std::task::{Context, Poll};

use arvik_core::{Body, Request, Response};
use base64::Engine;
use http::{HeaderValue, StatusCode, header::AUTHORIZATION};
use tower_layer::Layer;
use tower_service::Service;

/// Constant-time equality for secrets (tokens, passwords, CSRF tokens).
///
/// Accumulates an XOR of every byte pair, so where a mismatch occurs does not
/// change timing. Only the *length* remains observable, which is treated as
/// public information (scheme names like `Bearer ` and standard token formats
/// are not secret).
pub(crate) fn secret_eq(a: &str, b: &str) -> bool {
    let a = a.as_bytes();
    let b = b.as_bytes();
    if a.len() != b.len() {
        return false;
    }
    let mut diff = 0u8;
    for (x, y) in a.iter().zip(b.iter()) {
        diff |= x ^ y;
    }
    diff == 0
}

// ── AuthStrategy ──────────────────────────────────────────────────────────────

enum AuthStrategy {
    Bearer(String),
    Basic { username: String, password: String },
    Custom(Arc<dyn Fn(&Request) -> bool + Send + Sync + 'static>),
}

impl AuthStrategy {
    fn is_authorized(&self, req: &Request) -> bool {
        match self {
            AuthStrategy::Bearer(token) => {
                let expected = format!("Bearer {}", token);
                req.headers()
                    .get(AUTHORIZATION)
                    .and_then(|v| v.to_str().ok())
                    .map(|v| crate::auth::secret_eq(v, &expected))
                    .unwrap_or(false)
            }
            AuthStrategy::Basic { username, password } => {
                let credentials = base64::engine::general_purpose::STANDARD
                    .encode(format!("{}:{}", username, password));
                let expected = format!("Basic {}", credentials);
                req.headers()
                    .get(AUTHORIZATION)
                    .and_then(|v| v.to_str().ok())
                    .map(|v| crate::auth::secret_eq(v, &expected))
                    .unwrap_or(false)
            }
            AuthStrategy::Custom(f) => f(req),
        }
    }

    fn www_authenticate_value(&self) -> &'static str {
        match self {
            AuthStrategy::Bearer(_) | AuthStrategy::Custom(_) => r#"Bearer realm="api""#,
            AuthStrategy::Basic { .. } => r#"Basic realm="api""#,
        }
    }
}

// ── RequireAuthorizationLayer ─────────────────────────────────────────────────

/// Tower layer that enforces an authentication requirement.
#[derive(Clone)]
pub struct RequireAuthorizationLayer {
    strategy: Arc<AuthStrategy>,
}

impl RequireAuthorizationLayer {
    /// Require a static Bearer token in the `Authorization` header.
    pub fn bearer(token: impl Into<String>) -> Self {
        Self {
            strategy: Arc::new(AuthStrategy::Bearer(token.into())),
        }
    }

    /// Require HTTP Basic authentication.
    pub fn basic(username: impl Into<String>, password: impl Into<String>) -> Self {
        Self {
            strategy: Arc::new(AuthStrategy::Basic {
                username: username.into(),
                password: password.into(),
            }),
        }
    }

    /// Require a custom synchronous validation function.
    ///
    /// The function receives the request and returns `true` if authorized.
    pub fn custom<F>(validator: F) -> Self
    where
        F: Fn(&Request) -> bool + Send + Sync + 'static,
    {
        Self {
            strategy: Arc::new(AuthStrategy::Custom(Arc::new(validator))),
        }
    }
}

impl<S> Layer<S> for RequireAuthorizationLayer {
    type Service = RequireAuthorizationService<S>;

    fn layer(&self, inner: S) -> Self::Service {
        RequireAuthorizationService {
            inner,
            strategy: Arc::clone(&self.strategy),
        }
    }
}

/// Tower service produced by [`RequireAuthorizationLayer`].
#[derive(Clone)]
pub struct RequireAuthorizationService<S> {
    inner: S,
    strategy: Arc<AuthStrategy>,
}

impl<S> Service<Request> for RequireAuthorizationService<S>
where
    S: Service<Request, Response = Response, Error = Infallible> + Clone + Send + 'static,
    S::Future: Send + 'static,
{
    type Response = Response;
    type Error = Infallible;
    type Future = Pin<Box<dyn Future<Output = Result<Response, Infallible>> + Send + 'static>>;

    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.inner.poll_ready(cx)
    }

    fn call(&mut self, req: Request) -> Self::Future {
        // Evaluate auth synchronously before the async block so we only
        // borrow `self` here, not inside the future.
        let authorized = self.strategy.is_authorized(&req);
        let www_auth = self.strategy.www_authenticate_value();

        let cloned = self.inner.clone();
        let mut inner = std::mem::replace(&mut self.inner, cloned);

        Box::pin(async move {
            if !authorized {
                let response = http::Response::builder()
                    .status(StatusCode::UNAUTHORIZED)
                    .header(http::header::CONTENT_TYPE, "application/json")
                    .header("www-authenticate", HeaderValue::from_static(www_auth))
                    .body(Body::from(r#"{"error":"Unauthorized","code":401}"#))
                    .unwrap();
                return Ok(response); // type inferred from `inner.call` below
            }
            inner.call(req).await
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn secret_eq_compares_every_byte_without_short_circuit() {
        assert!(secret_eq("", ""));
        assert!(secret_eq("Bearer abc123", "Bearer abc123"));
        // Same length, single byte differs.
        assert!(!secret_eq("Bearer abc123", "Bearer abc124"));
        assert!(!secret_eq("Bearer abc123", "Xearer abc123"));
    }

    #[test]
    fn secret_eq_is_length_aware() {
        assert!(!secret_eq("abc", "abcd"));
        assert!(!secret_eq("abcd", "abc"));
    }

    #[test]
    fn bearer_strategy_uses_constant_time_compare() {
        let strategy = AuthStrategy::Bearer("s3cret-token".to_string());
        let mut req = Request::new(
            http::Request::builder()
                .uri("/")
                .header(AUTHORIZATION, "Bearer s3cret-token")
                .body(arvik_core::Body::empty())
                .unwrap(),
        );
        assert!(strategy.is_authorized(&req));

        req = Request::new(
            http::Request::builder()
                .uri("/")
                .header(AUTHORIZATION, "Bearer wrong-token")
                .body(arvik_core::Body::empty())
                .unwrap(),
        );
        assert!(!strategy.is_authorized(&req));
    }
}
