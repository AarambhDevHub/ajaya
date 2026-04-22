//! ## map_request
//!
//! Create middleware that **transforms only the request** before it reaches
//! the handler. The response is returned unchanged.
//!
//! Slightly more efficient than [`from_fn`] when you only need to mutate
//! the request (no response inspection).

use std::convert::Infallible;
use std::pin::Pin;
use std::task::{Context, Poll};

use ajaya_core::{Request, Response};
use tower_layer::Layer;
use tower_service::Service;

// ─────────────────────────────────────────────────────────────────────────────
// map_request
// ──────────────────────────────────────────────────────────────��──────────────

/// Create middleware that **transforms only the request** before it reaches
/// the handler. The response is returned unchanged.
///
/// Slightly more efficient than [`from_fn`] when you only need to mutate
/// the request (no response inspection).
///
/// ### Example
///
/// ```rust,ignore
/// use ajaya::middleware::map_request;
///
/// // Normalise Accept header
/// Router::new().layer(map_request(|mut req: Request| async move {
///     req.headers_mut().insert(
///         http::header::ACCEPT,
///         "application/json".parse().unwrap(),
///     );
///     req
/// }));
/// ```
pub fn map_request<F>(f: F) -> MapRequestLayer<F> {
    MapRequestLayer { f }
}

/// Tower [`Layer`] produced by [`map_request`].
#[derive(Clone, Debug)]
pub struct MapRequestLayer<F> {
    f: F,
}

impl<F, Fut, Svc> Layer<Svc> for MapRequestLayer<F>
where
    F: Fn(Request) -> Fut + Clone + Send + Sync + 'static,
    Fut: std::future::Future<Output = Request> + Send + 'static,
    Svc: Service<Request, Response = Response, Error = Infallible> + Clone + Send + 'static,
    Svc::Future: Send + 'static,
{
    type Service = MapRequestService<F, Svc>;

    fn layer(&self, inner: Svc) -> Self::Service {
        MapRequestService {
            f: self.f.clone(),
            inner,
        }
    }
}

/// Tower [`Service`] produced by [`MapRequestLayer`].
#[derive(Clone)]
pub struct MapRequestService<F, Svc> {
    f: F,
    inner: Svc,
}

impl<F, Svc> std::fmt::Debug for MapRequestService<F, Svc> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("MapRequestService")
            .field("f", &std::any::type_name::<F>())
            .finish_non_exhaustive()
    }
}

impl<F, Fut, Svc> Service<Request> for MapRequestService<F, Svc>
where
    F: Fn(Request) -> Fut + Clone + Send + Sync + 'static,
    Fut: std::future::Future<Output = Request> + Send + 'static,
    Svc: Service<Request, Response = Response, Error = Infallible> + Clone + Send + 'static,
    Svc::Future: Send + 'static,
{
    type Response = Response;
    type Error = Infallible;
    type Future =
        Pin<Box<dyn std::future::Future<Output = Result<Response, Infallible>> + Send + 'static>>;

    #[inline]
    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.inner.poll_ready(cx)
    }

    fn call(&mut self, req: Request) -> Self::Future {
        let cloned = self.inner.clone();
        let mut inner = std::mem::replace(&mut self.inner, cloned);
        let f = self.f.clone();
        Box::pin(async move {
            let req = f(req).await;
            inner.call(req).await
        })
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// map_request_with_state
// ─────────────────────────────────────────────────────────────────────────────

/// Create middleware that transforms the request and has access to shared
/// application state.
///
/// ### Example
///
/// ```rust,ignore
/// use ajaya::middleware::map_request_with_state;
///
/// #[derive(Clone)]
/// struct AppState { prefix: String }
///
/// let state = AppState { prefix: "/api/v1".into() };
/// Router::new()
///     .layer(map_request_with_state(state.clone(), |state: AppState, mut req: Request| async move {
///         tracing::debug!("prefix = {}", state.prefix);
///         req.extensions_mut().insert(state.prefix.clone());
///         req
///     }))
///     .with_state(state);
/// ```
pub fn map_request_with_state<S, F>(state: S, f: F) -> MapRequestWithStateLayer<S, F>
where
    S: Clone + Send + Sync + 'static,
{
    MapRequestWithStateLayer { f, state }
}

/// Tower [`Layer`] produced by [`map_request_with_state`].
#[derive(Clone, Debug)]
pub struct MapRequestWithStateLayer<S, F> {
    f: F,
    state: S,
}

impl<S, F, Fut, Svc> Layer<Svc> for MapRequestWithStateLayer<S, F>
where
    S: Clone + Send + Sync + 'static,
    F: Fn(S, Request) -> Fut + Clone + Send + Sync + 'static,
    Fut: std::future::Future<Output = Request> + Send + 'static,
    Svc: Service<Request, Response = Response, Error = Infallible> + Clone + Send + 'static,
    Svc::Future: Send + 'static,
{
    type Service = MapRequestWithStateService<S, F, Svc>;

    fn layer(&self, inner: Svc) -> Self::Service {
        MapRequestWithStateService {
            f: self.f.clone(),
            state: self.state.clone(),
            inner,
        }
    }
}

/// Tower [`Service`] produced by [`MapRequestWithStateLayer`].
#[derive(Clone)]
pub struct MapRequestWithStateService<S, F, Svc> {
    f: F,
    state: S,
    inner: Svc,
}

impl<S, F, Svc> std::fmt::Debug for MapRequestWithStateService<S, F, Svc> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("MapRequestWithStateService")
            .field("f", &std::any::type_name::<F>())
            .finish_non_exhaustive()
    }
}

impl<S, F, Fut, Svc> Service<Request> for MapRequestWithStateService<S, F, Svc>
where
    S: Clone + Send + Sync + 'static,
    F: Fn(S, Request) -> Fut + Clone + Send + Sync + 'static,
    Fut: std::future::Future<Output = Request> + Send + 'static,
    Svc: Service<Request, Response = Response, Error = Infallible> + Clone + Send + 'static,
    Svc::Future: Send + 'static,
{
    type Response = Response;
    type Error = Infallible;
    type Future =
        Pin<Box<dyn std::future::Future<Output = Result<Response, Infallible>> + Send + 'static>>;

    #[inline]
    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.inner.poll_ready(cx)
    }

    fn call(&mut self, req: Request) -> Self::Future {
        let cloned = self.inner.clone();
        let mut inner = std::mem::replace(&mut self.inner, cloned);
        let f = self.f.clone();
        let state = self.state.clone();
        Box::pin(async move {
            let req = f(state, req).await;
            inner.call(req).await
        })
    }
}
