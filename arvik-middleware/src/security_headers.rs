//! Security header middleware.
//!
//! Provides several layers for injecting HTTP security headers:
//!
//! - [`SensitiveHeadersLayer`] — marks headers as sensitive so they're redacted in logs
//! - [`SetResponseHeaderLayer`] — set, override, or append a single response header
//! - [`SetRequestHeaderLayer`] — set, override, or append a single request header
//! - [`SecurityHeadersLayer`] — injects the full OWASP-recommended security header suite
//!
//! # Example
//!
//! ```rust,ignore
//! use arvik_middleware::security_headers::{SecurityHeadersLayer, SetResponseHeaderLayer};
//!
//! // Full OWASP suite in one call:
//! Router::new().layer(SecurityHeadersLayer::new());
//!
//! // Or individual headers:
//! Router::new()
//!     .layer(SetResponseHeaderLayer::overriding(
//!         http::header::X_FRAME_OPTIONS,
//!         "SAMEORIGIN",
//!     ));
//! ```

use std::convert::Infallible;
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;
use std::task::{Context, Poll};

use arvik_core::{Request, Response};
use http::{HeaderName, HeaderValue};
use tower_layer::Layer;
use tower_service::Service;

// ── HeaderMode ───────────────────────────────────────────────────────────────

/// Controls how a header is set on the response.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HeaderMode {
    /// Only set the header if it does not already exist.
    IfNotPresent,
    /// Always set the header, overwriting any existing value.
    Overriding,
    /// Append the value even if the header already exists.
    Appending,
}

// ── SetResponseHeaderLayer ───────────────────────────────────────────────────

/// Tower layer that sets a header on every response.
#[derive(Clone)]
pub struct SetResponseHeaderLayer {
    name: HeaderName,
    value: HeaderValue,
    mode: HeaderMode,
}

impl SetResponseHeaderLayer {
    /// Set the header only if not already present.
    pub fn if_not_present<N, V>(name: N, value: V) -> Self
    where
        N: TryInto<HeaderName>,
        V: TryInto<HeaderValue>,
        <N as TryInto<HeaderName>>::Error: std::fmt::Debug,
        <V as TryInto<HeaderValue>>::Error: std::fmt::Debug,
    {
        Self {
            name: name.try_into().expect("invalid header name"),
            value: value.try_into().expect("invalid header value"),
            mode: HeaderMode::IfNotPresent,
        }
    }

    /// Always set the header, overwriting existing values.
    pub fn overriding<N, V>(name: N, value: V) -> Self
    where
        N: TryInto<HeaderName>,
        V: TryInto<HeaderValue>,
        <N as TryInto<HeaderName>>::Error: std::fmt::Debug,
        <V as TryInto<HeaderValue>>::Error: std::fmt::Debug,
    {
        Self {
            name: name.try_into().expect("invalid header name"),
            value: value.try_into().expect("invalid header value"),
            mode: HeaderMode::Overriding,
        }
    }

    /// Append the header value even if the header already exists.
    pub fn appending<N, V>(name: N, value: V) -> Self
    where
        N: TryInto<HeaderName>,
        V: TryInto<HeaderValue>,
        <N as TryInto<HeaderName>>::Error: std::fmt::Debug,
        <V as TryInto<HeaderValue>>::Error: std::fmt::Debug,
    {
        Self {
            name: name.try_into().expect("invalid header name"),
            value: value.try_into().expect("invalid header value"),
            mode: HeaderMode::Appending,
        }
    }
}

impl<S> Layer<S> for SetResponseHeaderLayer {
    type Service = SetResponseHeaderService<S>;

    fn layer(&self, inner: S) -> Self::Service {
        SetResponseHeaderService {
            inner,
            name: self.name.clone(),
            value: self.value.clone(),
            mode: self.mode,
        }
    }
}

/// Tower service produced by [`SetResponseHeaderLayer`].
#[derive(Clone)]
pub struct SetResponseHeaderService<S> {
    inner: S,
    name: HeaderName,
    value: HeaderValue,
    mode: HeaderMode,
}

impl<S> Service<Request> for SetResponseHeaderService<S>
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
        let cloned = self.inner.clone();
        let mut inner = std::mem::replace(&mut self.inner, cloned);
        let name = self.name.clone();
        let value = self.value.clone();
        let mode = self.mode;

        Box::pin(async move {
            let mut response = inner.call(req).await?;
            let headers = response.headers_mut();
            match mode {
                HeaderMode::IfNotPresent => {
                    headers.entry(&name).or_insert(value);
                }
                HeaderMode::Overriding => {
                    headers.insert(name, value);
                }
                HeaderMode::Appending => {
                    headers.append(name, value);
                }
            }
            Ok(response)
        })
    }
}

// ── SetRequestHeaderLayer ────────────────────────────────────────────────────

/// Tower layer that sets a header on every request.
#[derive(Clone)]
pub struct SetRequestHeaderLayer {
    name: HeaderName,
    value: HeaderValue,
    mode: HeaderMode,
}

impl SetRequestHeaderLayer {
    /// Set the request header only if not already present.
    pub fn if_not_present<N, V>(name: N, value: V) -> Self
    where
        N: TryInto<HeaderName>,
        V: TryInto<HeaderValue>,
        <N as TryInto<HeaderName>>::Error: std::fmt::Debug,
        <V as TryInto<HeaderValue>>::Error: std::fmt::Debug,
    {
        Self {
            name: name.try_into().expect("invalid header name"),
            value: value.try_into().expect("invalid header value"),
            mode: HeaderMode::IfNotPresent,
        }
    }

    /// Always set the request header, overwriting existing values.
    pub fn overriding<N, V>(name: N, value: V) -> Self
    where
        N: TryInto<HeaderName>,
        V: TryInto<HeaderValue>,
        <N as TryInto<HeaderName>>::Error: std::fmt::Debug,
        <V as TryInto<HeaderValue>>::Error: std::fmt::Debug,
    {
        Self {
            name: name.try_into().expect("invalid header name"),
            value: value.try_into().expect("invalid header value"),
            mode: HeaderMode::Overriding,
        }
    }
}

impl<S> Layer<S> for SetRequestHeaderLayer {
    type Service = SetRequestHeaderService<S>;

    fn layer(&self, inner: S) -> Self::Service {
        SetRequestHeaderService {
            inner,
            name: self.name.clone(),
            value: self.value.clone(),
            mode: self.mode,
        }
    }
}

/// Tower service produced by [`SetRequestHeaderLayer`].
#[derive(Clone)]
pub struct SetRequestHeaderService<S> {
    inner: S,
    name: HeaderName,
    value: HeaderValue,
    mode: HeaderMode,
}

impl<S> Service<Request> for SetRequestHeaderService<S>
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
        {
            let headers = req.headers_mut();
            match self.mode {
                HeaderMode::IfNotPresent => {
                    headers.entry(&self.name).or_insert(self.value.clone());
                }
                HeaderMode::Overriding => {
                    headers.insert(self.name.clone(), self.value.clone());
                }
                HeaderMode::Appending => {
                    headers.append(self.name.clone(), self.value.clone());
                }
            }
        }

        let cloned = self.inner.clone();
        let mut inner = std::mem::replace(&mut self.inner, cloned);
        Box::pin(async move { inner.call(req).await })
    }
}

// ── SensitiveHeadersLayer ─────────────────────────────────────────────────────

/// Tower layer that marks headers as sensitive for redaction in tracing/logs.
///
/// Headers listed here will have their values replaced with `[redacted]` in
/// tracing span fields (when using the `TraceLayer`). The actual header values
/// in requests and responses are unaffected.
///
/// # Example
///
/// ```rust,ignore
/// use arvik_middleware::security_headers::SensitiveHeadersLayer;
/// use http::header::{AUTHORIZATION, COOKIE};
///
/// Router::new()
///     .layer(SensitiveHeadersLayer::new([AUTHORIZATION, COOKIE]));
/// ```
#[derive(Clone)]
pub struct SensitiveHeadersLayer {
    headers: Vec<HeaderName>,
}

impl SensitiveHeadersLayer {
    /// Create a new `SensitiveHeadersLayer` with the given headers to redact.
    pub fn new(headers: impl IntoIterator<Item = HeaderName>) -> Self {
        Self {
            headers: headers.into_iter().collect(),
        }
    }
}

impl<S> Layer<S> for SensitiveHeadersLayer {
    type Service = SensitiveHeadersService<S>;

    fn layer(&self, inner: S) -> Self::Service {
        SensitiveHeadersService {
            inner,
            headers: self.headers.clone(),
        }
    }
}

/// Marker for sensitive header tracking (stored in extensions).
#[derive(Clone, Debug)]
pub struct SensitiveHeaders(pub Vec<HeaderName>);

/// Tower service produced by [`SensitiveHeadersLayer`].
#[derive(Clone)]
pub struct SensitiveHeadersService<S> {
    inner: S,
    headers: Vec<HeaderName>,
}

impl<S> Service<Request> for SensitiveHeadersService<S>
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
        req.extensions_mut()
            .insert(SensitiveHeaders(self.headers.clone()));

        let cloned = self.inner.clone();
        let mut inner = std::mem::replace(&mut self.inner, cloned);
        Box::pin(async move { inner.call(req).await })
    }
}

// ── SecurityHeadersLayer ──────────────────────────────────────────────────────

/// Tower layer that injects the full OWASP-recommended security header suite.
///
/// Sets the following headers on every response:
///
/// | Header | Value |
/// |---|---|
/// | `X-Frame-Options` | `DENY` |
/// | `X-Content-Type-Options` | `nosniff` |
/// | `X-XSS-Protection` | `1; mode=block` |
/// | `Strict-Transport-Security` | `max-age=31536000; includeSubDomains` |
/// | `Referrer-Policy` | `strict-origin-when-cross-origin` |
/// | `Permissions-Policy` | `geolocation=(), microphone=(), camera=()` |
/// | `Content-Security-Policy` | configurable, defaults to basic policy |
///
/// # Example
///
/// ```rust,ignore
/// use arvik_middleware::security_headers::SecurityHeadersLayer;
///
/// Router::new()
///     .route("/", get(handler))
///     .layer(SecurityHeadersLayer::new());
/// ```
#[derive(Debug, Clone)]
pub struct SecurityHeadersLayer {
    csp: Option<String>,
    hsts_max_age: u64,
    frame_options: &'static str,
}

impl Default for SecurityHeadersLayer {
    fn default() -> Self {
        Self {
            csp: Some("default-src 'self'; script-src 'self'; style-src 'self' 'unsafe-inline'; img-src 'self' data:; font-src 'self'; connect-src 'self'; frame-ancestors 'none'".to_owned()),
            hsts_max_age: 31_536_000, // 1 year
            frame_options: "DENY",
        }
    }
}

impl SecurityHeadersLayer {
    /// Create with OWASP-recommended defaults.
    pub fn new() -> Self {
        Self::default()
    }

    /// Override the Content-Security-Policy value.
    pub fn content_security_policy(mut self, csp: impl Into<String>) -> Self {
        self.csp = Some(csp.into());
        self
    }

    /// Disable the Content-Security-Policy header.
    pub fn no_content_security_policy(mut self) -> Self {
        self.csp = None;
        self
    }

    /// Set the HSTS max-age in seconds (default: 31536000 / 1 year).
    pub fn hsts_max_age(mut self, secs: u64) -> Self {
        self.hsts_max_age = secs;
        self
    }

    /// Set `X-Frame-Options` value. Options: `"DENY"`, `"SAMEORIGIN"`.
    pub fn frame_options(mut self, value: &'static str) -> Self {
        self.frame_options = value;
        self
    }
}

impl<S> Layer<S> for SecurityHeadersLayer {
    type Service = SecurityHeadersService<S>;

    fn layer(&self, inner: S) -> Self::Service {
        SecurityHeadersService {
            inner,
            headers: build_security_headers(self),
        }
    }
}

/// Tower service produced by [`SecurityHeadersLayer`].
#[derive(Clone)]
pub struct SecurityHeadersService<S> {
    inner: S,
    headers: Arc<[(HeaderName, HeaderValue)]>,
}

impl<S> Service<Request> for SecurityHeadersService<S>
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
        let cloned = self.inner.clone();
        let mut inner = std::mem::replace(&mut self.inner, cloned);
        let headers = Arc::clone(&self.headers);

        Box::pin(async move {
            let mut response = inner.call(req).await?;
            let h = response.headers_mut();

            for (name, value) in headers.iter() {
                h.entry(name.clone()).or_insert_with(|| value.clone());
            }

            Ok(response)
        })
    }
}

fn build_security_headers(config: &SecurityHeadersLayer) -> Arc<[(HeaderName, HeaderValue)]> {
    let mut headers = Vec::with_capacity(7);
    headers.push((
        HeaderName::from_static("x-frame-options"),
        HeaderValue::from_static(config.frame_options),
    ));
    headers.push((
        HeaderName::from_static("x-content-type-options"),
        HeaderValue::from_static("nosniff"),
    ));
    headers.push((
        HeaderName::from_static("x-xss-protection"),
        HeaderValue::from_static("1; mode=block"),
    ));
    headers.push((
        HeaderName::from_static("strict-transport-security"),
        HeaderValue::from_str(&format!(
            "max-age={}; includeSubDomains",
            config.hsts_max_age
        ))
        .expect("generated HSTS header is valid"),
    ));
    headers.push((
        HeaderName::from_static("referrer-policy"),
        HeaderValue::from_static("strict-origin-when-cross-origin"),
    ));
    headers.push((
        HeaderName::from_static("permissions-policy"),
        HeaderValue::from_static("geolocation=(), microphone=(), camera=(), payment=()"),
    ));

    if let Some(csp) = &config.csp {
        headers.push((
            HeaderName::from_static("content-security-policy"),
            HeaderValue::from_str(csp).expect("configured CSP header is valid"),
        ));
    }

    headers.into()
}

#[cfg(test)]
mod tests {
    use super::*;
    use arvik_core::{Body, ResponseBuilder};
    use std::future::{Ready, ready};

    #[derive(Clone)]
    struct OkService;

    impl Service<Request> for OkService {
        type Response = Response;
        type Error = Infallible;
        type Future = Ready<Result<Response, Infallible>>;

        fn poll_ready(&mut self, _cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
            Poll::Ready(Ok(()))
        }

        fn call(&mut self, _req: Request) -> Self::Future {
            ready(Ok(ResponseBuilder::new()
                .header("x-frame-options", "SAMEORIGIN")
                .body(Body::empty())))
        }
    }

    #[tokio::test]
    async fn security_headers_are_precomputed_and_do_not_override_existing() {
        let mut service = SecurityHeadersLayer::new().layer(OkService);
        let req = Request::new(http::Request::builder().body(Body::empty()).unwrap());
        let response = service.call(req).await.unwrap();

        assert_eq!(response.headers()["x-frame-options"], "SAMEORIGIN");
        assert_eq!(response.headers()["x-content-type-options"], "nosniff");
        assert_eq!(
            response.headers()["strict-transport-security"],
            "max-age=31536000; includeSubDomains"
        );
    }
}
