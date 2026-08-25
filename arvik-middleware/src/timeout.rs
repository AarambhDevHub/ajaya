//! Request timeout middleware.
//!
//! Returns `408 Request Timeout` if the inner service doesn't respond
//! within the configured duration.
//!
//! # Example
//!
//! ```rust,ignore
//! use arvik_middleware::timeout::TimeoutLayer;
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

use arvik_core::{Body, Request, Response};
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
    type Future = TimeoutFuture<S>;

    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.inner.poll_ready(cx)
    }

    fn call(&mut self, req: Request) -> Self::Future {
        let cloned = self.inner.clone();
        let mut inner = std::mem::replace(&mut self.inner, cloned);

        TimeoutFuture {
            fut: inner.call(req),
            sleep: tokio::time::sleep(self.duration),
            duration: self.duration,
        }
    }
}

// Concrete future for [`TimeoutService`] — polls the handler and a deadline
// side-by-side with no heap allocation per request (audit MW2).
pin_project_lite::pin_project! {
    pub struct TimeoutFuture<S>
    where
        S: Service<Request, Response = Response, Error = Infallible>,
    {
        #[pin]
        fut: S::Future,
        #[pin]
        sleep: tokio::time::Sleep,
        duration: Duration,
    }
}

impl<S> Future for TimeoutFuture<S>
where
    S: Service<Request, Response = Response, Error = Infallible>,
    S::Future: Send + 'static,
{
    type Output = Result<Response, Infallible>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let mut this = self.project();
        // Handler wins ties.
        if let Poll::Ready(result) = this.fut.as_mut().poll(cx) {
            return match result {
                Ok(response) => Poll::Ready(Ok(response)),
                Err(e) => match e {},
            };
        }

        if Future::poll(this.sleep.as_mut(), cx).is_ready() {
            tracing::warn!("Request timed out after {:?}", this.duration);
            return Poll::Ready(Ok(
                http::Response::builder()
                    .status(StatusCode::REQUEST_TIMEOUT)
                    .header(http::header::CONTENT_TYPE, "application/json")
                    .body(Body::from(format!(
                        r#"{{"error":"Request Timeout","code":408,"message":"Request exceeded the {}ms time limit"}}"#,
                        this.duration.as_millis()
                    )))
                    .unwrap()
            ));
        }

        Poll::Pending
    }
}
