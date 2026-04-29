//! Panic recovery middleware.
//!
//! Catches panics in handlers and returns a `500 Internal Server Error`
//! response instead of crashing the server task.
//!
//! Uses `tokio::task::spawn` under the hood — if the spawned task panics,
//! the `JoinError` is caught and converted to a 500 response.
//!
//! # Example
//!
//! ```rust,ignore
//! use ajaya_middleware::catch_panic::CatchPanicLayer;
//!
//! Router::new()
//!     .route("/unsafe", get(possibly_panicking_handler))
//!     .layer(CatchPanicLayer::new());
//!
//! // Custom panic response
//! Router::new()
//!     .layer(CatchPanicLayer::custom(|_panic_info| {
//!         (StatusCode::INTERNAL_SERVER_ERROR, "Something went very wrong")
//!             .into_response()
//!     }));
//! ```

use std::convert::Infallible;
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;
use std::task::{Context, Poll};

use ajaya_core::into_response::IntoResponse;
use ajaya_core::{Body, Request, Response};
use http::StatusCode;
use tower_layer::Layer;
use tower_service::Service;

type PanicHandler = Arc<dyn Fn() -> Response + Send + Sync + 'static>;

fn default_panic_response() -> Response {
    http::Response::builder()
        .status(StatusCode::INTERNAL_SERVER_ERROR)
        .header(http::header::CONTENT_TYPE, "application/json")
        .body(Body::from(
            r#"{"error":"Internal Server Error","code":500}"#,
        ))
        .unwrap()
}

/// Tower layer that catches handler panics and returns a 500 response.
#[derive(Clone, Default)]
pub struct CatchPanicLayer {
    panic_handler: Option<PanicHandler>,
}

impl CatchPanicLayer {
    /// Create a `CatchPanicLayer` that returns a default 500 response on panic.
    pub fn new() -> Self {
        Self::default()
    }

    /// Create a `CatchPanicLayer` with a custom panic response.
    ///
    /// The provided closure is called when a handler panics and should
    /// return the response to send to the client.
    pub fn custom<F, R>(handler: F) -> Self
    where
        F: Fn() -> R + Send + Sync + 'static,
        R: IntoResponse,
    {
        Self {
            panic_handler: Some(Arc::new(move || handler().into_response())),
        }
    }
}

impl<S> Layer<S> for CatchPanicLayer {
    type Service = CatchPanicService<S>;

    fn layer(&self, inner: S) -> Self::Service {
        CatchPanicService {
            inner,
            panic_handler: self.panic_handler.clone(),
        }
    }
}

/// Tower service produced by [`CatchPanicLayer`].
#[derive(Clone)]
pub struct CatchPanicService<S> {
    inner: S,
    panic_handler: Option<PanicHandler>,
}

impl<S> Service<Request> for CatchPanicService<S>
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
        let panic_handler = self.panic_handler.clone();

        Box::pin(async move {
            // Spawn onto a new task so we can catch panics via JoinError
            let handle = tokio::task::spawn(async move { inner.call(req).await });

            match handle.await {
                Ok(Ok(response)) => Ok(response),
                Ok(Err(infallible)) => match infallible {},
                Err(join_error) => {
                    let panic_payload = join_error.into_panic();

                    let panic_msg = panic_payload
                        .downcast_ref::<&str>()
                        .copied()
                        .or_else(|| panic_payload.downcast_ref::<String>().map(String::as_str))
                        .unwrap_or("unknown panic");

                    tracing::error!(panic = %panic_msg, "Handler panicked");

                    let response = match &panic_handler {
                        Some(handler) => handler(),
                        None => default_panic_response(),
                    };

                    Ok(response)
                }
            }
        })
    }
}
