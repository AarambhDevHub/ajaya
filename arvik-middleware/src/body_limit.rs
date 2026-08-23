//! Request body size limit middleware.
//!
//! Enforces a maximum request body size **without buffering**: requests whose
//! `Content-Length` exceeds the limit are rejected immediately; streaming
//! bodies are passed through wrapped in a counting frame filter that errors
//! once more than `limit` bytes have been read (readers surface that as
//! `413 Payload Too Large`).
//!
//! To configure the *extractor-side* limit instead (`Bytes`, `String`, `Json`,
//! `Form`), see [`DefaultBodyLimitLayer`].
//!
//! # Example
//!
//! ```rust,ignore
//! use arvik_middleware::body_limit::RequestBodyLimitLayer;
//!
//! // 10MB limit
//! Router::new()
//!     .layer(RequestBodyLimitLayer::new(10 * 1024 * 1024));
//! ```

use std::convert::Infallible;
use std::future::Future;
use std::pin::Pin;
use std::task::{Context, Poll};

use arvik_core::body_limit::DefaultBodyLimit;
use arvik_core::{Body, Request, Response};
use http::StatusCode;
use http_body::Frame;
use tokio_util::bytes::Bytes;
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

            // For streaming bodies: pass the body through unbuffered with a
            // counting cap — readers get an error once the limit is crossed.
            let (parts, body) = req.into_request_parts();
            let limited = Body::new(LimitedBody {
                inner: body,
                limit: limit as u64,
                read: 0,
            });
            let req = Request::from_request_parts(parts, limited);
            inner.call(req).await
        })
    }
}

/// Streaming frame filter that errors once more than `limit` bytes have been
/// read. Frames themselves are passed through untouched — nothing is buffered.
struct LimitedBody {
    inner: Body,
    limit: u64,
    read: u64,
}

impl http_body::Body for LimitedBody {
    type Data = Bytes;
    type Error = arvik_core::body::BoxError;

    fn poll_frame(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<Option<Result<Frame<Self::Data>, Self::Error>>> {
        let this = &mut *self;
        match Pin::new(&mut this.inner).poll_frame(cx) {
            Poll::Ready(Some(Ok(frame))) => match frame.into_data() {
                Ok(data) => {
                    this.read += data.len() as u64;
                    if this.read > this.limit {
                        return Poll::Ready(Some(Err(Box::new(
                            arvik_core::body_limit::BodyTooLarge,
                        ))));
                    }
                    Poll::Ready(Some(Ok(Frame::data(data))))
                }
                // Trailers frames pass through unchanged.
                Err(frame) => Poll::Ready(Some(Ok(frame))),
            },
            other => other,
        }
    }

    fn is_end_stream(&self) -> bool {
        self.inner.is_end_stream()
    }

    fn size_hint(&self) -> http_body::SizeHint {
        self.inner.size_hint()
    }
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

// ── DefaultBodyLimitLayer ────────────────────────────────────────────────────
//
// Sets the size limit that *buffering extractors* (Bytes, String, Json, Form)
// enforce, by inserting a `DefaultBodyLimit` extension into each request.
// Unlike `RequestBodyLimitLayer`, this layer reads nothing itself — the
// extractors enforce the limit while collecting.

/// Tower layer that configures the extractor-side body limit.
///
/// Buffering extractors accept up to `limit` bytes instead of the 2 MiB
/// default ([`arvik_core::DEFAULT_BODY_LIMIT`]); larger requests are rejected
/// with `413 Payload Too Large` by the extractor itself.
///
/// ```rust,ignore
/// use arvik_middleware::body_limit::DefaultBodyLimitLayer;
///
/// // Allow 16 MiB JSON bodies on this route.
/// Router::new()
///     .route("/upload", post(upload))
///     .layer(DefaultBodyLimitLayer::max(16 * 1024 * 1024));
/// ```
#[derive(Debug, Clone, Copy)]
pub struct DefaultBodyLimitLayer {
    limit: usize,
}

impl DefaultBodyLimitLayer {
    /// Allow buffering extractors to accept up to `limit` bytes.
    pub fn max(limit: usize) -> Self {
        Self { limit }
    }

    /// Disable the extractor-side limit for wrapped routes.
    ///
    /// Only sensible behind another protection (e.g. an upstream proxy
    /// enforcing its own size cap).
    pub fn disabled() -> Self {
        Self { limit: usize::MAX }
    }
}

impl<S> Layer<S> for DefaultBodyLimitLayer {
    type Service = DefaultBodyLimitService<S>;

    fn layer(&self, inner: S) -> Self::Service {
        DefaultBodyLimitService {
            inner,
            limit: self.limit,
        }
    }
}

/// Tower service produced by [`DefaultBodyLimitLayer`].
#[derive(Clone)]
pub struct DefaultBodyLimitService<S> {
    inner: S,
    limit: usize,
}

impl<S> Service<Request> for DefaultBodyLimitService<S>
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
        let cloned = self.inner.clone();
        let mut inner = std::mem::replace(&mut self.inner, cloned);

        req.extensions_mut().insert(DefaultBodyLimit(self.limit));
        Box::pin(inner.call(req))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use arvik_core::IntoResponse;
    use arvik_core::body_limit::DefaultBodyLimit;
    use arvik_core::extract::FromRequest;

    /// Inner service that reads the whole body via the `Bytes` extractor and
    /// reports its length — with the extractor-side default disabled so this
    /// suite exercises *this layer's* enforcement.
    #[derive(Clone)]
    struct ReadBodyService;

    impl Service<Request> for ReadBodyService {
        type Response = Response;
        type Error = Infallible;
        type Future = Pin<Box<dyn Future<Output = Result<Response, Infallible>> + Send + 'static>>;

        fn poll_ready(&mut self, _: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
            Poll::Ready(Ok(()))
        }

        fn call(&mut self, mut req: Request) -> Self::Future {
            Box::pin(async move {
                req.extensions_mut().insert(DefaultBodyLimit::disabled());
                let resp = match <Bytes as FromRequest<()>>::from_request(req, &()).await {
                    Ok(b) => http::Response::builder()
                        .status(StatusCode::OK)
                        .body(Body::from(format!("len={}", b.len())))
                        .unwrap(),
                    Err(err) => err.into_response(),
                };
                Ok(resp)
            })
        }
    }

    fn chunked_request(chunks: &[&[u8]]) -> Request {
        Request::new(
            http::Request::builder()
                .method(http::Method::POST)
                .uri("/")
                .header(http::header::TRANSFER_ENCODING, "chunked")
                .body(Body::from_chunks(chunks.iter().map(|c| c.to_vec())))
                .unwrap(),
        )
    }

    #[tokio::test]
    async fn streams_body_through_intact_under_the_limit() {
        let mut svc = RequestBodyLimitLayer::new(64).layer(ReadBodyService);
        // Two chunks prove pass-through: nothing is reassembled by the layer.
        let res = svc
            .call(chunked_request(&[b"hello ", b"world"]))
            .await
            .unwrap();
        assert_eq!(res.status(), StatusCode::OK);
        assert_eq!(Body::to_string(res.into_body()).await.unwrap(), "len=11");
    }

    #[tokio::test]
    async fn streaming_body_over_limit_yields_413_from_reader() {
        let mut svc = RequestBodyLimitLayer::new(8).layer(ReadBodyService);
        let res = svc
            .call(chunked_request(&[b"0123456789", b"abcdefghij"]))
            .await
            .unwrap();
        assert_eq!(res.status(), StatusCode::PAYLOAD_TOO_LARGE);
    }

    #[tokio::test]
    async fn oversized_content_length_is_rejected_before_reading() {
        let mut svc = RequestBodyLimitLayer::new(8).layer(ReadBodyService);
        let req = Request::new(
            http::Request::builder()
                .method(http::Method::POST)
                .uri("/")
                .header(http::header::CONTENT_LENGTH, "1000")
                .body(Body::from(Bytes::from_static(b"tiny")))
                .unwrap(),
        );
        let res = svc.call(req).await.unwrap();
        assert_eq!(res.status(), StatusCode::PAYLOAD_TOO_LARGE);
    }
}
