//! Request ID middleware.
//!
//! Generates a unique UUID v4 per request and attaches it as the
//! `x-request-id` response header and as a typed [`RequestId`] extension.
//!
//! [`PropagateRequestIdLayer`] forwards an incoming `x-request-id` header
//! to the response (for proxied requests).
//!
//! # Example
//!
//! ```rust,ignore
//! use ajaya_middleware::request_id::{RequestIdLayer, PropagateRequestIdLayer, RequestId};
//! use ajaya::Extension;
//!
//! async fn handler(Extension(rid): Extension<RequestId>) -> String {
//!     format!("Request ID: {}", rid.as_str())
//! }
//!
//! Router::new()
//!     .route("/", get(handler))
//!     .layer(RequestIdLayer::new());
//! ```

use std::convert::Infallible;
use std::future::Future;
use std::pin::Pin;
use std::task::{Context, Poll};

use ajaya_core::{Request, Response};
use http::HeaderValue;
use tower_layer::Layer;
use tower_service::Service;
use uuid::Uuid;

pub const X_REQUEST_ID: &str = "x-request-id";

/// A unique request identifier.
///
/// Stored as a typed extension on the request for handler access via
/// `Extension<RequestId>`.
#[derive(Debug, Clone)]
pub struct RequestId(String);

impl RequestId {
    /// Create a new `RequestId` with a UUID v4 value.
    pub fn new() -> Self {
        Self(Uuid::new_v4().to_string())
    }

    /// Create a `RequestId` from an existing string (e.g., from incoming header).
    pub fn from_string(id: impl Into<String>) -> Self {
        Self(id.into())
    }

    /// Get the request ID as a string slice.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl Default for RequestId {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Display for RequestId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

// ── RequestIdLayer ───────────────────────────────────────────────────────────

/// Tower layer that generates a unique `x-request-id` per request.
///
/// If an incoming `x-request-id` header is already present, it is reused
/// rather than overwritten. The ID is also available via `Extension<RequestId>`.
#[derive(Debug, Clone, Default)]
pub struct RequestIdLayer;

impl RequestIdLayer {
    /// Create a new `RequestIdLayer`.
    pub fn new() -> Self {
        Self
    }
}

impl<S> Layer<S> for RequestIdLayer {
    type Service = RequestIdService<S>;

    fn layer(&self, inner: S) -> Self::Service {
        RequestIdService { inner }
    }
}

/// Tower service produced by [`RequestIdLayer`].
#[derive(Clone)]
pub struct RequestIdService<S> {
    inner: S,
}

impl<S> Service<Request> for RequestIdService<S>
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
        // Reuse incoming ID or generate new one
        let request_id = req
            .headers()
            .get(X_REQUEST_ID)
            .and_then(|v| v.to_str().ok())
            .map(RequestId::from_string)
            .unwrap_or_else(RequestId::new);

        let id_str = request_id.0.clone();

        // Insert as extension so handlers can access it
        req.extensions_mut().insert(request_id);

        // Also set on request headers for downstream middleware
        if let Ok(val) = HeaderValue::from_str(&id_str) {
            req.headers_mut().insert(X_REQUEST_ID, val);
        }

        let cloned = self.inner.clone();
        let mut inner = std::mem::replace(&mut self.inner, cloned);

        Box::pin(async move {
            let mut response = inner.call(req).await?;

            // Propagate the request ID to the response
            if let Ok(val) = HeaderValue::from_str(&id_str) {
                response.headers_mut().insert(X_REQUEST_ID, val);
            }

            Ok(response)
        })
    }
}

// ── PropagateRequestIdLayer ──────────────────────────────────────────────────

/// Tower layer that copies an incoming `x-request-id` header to the response.
///
/// Unlike [`RequestIdLayer`], this does **not** generate a new ID if none is
/// present. Use this on services behind a proxy that injects the header.
#[derive(Debug, Clone, Default)]
pub struct PropagateRequestIdLayer;

impl PropagateRequestIdLayer {
    /// Create a new `PropagateRequestIdLayer`.
    pub fn new() -> Self {
        Self
    }
}

impl<S> Layer<S> for PropagateRequestIdLayer {
    type Service = PropagateRequestIdService<S>;

    fn layer(&self, inner: S) -> Self::Service {
        PropagateRequestIdService { inner }
    }
}

/// Tower service produced by [`PropagateRequestIdLayer`].
#[derive(Clone)]
pub struct PropagateRequestIdService<S> {
    inner: S,
}

impl<S> Service<Request> for PropagateRequestIdService<S>
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
        let incoming_id = req.headers().get(X_REQUEST_ID).cloned();

        let cloned = self.inner.clone();
        let mut inner = std::mem::replace(&mut self.inner, cloned);

        Box::pin(async move {
            let mut response = inner.call(req).await?;

            if let Some(id) = incoming_id {
                response.headers_mut().insert(X_REQUEST_ID, id);
            }

            Ok(response)
        })
    }
}
