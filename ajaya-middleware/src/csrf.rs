//! CSRF (Cross-Site Request Forgery) protection middleware.
//!
//! Implements the **double-submit cookie** pattern:
//!
//! 1. On the first request, a random CSRF token is generated and set as
//!    a `csrf_token` cookie (not HttpOnly so JS can read it).
//! 2. On state-changing requests (POST, PUT, PATCH, DELETE), the value of
//!    the `x-csrf-token` request header must match the `csrf_token` cookie.
//! 3. Safe methods (GET, HEAD, OPTIONS, TRACE) are never checked.
//!
//! The token is also available as `Extension<CsrfToken>` for use in handlers
//! (e.g., to inject into HTML forms).
//!
//! # Example
//!
//! ```rust,ignore
//! use ajaya_middleware::csrf::CsrfLayer;
//! use ajaya::Extension;
//!
//! // GET /form — render form with CSRF token
//! async fn render_form(Extension(csrf): Extension<CsrfToken>) -> Html<String> {
//!     Html(format!(
//!         r#"<form method="POST">
//!            <input type="hidden" name="csrf_token" value="{}">
//!            <button>Submit</button></form>"#,
//!         csrf.as_str()
//!     ))
//! }
//!
//! Router::new()
//!     .route("/form", get(render_form).post(handle_form))
//!     .layer(CsrfLayer::new());
//! ```
//!
//! # JavaScript (SPA) usage
//!
//! Read the `csrf_token` cookie in JavaScript and send it as the
//! `x-csrf-token` header:
//!
//! ```javascript
//! const token = document.cookie.match(/csrf_token=([^;]+)/)?.[1];
//! fetch('/api/data', {
//!   method: 'POST',
//!   headers: { 'x-csrf-token': token }
//! });
//! ```

use std::convert::Infallible;
use std::future::Future;
use std::pin::Pin;
use std::task::{Context, Poll};

use ajaya_core::{Body, Request, Response};
use cookie::{Cookie, SameSite};
use http::{HeaderValue, Method, StatusCode, header::COOKIE};
use tower_layer::Layer;
use tower_service::Service;
use uuid::Uuid;

pub const CSRF_COOKIE_NAME: &str = "csrf_token";
pub const CSRF_HEADER_NAME: &str = "x-csrf-token";
pub const CSRF_FORM_FIELD: &str = "csrf_token";

/// The CSRF token for the current request.
///
/// Available as `Extension<CsrfToken>` in all handlers.
#[derive(Debug, Clone)]
pub struct CsrfToken(String);

impl CsrfToken {
    /// Generate a new random CSRF token.
    pub fn new() -> Self {
        Self(Uuid::new_v4().to_string())
    }

    /// Create from an existing token string.
    pub fn from_string(token: impl Into<String>) -> Self {
        Self(token.into())
    }

    /// Get the token as a string slice.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl Default for CsrfToken {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Display for CsrfToken {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Tower layer that enforces CSRF protection via the double-submit cookie pattern.
#[derive(Debug, Clone)]
pub struct CsrfLayer {
    cookie_name: &'static str,
    header_name: &'static str,
    secure: bool,
    same_site: SameSite,
}

impl Default for CsrfLayer {
    fn default() -> Self {
        Self {
            cookie_name: CSRF_COOKIE_NAME,
            header_name: CSRF_HEADER_NAME,
            secure: false, // set true in production (HTTPS)
            same_site: SameSite::Strict,
        }
    }
}

impl CsrfLayer {
    /// Create a new `CsrfLayer` with default settings.
    pub fn new() -> Self {
        Self::default()
    }

    /// Mark the CSRF cookie as `Secure` (HTTPS only). Enable in production.
    pub fn secure(mut self, secure: bool) -> Self {
        self.secure = secure;
        self
    }

    /// Set the `SameSite` attribute of the CSRF cookie.
    pub fn same_site(mut self, same_site: SameSite) -> Self {
        self.same_site = same_site;
        self
    }
}

impl<S> Layer<S> for CsrfLayer {
    type Service = CsrfService<S>;

    fn layer(&self, inner: S) -> Self::Service {
        CsrfService {
            inner,
            config: self.clone(),
        }
    }
}

/// Tower service produced by [`CsrfLayer`].
#[derive(Clone)]
pub struct CsrfService<S> {
    inner: S,
    config: CsrfLayer,
}

impl<S> Service<Request> for CsrfService<S>
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

    fn call(&mut self, mut req: Request) -> Self::Future {
        let config = self.config.clone();
        let cloned = self.inner.clone();
        let mut inner = std::mem::replace(&mut self.inner, cloned);

        Box::pin(async move {
            // 1. Extract or generate the CSRF token from the cookie
            let cookie_token = extract_csrf_cookie(req.headers(), config.cookie_name);
            let (csrf_token, is_new) = match cookie_token {
                Some(t) => (CsrfToken::from_string(t), false),
                None => (CsrfToken::new(), true),
            };

            // 2. Insert into request extensions
            req.extensions_mut().insert(csrf_token.clone());

            // 3. For state-changing methods, verify the header matches the cookie
            let method = req.method().clone();
            if is_state_changing(&method) {
                if is_new {
                    // No cookie yet — first request, cannot be CSRF-protected
                    tracing::warn!(method = %method, "CSRF check failed: no token cookie");
                    return Ok(csrf_forbidden());
                }

                let header_token = req
                    .headers()
                    .get(config.header_name)
                    .and_then(|v| v.to_str().ok())
                    .map(|s| s.to_string());

                match header_token {
                    Some(h) if h == csrf_token.as_str() => {
                        // Valid — proceed
                    }
                    Some(bad) => {
                        tracing::warn!(
                            method = %method,
                            header_token = %bad,
                            "CSRF check failed: token mismatch"
                        );
                        return Ok(csrf_forbidden());
                    }
                    None => {
                        tracing::warn!(
                            method = %method,
                            "CSRF check failed: missing x-csrf-token header"
                        );
                        return Ok(csrf_forbidden());
                    }
                }
            }

            // 4. Call inner service
            let mut response = inner.call(req).await?;

            // 5. Set csrf_token cookie on the response if it's new
            if is_new {
                let mut cookie = Cookie::new(config.cookie_name, csrf_token.0.clone());
                cookie.set_http_only(false); // JS must read this
                cookie.set_secure(config.secure);
                cookie.set_same_site(config.same_site);
                cookie.set_path("/");

                if let Ok(val) = HeaderValue::from_str(&cookie.to_string()) {
                    response.headers_mut().append(http::header::SET_COOKIE, val);
                }
            }

            Ok(response)
        })
    }
}

fn is_state_changing(method: &Method) -> bool {
    !matches!(
        *method,
        Method::GET | Method::HEAD | Method::OPTIONS | Method::TRACE
    )
}

fn extract_csrf_cookie(headers: &http::HeaderMap, cookie_name: &str) -> Option<String> {
    for header_val in headers.get_all(COOKIE) {
        if let Ok(cookie_str) = header_val.to_str() {
            for pair in cookie_str.split(';') {
                let pair = pair.trim();
                if let Some((name, value)) = pair.split_once('=') {
                    if name.trim() == cookie_name {
                        return Some(value.trim().to_string());
                    }
                }
            }
        }
    }
    None
}

fn csrf_forbidden() -> Response {
    http::Response::builder()
        .status(StatusCode::FORBIDDEN)
        .header(http::header::CONTENT_TYPE, "application/json")
        .body(Body::from(
            r#"{"error":"Forbidden","code":403,"message":"CSRF token validation failed"}"#,
        ))
        .unwrap()
}
