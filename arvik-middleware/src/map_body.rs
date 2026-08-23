//! Body transformation middleware.
//!
//! [`MapRequestBodyLayer`] transforms the request body before it reaches the handler.
//! [`MapResponseBodyLayer`] transforms the response body after the handler runs.
//!
//! These are lower-level than `map_request`/`map_response` — they operate
//! on the raw body bytes rather than the full request/response.
//!
//! # Example
//!
//! ```rust,ignore
//! use arvik_middleware::map_body::{MapRequestBodyLayer, MapResponseBodyLayer};
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
//!     out.extend_from_slice(b"\n<!-- served by arvik -->");
//!     Bytes::from(out)
//! }));
//! ```

use std::convert::Infallible;
use std::future::Future;
use std::pin::Pin;
use std::task::{Context, Poll};

use arvik_core::{Body, Request, Response};
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
            let (mut parts, body) = req.into_request_parts();

            let bytes = match body.to_bytes().await {
                Ok(b) => b,
                Err(_) => Bytes::new(),
            };

            let transformed = f(bytes).await;
            // The transform may change the size — refresh Content-Length so
            // downstream length checks describe the real body.
            if parts
                .headers_mut()
                .contains_key(http::header::CONTENT_LENGTH)
            {
                let value = http::HeaderValue::from_str(&transformed.len().to_string())
                    .expect("Content-Length from usize is a valid header value");
                parts
                    .headers_mut()
                    .insert(http::header::CONTENT_LENGTH, value);
            }
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
            let (mut parts, body) = response.into_parts();

            let bytes = match body.to_bytes().await {
                Ok(b) => b,
                Err(_) => Bytes::new(),
            };

            let transformed = f(bytes).await;
            // The transform may change the size — a stale Content-Length makes
            // hyper frame the response by the old length (truncated bodies).
            if parts.headers.contains_key(http::header::CONTENT_LENGTH) {
                let value = http::HeaderValue::from_str(&transformed.len().to_string())
                    .expect("Content-Length from usize is a valid header value");
                parts.headers.insert(http::header::CONTENT_LENGTH, value);
            }
            let new_body = Body::from_bytes(transformed);
            Ok(http::Response::from_parts(parts, new_body))
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use arvik_core::extract::FromRequest;

    #[derive(Clone)]
    struct FixedResponseService;

    impl Service<Request> for FixedResponseService {
        type Response = Response;
        type Error = Infallible;
        type Future = Pin<Box<dyn Future<Output = Result<Response, Infallible>> + Send + 'static>>;

        fn poll_ready(&mut self, _: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
            Poll::Ready(Ok(()))
        }

        fn call(&mut self, _: Request) -> Self::Future {
            Box::pin(async move {
                // "hello" with an accurate Content-Length.
                let res = http::Response::builder()
                    .status(http::StatusCode::OK)
                    .header(http::header::CONTENT_LENGTH, "5")
                    .body(Body::from(Bytes::from_static(b"hello")))
                    .unwrap();
                Ok(res)
            })
        }
    }

    #[derive(Clone)]
    struct EchoBodyService;

    impl Service<Request> for EchoBodyService {
        type Response = Response;
        type Error = Infallible;
        type Future = Pin<Box<dyn Future<Output = Result<Response, Infallible>> + Send + 'static>>;

        fn poll_ready(&mut self, _: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
            Poll::Ready(Ok(()))
        }

        fn call(&mut self, req: Request) -> Self::Future {
            Box::pin(async move {
                let cl = req
                    .headers()
                    .get(http::header::CONTENT_LENGTH)
                    .and_then(|v| v.to_str().ok())
                    .unwrap_or("absent")
                    .to_string();
                let bytes = <Bytes as FromRequest<()>>::from_request(req, &())
                    .await
                    .unwrap();
                let body = format!("len={cl}|body={}", String::from_utf8_lossy(&bytes));
                Ok(http::Response::builder()
                    .status(http::StatusCode::OK)
                    .body(Body::from(body))
                    .unwrap())
            })
        }
    }

    /// Size-changing transform from the module docs' own example.
    fn append_footer(bytes: Bytes) -> Pin<Box<dyn Future<Output = Bytes> + Send>> {
        Box::pin(async move {
            let mut out = bytes.to_vec();
            out.extend_from_slice(b"!!");
            Bytes::from(out)
        })
    }

    #[tokio::test]
    async fn response_content_length_tracks_size_changing_transform() {
        let mut svc = MapResponseBodyLayer::new(append_footer).layer(FixedResponseService);
        let res = svc
            .call(Request::new(
                http::Request::builder()
                    .uri("/")
                    .body(Body::empty())
                    .unwrap(),
            ))
            .await
            .unwrap();

        assert_eq!(res.status(), http::StatusCode::OK);
        let cl: usize = res
            .headers()
            .get(http::header::CONTENT_LENGTH)
            .unwrap()
            .to_str()
            .unwrap()
            .parse()
            .unwrap();
        assert_eq!(cl, 7, "Content-Length must describe the transformed body");
        assert_eq!(Body::to_string(res.into_body()).await.unwrap(), "hello!!");
    }

    #[tokio::test]
    async fn request_content_length_tracks_size_changing_transform() {
        let mut svc = MapRequestBodyLayer::new(append_footer).layer(EchoBodyService);
        let res = svc
            .call(Request::new(
                http::Request::builder()
                    .method(http::Method::POST)
                    .uri("/")
                    .header(http::header::CONTENT_LENGTH, "3")
                    .body(Body::from(Bytes::from_static(b"abc")))
                    .unwrap(),
            ))
            .await
            .unwrap();

        let text = Body::to_string(res.into_body()).await.unwrap();
        assert_eq!(text, "len=5|body=abc!!");
    }
}
