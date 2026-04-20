//! Type-erased Tower service and layer infrastructure.
//!
//! Provides [`BoxCloneService`] — a heap-allocated, clone-friendly
//! service wrapper for `Service<Request, Response = Response, Error = Infallible>` —
//! and the [`LayerFn`] type alias that `Router` and `MethodRouter` use to
//! store type-erased Tower layers.
//!
//! # Design
//!
//! We roll our own `BoxCloneService` instead of depending on `tower::util::BoxCloneService`
//! to keep the public API stable across Tower minor versions and to avoid
//! the `tower/full` feature requirement.

use std::convert::Infallible;
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;
use std::task::{Context, Poll};

use ajaya_core::{Request, Response};
use tower_layer::Layer;
use tower_service::Service;

// ── Public types ────────────────────────────────────────────────────────────

/// A type-erased, cheaply-cloneable Tower service for Ajaya's request/response pair.
///
/// Wraps any `Service<Request, Response = Response, Error = Infallible>` that is
/// `Clone + Send + 'static` with a `Send + 'static` future.
pub struct BoxCloneService(Box<dyn ErasedSvc>);

/// A closure that applies a Tower layer to a [`BoxCloneService`] and returns a new one.
///
/// Stored inside [`Router`] and [`MethodRouter`] to defer layer application until
/// the service is actually built at serve time.
///
/// ```rust,ignore
/// // Created by Router::layer():
/// Arc::new(move |svc: BoxCloneService| BoxCloneService::new(my_layer.clone().layer(svc)))
/// ```
pub type LayerFn = Arc<dyn Fn(BoxCloneService) -> BoxCloneService + Send + Sync + 'static>;

// ── BoxCloneService ─────────────────────────────────────────────────────────

/// Object-safe inner trait.
trait ErasedSvc: Send {
    fn call_erased(
        &mut self,
        req: Request,
    ) -> Pin<Box<dyn Future<Output = Result<Response, Infallible>> + Send + 'static>>;

    fn poll_ready_erased(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Infallible>>;

    fn clone_box(&self) -> Box<dyn ErasedSvc>;
}

impl<S> ErasedSvc for S
where
    S: Service<Request, Response = Response, Error = Infallible> + Clone + Send + 'static,
    S::Future: Send + 'static,
{
    fn call_erased(
        &mut self,
        req: Request,
    ) -> Pin<Box<dyn Future<Output = Result<Response, Infallible>> + Send + 'static>> {
        Box::pin(Service::call(self, req))
    }

    fn poll_ready_erased(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Infallible>> {
        Service::poll_ready(self, cx)
    }

    fn clone_box(&self) -> Box<dyn ErasedSvc> {
        Box::new(self.clone())
    }
}

impl BoxCloneService {
    /// Wrap a concrete service in a `BoxCloneService`.
    pub fn new<S>(svc: S) -> Self
    where
        S: Service<Request, Response = Response, Error = Infallible> + Clone + Send + 'static,
        S::Future: Send + 'static,
    {
        Self(Box::new(svc))
    }
}

impl Clone for BoxCloneService {
    fn clone(&self) -> Self {
        Self(self.0.clone_box())
    }
}

impl Service<Request> for BoxCloneService {
    type Response = Response;
    type Error = Infallible;
    type Future = Pin<Box<dyn Future<Output = Result<Response, Infallible>> + Send + 'static>>;

    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.0.poll_ready_erased(cx)
    }

    fn call(&mut self, req: Request) -> Self::Future {
        self.0.call_erased(req)
    }
}

impl std::fmt::Debug for BoxCloneService {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("BoxCloneService").finish_non_exhaustive()
    }
}

// ── Layer helpers ───────────────────────────────────────────────────────────

/// Convert any Tower [`Layer`] into a [`LayerFn`].
///
/// Used by [`Router::layer`], [`Router::route_layer`], and [`MethodRouter::layer`].
///
/// # Bounds
///
/// The resulting service `L::Service` must be:
/// - `Service<Request, Response = Response, Error = Infallible>`
/// - `Clone + Send + 'static`
/// - Its future must be `Send + 'static`
pub fn into_layer_fn<L>(layer: L) -> LayerFn
where
    L: Layer<BoxCloneService> + Clone + Send + Sync + 'static,
    L::Service: Service<Request, Response = Response, Error = Infallible> + Clone + Send + 'static,
    <L::Service as Service<Request>>::Future: Send + 'static,
{
    Arc::new(move |svc: BoxCloneService| -> BoxCloneService {
        BoxCloneService::new(layer.clone().layer(svc))
    })
}

/// Apply all layers in order (first added = innermost) to `base`.
///
/// Given `layers = [A, B]` and a `base` service, produces `B(A(base))`.
/// When a request arrives it will be processed by B first, then A, then base.
/// This matches `.layer(A).layer(B)` ordering.
#[inline]
pub fn apply_layers(base: BoxCloneService, layers: &[LayerFn]) -> BoxCloneService {
    layers.iter().fold(base, |svc, f| f(svc))
}

/// Poll `svc` ready, then call it with `req`, returning the response.
///
/// All services in Ajaya return `Poll::Ready(Ok(()))` immediately; this
/// helper is provided for correctness and future-proofing.
pub async fn oneshot(mut svc: BoxCloneService, req: Request) -> Response {
    use std::future::poll_fn;

    // Poll ready (our services return immediately but this is correct Tower usage)
    poll_fn(|cx| svc.poll_ready(cx))
        .await
        .unwrap_or_else(|infallible| match infallible {});

    svc.call(req)
        .await
        .unwrap_or_else(|infallible| match infallible {})
}
