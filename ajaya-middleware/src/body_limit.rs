//! Request body size limit middleware.
//!
//! Enforces a maximum request body size. Checks the `Content-Length`
//! header immediately; for streaming bodies without `Content-Length`,
//! enforces the limit during body collection.
//!
//! Returns `413 Payload Too Large` when the limit is exceeded.
//!
//! # Example
//!
//! ```rust,ignore
//! use ajaya_middleware::body_limit::RequestBodyLimitLayer;
//!
//! // 10MB limit
//! Router::new()
//!     .layer(RequestBodyLimitLayer::new(10 * 1024 * 1024));
//! ```

use std::convert::Infallible;
use std::future::Future;
use std::pin::Pin;
use std::task::{Context, Poll};

use ajaya_core::{Body, Request, Response};
use http::StatusCode;
use tokio_util::bytes::{Bytes, BytesMut};
use tower_layer::Layer;
use tower_service::Service;

/// Tower layer that enforces a maximum request body size.
#[derive(Debug, Clone, Copy)]
pub struct RequestBodyLimitLayer {
    limit: usize,
}

impl RequestBodyLimitLayer {
    /// Create a new `RequestBodyLimitLayer` with the given byte limit.
    pub fn new(limit: usize) -> Self {
        Self { limit }
    }
}

impl<S> Layer<S> for RequestBodyLimitLayer {
    type Service = RequestBodyLimitService<S>;

    fn layer(&self, inner: S) -> Self::Service {
        RequestBodyLimitService {
            inner,
            limit: self.limit,
        }
    }
}

/// Tower service produced by [`RequestBodyLimitLayer`].
#[derive(Clone)]
pub struct RequestBodyLimitService<S> {
    inner: S,
    limit: usize,
}

impl<S> Service<Request> for RequestBodyLimitService<S>
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
        let limit = self.limit;

        let cloned = self.inner.clone();
        let mut inner = std::mem::replace(&mut self.inner, cloned);

        Box::pin(async move {
            // Fast-path: reject based on Content-Length header immediately.
            if let Some(content_length) = req
                .headers()
                .get(http::header::CONTENT_LENGTH)
                .and_then(|v| v.to_str().ok())
                .and_then(|s| s.parse::<usize>().ok())
            {
                if content_length > limit {
                    tracing::warn!(
                        content_length = content_length,
                        limit = limit,
                        "Request body exceeds size limit (Content-Length check)"
                    );
                    return Ok::<Response, Infallible>(payload_too_large(limit));
                }
            }

            // For streaming bodies: collect up to limit bytes.
            let (parts, body) = req.into_request_parts();

            match collect_limited(body, limit).await {
                Ok(bytes) => {
                    let req = Request::from_request_parts(parts, Body::from_bytes(bytes));
                    inner.call(req).await
                }
                Err(()) => {
                    tracing::warn!(
                        limit = limit,
                        "Request body exceeds size limit (stream check)"
                    );
                    Ok::<Response, Infallible>(payload_too_large(limit))
                }
            }
        })
    }
}

/// Collect up to `limit` bytes from `body`.
///
/// Returns `Err(())` if the body exceeds the limit.
///
/// FIX: replaced the previous `futures_util::future::poll_fn` + manual
/// pin projection with a straightforward loop using `std::future::poll_fn`
/// and `Pin::new_unchecked` — body is `Unpin` so this is safe — or simply
/// box the body to get a stable address.
async fn collect_limited(body: Body, limit: usize) -> Result<Bytes, ()> {
    use http_body::Body as HttpBodyTrait;

    // Box the body so we can pin it without needing the caller to be Unpin.
    let mut pinned = Box::pin(body);
    let mut buf = BytesMut::new();

    loop {
        match std::future::poll_fn(|cx| pinned.as_mut().poll_frame(cx)).await {
            Some(Ok(frame)) => {
                if let Ok(data) = frame.into_data() {
                    buf.extend_from_slice(&data);
                    if buf.len() > limit {
                        return Err(());
                    }
                }
                // Trailers frames are silently skipped.
            }
            Some(Err(_)) => return Err(()),
            None => break,
        }
    }

    Ok(buf.freeze())
}

fn payload_too_large(limit: usize) -> Response {
    http::Response::builder()
        .status(StatusCode::PAYLOAD_TOO_LARGE)
        .header(http::header::CONTENT_TYPE, "application/json")
        .body(Body::from(format!(
            r#"{{"error":"Payload Too Large","code":413,"limit_bytes":{}}}"#,
            limit
        )))
        .unwrap()
}
