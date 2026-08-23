//! Panic recovery middleware.
//!
//! Catches panics in handlers and returns a `500 Internal Server Error`
//! response instead of crashing the connection task.
//!
//! Wraps the handler future in `catch_unwind` directly — zero per-request
//! overhead. Note: panics raised later, while *polling the response body
//! stream*, happen outside this wrapper (the body is driven by hyper after
//! the response is returned).
//!
//! # Example
//!
//! ```rust,ignore
//! use arvik_middleware::catch_panic::CatchPanicLayer;
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

use futures_util::FutureExt as _;
use std::convert::Infallible;
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;
use std::task::{Context, Poll};

use arvik_core::into_response::IntoResponse;
use arvik_core::{Body, Request, Response};
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
            // catch_unwind on the future directly — no per-request task spawn,
            // which cost an allocation + scheduling round-trip on 100% of
            // requests to handle the rare panic case.
            //
            // AssertUnwindSafe: the tower service contract (clone-per-call,
            // infallible) already treats inner state as recoverable.
            match std::panic::AssertUnwindSafe(inner.call(req))
                .catch_unwind()
                .await
            {
                Ok(Ok(response)) => Ok(response),
                Ok(Err(infallible)) => match infallible {},
                Err(panic_payload) => {
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
