//! Body transformation middleware.
//!
//! [`MapRequestBodyLayer`] transforms the request body before it reaches the handler.
//! [`MapResponseBodyLayer`] transforms the response body after the handler runs.
//!
//! These are lower-level than [`map_request`]/[`map_response`] — they operate
//! on the raw body bytes rather than the full request/response.
//!
//! # Example
//!
//! ```rust,ignore
//! use ajaya_middleware::map_body::{MapRequestBodyLayer, MapResponseBodyLayer};
//! use bytes::Bytes;
//!
//! // Uppercase every request body (silly example)
//! Router::new().layer(MapRequestBodyLayer::new(|bytes: Bytes| async move {
//!     Bytes::from(bytes.to_ascii_uppercase())
//! }));
//!
//! // Append a footer to every response body
//! Router::new().layer(MapResponseBodyLayer::new(|bytes: Bytes| async move {
//!     let mut out = bytes.to_vec();
//!     out.extend_from_slice(b"\n<!-- served by ajaya -->");
//!     Bytes::from(out)
//! }));
//! ```

use std::convert::Infallible;
use std::future::Future;
use std::pin::Pin;
use std::task::{Context, Poll};

use ajaya_core::{Body, Request, Response};
use tokio_util::bytes::Bytes;
use tower_layer::Layer;
use tower_service::Service;

// ── MapRequestBodyLayer ───────────────────────────────────────────────────────

/// Tower layer that transforms the request body.
#[derive(Clone, Debug)]
pub struct MapRequestBodyLayer<F> {
    f: F,
}

impl<F> MapRequestBodyLayer<F> {
    /// Create a new `MapRequestBodyLayer` with the given transform function.
    pub fn new(f: F) -> Self {
        Self { f }
    }
}

impl<F, Fut, Svc> Layer<Svc> for MapRequestBodyLayer<F>
where
    F: Fn(Bytes) -> Fut + Clone + Send + Sync + 'static,
    Fut: Future<Output = Bytes> + Send + 'static,
    Svc: Service<Request, Response = Response, Error = Infallible> + Clone + Send + 'static,
    Svc::Future: Send + 'static,
{
    type Service = MapRequestBodyService<F, Svc>;

    fn layer(&self, inner: Svc) -> Self::Service {
        MapRequestBodyService {
            f: self.f.clone(),
            inner,
        }
    }
}

/// Tower service produced by [`MapRequestBodyLayer`].
#[derive(Clone)]
pub struct MapRequestBodyService<F, Svc> {
    f: F,
    inner: Svc,
}

impl<F, Fut, Svc> Service<Request> for MapRequestBodyService<F, Svc>
where
    F: Fn(Bytes) -> Fut + Clone + Send + Sync + 'static,
    Fut: Future<Output = Bytes> + Send + 'static,
    Svc: Service<Request, Response = Response, Error = Infallible> + Clone + Send + 'static,
    Svc::Future: Send + 'static,
{
    type Response = Response;
    type Error = Infallible;
    type Future = Pin<Box<dyn Future<Output = Result<Response, Infallible>> + Send + 'static>>;

    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.inner.poll_ready(cx)
    }

    fn call(&mut self, req: Request) -> Self::Future {
        let f = self.f.clone();
        let cloned = self.inner.clone();
        let mut inner = std::mem::replace(&mut self.inner, cloned);

        Box::pin(async move {
            let (parts, body) = req.into_request_parts();

            let bytes = match body.to_bytes().await {
                Ok(b) => b,
                Err(_) => Bytes::new(),
            };

            let transformed = f(bytes).await;
            let new_req = Request::from_request_parts(parts, Body::from_bytes(transformed));
            inner.call(new_req).await
        })
    }
}

// ── MapResponseBodyLayer ──────────────────────────────────────────────────────

/// Tower layer that transforms the response body.
#[derive(Clone, Debug)]
pub struct MapResponseBodyLayer<F> {
    f: F,
}

impl<F> MapResponseBodyLayer<F> {
    /// Create a new `MapResponseBodyLayer` with the given transform function.
    pub fn new(f: F) -> Self {
        Self { f }
    }
}

impl<F, Fut, Svc> Layer<Svc> for MapResponseBodyLayer<F>
where
    F: Fn(Bytes) -> Fut + Clone + Send + Sync + 'static,
    Fut: Future<Output = Bytes> + Send + 'static,
    Svc: Service<Request, Response = Response, Error = Infallible> + Clone + Send + 'static,
    Svc::Future: Send + 'static,
{
    type Service = MapResponseBodyService<F, Svc>;

    fn layer(&self, inner: Svc) -> Self::Service {
        MapResponseBodyService {
            f: self.f.clone(),
            inner,
        }
    }
}

/// Tower service produced by [`MapResponseBodyLayer`].
#[derive(Clone)]
pub struct MapResponseBodyService<F, Svc> {
    f: F,
    inner: Svc,
}

impl<F, Fut, Svc> Service<Request> for MapResponseBodyService<F, Svc>
where
    F: Fn(Bytes) -> Fut + Clone + Send + Sync + 'static,
    Fut: Future<Output = Bytes> + Send + 'static,
    Svc: Service<Request, Response = Response, Error = Infallible> + Clone + Send + 'static,
    Svc::Future: Send + 'static,
{
    type Response = Response;
    type Error = Infallible;
    type Future = Pin<Box<dyn Future<Output = Result<Response, Infallible>> + Send + 'static>>;

    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.inner.poll_ready(cx)
    }

    fn call(&mut self, req: Request) -> Self::Future {
        let f = self.f.clone();
        let cloned = self.inner.clone();
        let mut inner = std::mem::replace(&mut self.inner, cloned);

        Box::pin(async move {
            let response = inner.call(req).await?;
            let (parts, body) = response.into_parts();

            let bytes = match body.to_bytes().await {
                Ok(b) => b,
                Err(_) => Bytes::new(),
            };

            let transformed = f(bytes).await;
            let new_body = Body::from_bytes(transformed);
            Ok(http::Response::from_parts(parts, new_body))
        })
    }
}
