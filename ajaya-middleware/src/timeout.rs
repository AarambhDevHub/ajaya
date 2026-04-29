//! Request timeout middleware.
//!
//! Returns `408 Request Timeout` if the inner service doesn't respond
//! within the configured duration.
//!
//! # Example
//!
//! ```rust,ignore
//! use ajaya_middleware::timeout::TimeoutLayer;
//! use std::time::Duration;
//!
//! Router::new()
//!     .route("/slow", get(slow_handler))
//!     .layer(TimeoutLayer::new(Duration::from_secs(30)));
//! ```

use std::convert::Infallible;
use std::future::Future;
use std::pin::Pin;
use std::task::{Context, Poll};
use std::time::Duration;

use ajaya_core::{Body, Request, Response};
use http::StatusCode;
use tower_layer::Layer;
use tower_service::Service;

/// Tower layer that enforces a request timeout.
#[derive(Debug, Clone)]
pub struct TimeoutLayer {
    duration: Duration,
}

impl TimeoutLayer {
    /// Create a new `TimeoutLayer` with the given duration.
    ///
    /// Returns `408 Request Timeout` if the handler doesn't complete
    /// within this duration.
    pub fn new(duration: Duration) -> Self {
        Self { duration }
    }
}

impl<S> Layer<S> for TimeoutLayer {
    type Service = TimeoutService<S>;

    fn layer(&self, inner: S) -> Self::Service {
        TimeoutService {
            inner,
            duration: self.duration,
        }
    }
}

/// Tower service produced by [`TimeoutLayer`].
#[derive(Clone)]
pub struct TimeoutService<S> {
    inner: S,
    duration: Duration,
}

impl<S> Service<Request> for TimeoutService<S>
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
        let duration = self.duration;

        Box::pin(async move {
            match tokio::time::timeout(duration, inner.call(req)).await {
                Ok(result) => result,
                Err(_elapsed) => {
                    tracing::warn!("Request timed out after {:?}", duration);
                    Ok(http::Response::builder()
                        .status(StatusCode::REQUEST_TIMEOUT)
                        .header(http::header::CONTENT_TYPE, "application/json")
                        .body(Body::from(format!(
                            r#"{{"error":"Request Timeout","code":408,"message":"Request exceeded the {}ms time limit"}}"#,
                            duration.as_millis()
                        )))
                        .unwrap())
                }
            }
        })
    }
}
