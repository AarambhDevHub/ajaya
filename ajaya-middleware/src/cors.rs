//! CORS (Cross-Origin Resource Sharing) middleware.
//!
//! Provides [`CorsLayer`] — a Tower [`Layer`] that handles preflight
//! (OPTIONS) requests and injects the appropriate `Access-Control-*`
//! response headers according to the [CORS specification].
//!
//! [CORS specification]: https://fetch.spec.whatwg.org/#http-cors-protocol
//!
//! # Quick start
//!
//! ```rust,ignore
//! use ajaya::Router;
//! use ajaya_middleware::cors::CorsLayer;
//!
//! let app = Router::new()
//!     .route("/api/users", get(handler))
//!     .layer(CorsLayer::permissive());           // development
//!
//! // Or configure explicitly for production:
//! let app = Router::new()
//!     .route("/api/users", get(handler))
//!     .layer(
//!         CorsLayer::new()
//!             .allow_origin("https://app.example.com".parse().unwrap())
//!             .allow_methods([Method::GET, Method::POST])
//!             .allow_headers([AUTHORIZATION, CONTENT_TYPE])
//!             .allow_credentials(true)
//!             .max_age(Duration::from_secs(3600)),
//!     );
//! ```
//!
//! # Layer ordering
//!
//! CORS should be the **outermost** layer so it can intercept preflight
//! requests before any auth middleware rejects them:
//!
//! ```rust,ignore
//! Router::new()
//!     .route_layer(RequireAuthLayer::new()) // inner — skipped on preflight
//!     .layer(CorsLayer::permissive())       // outer — always runs
//! ```
//!
//! # Credential support
//!
//! When `allow_credentials(true)` is set, the `Access-Control-Allow-Origin`
//! header **cannot** be `*` per the CORS spec. Ajaya automatically switches
//! from `*` to reflecting the request `Origin`.

use std::convert::Infallible;
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;
use std::task::{Context, Poll};
use std::time::Duration;

use ajaya_core::response::ResponseBuilder;
use ajaya_core::{Body, Request, Response};
use http::{
    HeaderName, HeaderValue, Method, StatusCode,
    header::{
        ACCESS_CONTROL_ALLOW_CREDENTIALS, ACCESS_CONTROL_ALLOW_HEADERS,
        ACCESS_CONTROL_ALLOW_METHODS, ACCESS_CONTROL_ALLOW_ORIGIN, ACCESS_CONTROL_EXPOSE_HEADERS,
        ACCESS_CONTROL_MAX_AGE, ACCESS_CONTROL_REQUEST_HEADERS, ACCESS_CONTROL_REQUEST_METHOD,
        ORIGIN, VARY,
    },
};
use tower_layer::Layer;
use tower_service::Service;

// ── Configuration ────────────────────────────────────────────────────────────

/// Tower layer that adds CORS support.
///
/// See [module documentation](self) for examples.
#[derive(Debug, Clone)]
pub struct CorsLayer {
    config: Arc<CorsConfig>,
}

#[derive(Debug, Clone)]
struct CorsConfig {
    allow_origins: AllowOrigin,
    allow_methods: AllowMethods,
    allow_headers: AllowHeaders,
    expose_headers: Vec<HeaderName>,
    allow_credentials: bool,
    max_age_secs: Option<u64>,
}

#[derive(Debug, Clone)]
enum AllowOrigin {
    /// Respond with `Access-Control-Allow-Origin: *`
    Any,
    /// Respond with the exact matching origin from an allowlist.
    List(Vec<HeaderValue>),
    /// Reflect the request `Origin` verbatim.
    /// Required when `allow_credentials` is `true`.
    Mirror,
}

#[derive(Debug, Clone)]
enum AllowMethods {
    Any,
    List(Vec<Method>),
}

#[derive(Debug, Clone)]
enum AllowHeaders {
    Any,
    /// Reflect `Access-Control-Request-Headers` from the preflight.
    Mirror,
    List(Vec<HeaderName>),
}

// ── CorsLayer builder ────────────────────────────────────────────────────────

impl CorsLayer {
    /// Create a minimal `CorsLayer` with **no** allowed origins.
    ///
    /// Use the builder methods to configure origins, methods, and headers.
    pub fn new() -> Self {
        Self {
            config: Arc::new(CorsConfig {
                allow_origins: AllowOrigin::List(Vec::new()),
                allow_methods: AllowMethods::List(vec![
                    Method::GET,
                    Method::POST,
                    Method::PUT,
                    Method::DELETE,
                    Method::PATCH,
                ]),
                allow_headers: AllowHeaders::List(Vec::new()),
                expose_headers: Vec::new(),
                allow_credentials: false,
                max_age_secs: Some(3600),
            }),
        }
    }

    /// Allow all origins, all methods, all headers. Credentials **not** allowed.
    ///
    /// `Access-Control-Allow-Origin: *` is sent for all cross-origin requests.
    /// Suitable for fully public read-only APIs.
    pub fn permissive() -> Self {
        Self {
            config: Arc::new(CorsConfig {
                allow_origins: AllowOrigin::Any,
                allow_methods: AllowMethods::Any,
                allow_headers: AllowHeaders::Any,
                expose_headers: Vec::new(),
                allow_credentials: false,
                max_age_secs: Some(86400),
            }),
        }
    }

    /// Allow all origins with credentials.
    ///
    /// Mirrors the request `Origin` (CORS spec forbids `*` with credentials).
    /// Suitable for trusted cross-origin applications that need cookies or auth.
    pub fn very_permissive() -> Self {
        Self {
            config: Arc::new(CorsConfig {
                allow_origins: AllowOrigin::Mirror,
                allow_methods: AllowMethods::Any,
                allow_headers: AllowHeaders::Mirror,
                expose_headers: Vec::new(),
                allow_credentials: true,
                max_age_secs: Some(86400),
            }),
        }
    }

    // ── Builder helpers ──────────────────────────────────────────────────────

    fn mutate<F: FnOnce(&mut CorsConfig)>(self, f: F) -> Self {
        let mut cfg = Arc::try_unwrap(self.config).unwrap_or_else(|arc| (*arc).clone());
        f(&mut cfg);
        Self {
            config: Arc::new(cfg),
        }
    }

    /// Set the allowed origin(s).
    ///
    /// ```rust,ignore
    /// // Single origin
    /// .allow_origin("https://app.example.com".parse::<HeaderValue>().unwrap())
    ///
    /// // Multiple origins
    /// .allow_origin([
    ///     "https://app.example.com".parse().unwrap(),
    ///     "https://admin.example.com".parse().unwrap(),
    /// ])
    /// ```
    pub fn allow_origin<O: IntoAllowOrigin>(self, origin: O) -> Self {
        self.mutate(|c| c.allow_origins = origin.into_allow_origin())
    }

    /// Set the allowed HTTP methods for CORS requests.
    ///
    /// ```rust,ignore
    /// .allow_methods([Method::GET, Method::POST, Method::DELETE])
    /// ```
    pub fn allow_methods<I>(self, methods: I) -> Self
    where
        I: IntoIterator<Item = Method>,
    {
        self.mutate(|c| c.allow_methods = AllowMethods::List(methods.into_iter().collect()))
    }

    /// Set the allowed request headers.
    ///
    /// ```rust,ignore
    /// use http::header::{AUTHORIZATION, CONTENT_TYPE};
    /// .allow_headers([AUTHORIZATION, CONTENT_TYPE])
    /// ```
    pub fn allow_headers<I>(self, headers: I) -> Self
    where
        I: IntoIterator<Item = HeaderName>,
    {
        self.mutate(|c| c.allow_headers = AllowHeaders::List(headers.into_iter().collect()))
    }

    /// Expose response headers to the browser via `Access-Control-Expose-Headers`.
    pub fn expose_headers<I>(self, headers: I) -> Self
    where
        I: IntoIterator<Item = HeaderName>,
    {
        self.mutate(|c| c.expose_headers = headers.into_iter().collect())
    }

    /// Allow or deny cookies, HTTP auth, and TLS client certificates.
    ///
    /// When set to `true`:
    /// - `Access-Control-Allow-Credentials: true` is added to responses.
    /// - `Access-Control-Allow-Origin` cannot be `*`; the layer automatically
    ///   switches to mirroring the request origin.
    pub fn allow_credentials(self, allow: bool) -> Self {
        self.mutate(|c| {
            c.allow_credentials = allow;
            if allow {
                if let AllowOrigin::Any = c.allow_origins {
                    c.allow_origins = AllowOrigin::Mirror;
                }
            }
        })
    }

    /// Set the preflight response cache duration (`Access-Control-Max-Age`).
    pub fn max_age(self, duration: Duration) -> Self {
        self.mutate(|c| c.max_age_secs = Some(duration.as_secs()))
    }
}

impl Default for CorsLayer {
    fn default() -> Self {
        Self::new()
    }
}

// ── IntoAllowOrigin conversions ───────────────────────────────────────────────

/// Types that can be converted into an [`AllowOrigin`] configuration.
pub trait IntoAllowOrigin: sealed::Sealed {
    #[doc(hidden)]
    #[allow(private_interfaces)]
    fn into_allow_origin(self) -> AllowOrigin;
}

mod sealed {
    pub trait Sealed {}
    impl Sealed for http::HeaderValue {}
    impl<const N: usize> Sealed for [http::HeaderValue; N] {}
    impl Sealed for Vec<http::HeaderValue> {}
}

impl IntoAllowOrigin for HeaderValue {
    #[allow(private_interfaces)]
    fn into_allow_origin(self) -> AllowOrigin {
        AllowOrigin::List(vec![self])
    }
}

impl<const N: usize> IntoAllowOrigin for [HeaderValue; N] {
    #[allow(private_interfaces)]
    fn into_allow_origin(self) -> AllowOrigin {
        AllowOrigin::List(self.into())
    }
}

impl IntoAllowOrigin for Vec<HeaderValue> {
    #[allow(private_interfaces)]
    fn into_allow_origin(self) -> AllowOrigin {
        AllowOrigin::List(self)
    }
}

// ── Tower Layer + Service ─────────────────────────────────────────────────────

impl<S> Layer<S> for CorsLayer {
    type Service = CorsService<S>;

    fn layer(&self, inner: S) -> Self::Service {
        CorsService {
            inner,
            config: Arc::clone(&self.config),
        }
    }
}

/// The Tower service produced by [`CorsLayer`].
#[derive(Clone)]
pub struct CorsService<S> {
    inner: S,
    config: Arc<CorsConfig>,
}

impl<S> Service<Request> for CorsService<S>
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
        let config = Arc::clone(&self.config);
        // Clone inner so we can move it into the async block.
        // `std::mem::replace` ensures the original `self.inner` is left in a
        // valid (cloned) state for the next `call`.
        let cloned = self.inner.clone();
        let mut inner = std::mem::replace(&mut self.inner, cloned);

        Box::pin(async move {
            let origin = match req.headers().get(ORIGIN).cloned() {
                Some(o) => o,
                None => {
                    // Not a CORS request — pass through unchanged
                    return inner.call(req).await;
                }
            };

            // ── Preflight ─────────────────────────────────────────────────
            let is_preflight = req.method() == Method::OPTIONS
                && req.headers().contains_key(ACCESS_CONTROL_REQUEST_METHOD);

            if is_preflight {
                tracing::trace!(
                    origin = %origin.to_str().unwrap_or("?"),
                    "CORS preflight"
                );
                return Ok(config.build_preflight(&origin, &req));
            }

            // ── Actual CORS request ───────────────────────────────────────
            let mut response = inner.call(req).await?;
            config.apply_response_headers(&mut response, &origin);
            Ok(response)
        })
    }
}

// ── CORS logic ────────────────────────────────────────────────────────────────

impl CorsConfig {
    /// Resolve the `Access-Control-Allow-Origin` value for a given request origin.
    ///
    /// Returns `None` if the origin is not allowed (no CORS headers should be added).
    fn resolved_origin(&self, origin: &HeaderValue) -> Option<HeaderValue> {
        match &self.allow_origins {
            AllowOrigin::Any if !self.allow_credentials => Some(HeaderValue::from_static("*")),
            AllowOrigin::Any | AllowOrigin::Mirror => {
                // Must mirror when credentials are enabled or explicitly configured
                Some(origin.clone())
            }
            AllowOrigin::List(list) => {
                if list.iter().any(|o| o == origin) {
                    Some(origin.clone())
                } else {
                    None // origin not on allowlist
                }
            }
        }
    }

    fn methods_header(&self) -> HeaderValue {
        match &self.allow_methods {
            AllowMethods::Any => {
                HeaderValue::from_static("GET, HEAD, POST, PUT, DELETE, PATCH, OPTIONS")
            }
            AllowMethods::List(ms) => {
                let s = ms.iter().map(Method::as_str).collect::<Vec<_>>().join(", ");
                HeaderValue::from_str(&s).unwrap_or_else(|_| HeaderValue::from_static("GET"))
            }
        }
    }

    fn headers_header(&self, request_headers: &http::HeaderMap) -> Option<HeaderValue> {
        match &self.allow_headers {
            AllowHeaders::Any => Some(HeaderValue::from_static("*")),
            AllowHeaders::Mirror => request_headers.get(ACCESS_CONTROL_REQUEST_HEADERS).cloned(),
            AllowHeaders::List(hs) => {
                if hs.is_empty() {
                    return None;
                }
                let s = hs
                    .iter()
                    .map(HeaderName::as_str)
                    .collect::<Vec<_>>()
                    .join(", ");
                HeaderValue::from_str(&s).ok()
            }
        }
    }

    /// Build the complete preflight response.
    fn build_preflight(&self, origin: &HeaderValue, req: &Request) -> Response {
        let Some(allow_origin) = self.resolved_origin(origin) else {
            tracing::debug!(
                origin = %origin.to_str().unwrap_or("?"),
                "CORS preflight rejected: origin not allowed"
            );
            // Return a bare 204 with no CORS headers; the browser rejects the request.
            return ResponseBuilder::new()
                .status(StatusCode::NO_CONTENT)
                .body(Body::empty());
        };

        let mut builder = ResponseBuilder::new()
            .status(StatusCode::NO_CONTENT)
            .header(ACCESS_CONTROL_ALLOW_ORIGIN, allow_origin)
            .header(ACCESS_CONTROL_ALLOW_METHODS, self.methods_header());

        if let Some(h) = self.headers_header(req.headers()) {
            builder = builder.header(ACCESS_CONTROL_ALLOW_HEADERS, h);
        }

        if self.allow_credentials {
            builder = builder.header(ACCESS_CONTROL_ALLOW_CREDENTIALS, "true");
        }

        if let Some(age) = self.max_age_secs {
            if let Ok(v) = HeaderValue::from_str(&age.to_string()) {
                builder = builder.header(ACCESS_CONTROL_MAX_AGE, v);
            }
        }

        // Vary: Origin — required when response differs per origin
        if !matches!(&self.allow_origins, AllowOrigin::Any) || self.allow_credentials {
            builder = builder.header(VARY, "origin");
        }

        builder.body(Body::empty())
    }

    /// Inject CORS headers into a regular (non-preflight) response.
    fn apply_response_headers(&self, response: &mut Response, origin: &HeaderValue) {
        let Some(allow_origin) = self.resolved_origin(origin) else {
            tracing::debug!(
                origin = %origin.to_str().unwrap_or("?"),
                "CORS request: origin not in allowlist, skipping CORS headers"
            );
            return;
        };

        let headers = response.headers_mut();
        headers.insert(ACCESS_CONTROL_ALLOW_ORIGIN, allow_origin);

        if self.allow_credentials {
            headers.insert(
                ACCESS_CONTROL_ALLOW_CREDENTIALS,
                HeaderValue::from_static("true"),
            );
        }

        if !self.expose_headers.is_empty() {
            let s = self
                .expose_headers
                .iter()
                .map(HeaderName::as_str)
                .collect::<Vec<_>>()
                .join(", ");
            if let Ok(v) = HeaderValue::from_str(&s) {
                headers.insert(ACCESS_CONTROL_EXPOSE_HEADERS, v);
            }
        }

        // Vary: Origin — ensures caches don't serve wrong-origin responses
        if !matches!(&self.allow_origins, AllowOrigin::Any) || self.allow_credentials {
            headers.append(VARY, HeaderValue::from_static("origin"));
        }
    }
}
