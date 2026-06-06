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
//! use arvik_middleware::request_id::{RequestIdLayer, PropagateRequestIdLayer, RequestId};
//! use arvik::Extension;
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

use arvik_core::{Request, Response};
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
        // Reuse an incoming valid request ID, otherwise generate a UUID v4.
        let incoming = req.headers().get(X_REQUEST_ID).and_then(|value| {
            value
                .to_str()
                .ok()
                .map(|id| (RequestId::from_string(id), value.clone()))
        });

        let (request_id, header_value) = match incoming {
            Some(incoming) => incoming,
            None => {
                let request_id = RequestId::new();
                let header_value =
                    HeaderValue::from_str(request_id.as_str()).expect("UUID is a valid header");
                (request_id, header_value)
            }
        };

        // Insert as extension so handlers can access it
        req.extensions_mut().insert(request_id);

        // Also set on request headers for downstream middleware
        req.headers_mut().insert(X_REQUEST_ID, header_value.clone());

        let cloned = self.inner.clone();
        let mut inner = std::mem::replace(&mut self.inner, cloned);

        Box::pin(async move {
            let mut response = inner.call(req).await?;

            // Propagate the request ID to the response
            response.headers_mut().insert(X_REQUEST_ID, header_value);

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

#[cfg(test)]
mod tests {
    use super::*;
    use arvik_core::{Body, ResponseBuilder};
    use std::future::{Ready, ready};

    #[derive(Clone)]
    struct EchoRequestIdService;

    impl Service<Request> for EchoRequestIdService {
        type Response = Response;
        type Error = Infallible;
        type Future = Ready<Result<Response, Infallible>>;

        fn poll_ready(&mut self, _cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
            Poll::Ready(Ok(()))
        }

        fn call(&mut self, req: Request) -> Self::Future {
            let id = req
                .extensions()
                .get::<RequestId>()
                .map(RequestId::as_str)
                .unwrap_or("")
                .to_string();
            ready(Ok(ResponseBuilder::new().body(Body::from(id))))
        }
    }

    #[tokio::test]
    async fn request_id_reuses_incoming_header_and_propagates_response() {
        let mut service = RequestIdLayer::new().layer(EchoRequestIdService);
        let req = Request::new(
            http::Request::builder()
                .header(X_REQUEST_ID, "incoming-id")
                .body(Body::empty())
                .unwrap(),
        );

        let response = service.call(req).await.unwrap();
        assert_eq!(response.headers()[X_REQUEST_ID], "incoming-id");
        assert_eq!(
            response.into_body().to_bytes().await.unwrap().as_ref(),
            b"incoming-id"
        );
    }

    #[tokio::test]
    async fn request_id_generates_when_missing() {
        let mut service = RequestIdLayer::new().layer(EchoRequestIdService);
        let req = Request::new(http::Request::builder().body(Body::empty()).unwrap());

        let response = service.call(req).await.unwrap();
        assert!(response.headers().contains_key(X_REQUEST_ID));
        let body = response.into_body().to_bytes().await.unwrap();
        assert!(!body.is_empty());
    }
}
