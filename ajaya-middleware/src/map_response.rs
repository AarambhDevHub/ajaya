//! ## map_response
//!
//! Create middleware that **transforms only the response** after the handler
//! runs. The request is passed through unchanged.
//!
//! Slightly more efficient than [`from_fn`] when you only need to mutate
//! the response (no request inspection).

use std::convert::Infallible;
use std::pin::Pin;
use std::task::{Context, Poll};

use ajaya_core::{IntoResponse, Request, Response};
use tower_layer::Layer;
use tower_service::Service;

// ─────────────────────────────────────────────────────────────────────────────
// map_response
// ─────────────────────────────────────────────────────────────────────────────

/// Create middleware that **transforms only the response** after the handler
/// runs. The request is passed through unchanged.
///
/// Slightly more efficient than [`from_fn`] when you only need to mutate
/// the response (no request inspection).
///
/// ### Example
///
/// ```rust,ignore
/// use ajaya::middleware::map_response;
///
/// // Append security headers to every response
/// Router::new().layer(map_response(|mut res: Response| async move {
///     res.headers_mut().insert("x-powered-by", "ajaya".parse().unwrap());
///     res
/// }));
/// ```
pub fn map_response<F>(f: F) -> MapResponseLayer<F> {
    MapResponseLayer { f }
}

/// Tower [`Layer`] produced by [`map_response`].
#[derive(Clone, Debug)]
pub struct MapResponseLayer<F> {
    f: F,
}

impl<F, Fut, Res, Svc> Layer<Svc> for MapResponseLayer<F>
where
    F: Fn(Response) -> Fut + Clone + Send + Sync + 'static,
    Fut: std::future::Future<Output = Res> + Send + 'static,
    Res: IntoResponse,
    Svc: Service<Request, Response = Response, Error = Infallible> + Clone + Send + 'static,
    Svc::Future: Send + 'static,
{
    type Service = MapResponseService<F, Svc>;

    fn layer(&self, inner: Svc) -> Self::Service {
        MapResponseService {
            f: self.f.clone(),
            inner,
        }
    }
}

/// Tower [`Service`] produced by [`MapResponseLayer`].
#[derive(Clone)]
pub struct MapResponseService<F, Svc> {
    f: F,
    inner: Svc,
}

impl<F, Svc> std::fmt::Debug for MapResponseService<F, Svc> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("MapResponseService")
            .field("f", &std::any::type_name::<F>())
            .finish_non_exhaustive()
    }
}

impl<F, Fut, Res, Svc> Service<Request> for MapResponseService<F, Svc>
where
    F: Fn(Response) -> Fut + Clone + Send + Sync + 'static,
    Fut: std::future::Future<Output = Res> + Send + 'static,
    Res: IntoResponse,
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
            let res = inner.call(req).await.unwrap_or_else(|i| match i {});
            Ok(f(res).await.into_response())
        })
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// map_response_with_state
// ─────────────────────────────────────────────────────────────────────────────

/// Create middleware that transforms the response and has access to shared
/// application state.
///
/// ### Example
///
/// ```rust,ignore
/// use ajaya::middleware::map_response_with_state;
///
/// #[derive(Clone)]
/// struct AppState { version: String }
///
/// let state = AppState { version: "1.0.0".into() };
/// Router::new()
///     .layer(map_response_with_state(state.clone(), |state: AppState, mut res: Response| async move {
///         res.headers_mut().insert("x-api-version", state.version.parse().unwrap());
///         res
///     }))
///     .with_state(state);
/// ```
pub fn map_response_with_state<S, F>(state: S, f: F) -> MapResponseWithStateLayer<S, F>
where
    S: Clone + Send + Sync + 'static,
{
    MapResponseWithStateLayer { f, state }
}

/// Tower [`Layer`] produced by [`map_response_with_state`].
#[derive(Clone, Debug)]
pub struct MapResponseWithStateLayer<S, F> {
    f: F,
    state: S,
}

impl<S, F, Fut, Res, Svc> Layer<Svc> for MapResponseWithStateLayer<S, F>
where
    S: Clone + Send + Sync + 'static,
    F: Fn(S, Response) -> Fut + Clone + Send + Sync + 'static,
    Fut: std::future::Future<Output = Res> + Send + 'static,
    Res: IntoResponse,
    Svc: Service<Request, Response = Response, Error = Infallible> + Clone + Send + 'static,
    Svc::Future: Send + 'static,
{
    type Service = MapResponseWithStateService<S, F, Svc>;

    fn layer(&self, inner: Svc) -> Self::Service {
        MapResponseWithStateService {
            f: self.f.clone(),
            state: self.state.clone(),
            inner,
        }
    }
}

/// Tower [`Service`] produced by [`MapResponseWithStateLayer`].
#[derive(Clone)]
pub struct MapResponseWithStateService<S, F, Svc> {
    f: F,
    state: S,
    inner: Svc,
}

impl<S, F, Svc> std::fmt::Debug for MapResponseWithStateService<S, F, Svc> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("MapResponseWithStateService")
            .field("f", &std::any::type_name::<F>())
            .finish_non_exhaustive()
    }
}

impl<S, F, Fut, Res, Svc> Service<Request> for MapResponseWithStateService<S, F, Svc>
where
    S: Clone + Send + Sync + 'static,
    F: Fn(S, Response) -> Fut + Clone + Send + Sync + 'static,
    Fut: std::future::Future<Output = Res> + Send + 'static,
    Res: IntoResponse,
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
            let res = inner.call(req).await.unwrap_or_else(|i| match i {});
            Ok(f(state, res).await.into_response())
        })
    }
}
