//! ## FromFnLayer and FromFnService
//!
//! Tower [`Layer`] and [`Service`] produced by [`from_fn`] and [`from_fn_with_state`].
//!
//! Apply it to a [`Router`] via `.layer(from_fn(my_fn))`.
//! You typically do not interact with these types directly.

use std::marker::PhantomData;

use crate::{MiddlewareFn, Next};
use ajaya_core::{Request, Response};
use ajaya_router::layer::BoxCloneService;
use tower_layer::Layer;
use tower_service::Service;

// ─────────────────────────────────────────────────────────────────────────────
// from_fn (stateless — S = ())
// ───────────────────────────────────────────────────────��─────────────────────

/// Create a Tower middleware layer from a plain async function.
///
/// The function may accept any number of [`FromRequestParts`] extractors
/// as leading parameters, followed by `Request` and `Next`:
///
/// ```text
/// async fn name([T1, T2, …,] req: Request, next: Next) -> impl IntoResponse
/// ```
///
/// All extractors that work with state `()` are available without any
/// additional setup: [`Method`], [`Uri`], [`Version`], [`HeaderMap`],
/// [`Path<T>`], [`Query<T>`], [`MatchedPath`], [`RawPathParams`],
/// [`ConnectInfo<T>`], [`Extension<T>`], [`TypedHeader<T>`], [`CookieJar`].
///
/// For extractors that need shared application state — [`State<S>`],
/// [`SignedCookieJar`], [`PrivateCookieJar`] — use [`from_fn_with_state`].
///
/// ### Examples
///
/// Zero extractors (original style — still fully supported):
/// ```rust,ignore
/// async fn log(req: Request, next: Next) -> Response {
///     let path = req.uri().path().to_string();
///     let res = next.run(req).await;
///     tracing::info!("{} → {}", path, res.status());
///     res
/// }
///
/// Router::new().layer(from_fn(log));
/// ```
///
/// With extractors:
/// ```rust,ignore
/// use ajaya::{CookieJar, Path};
/// use ajaya::middleware::{from_fn, Next};
///
/// async fn check_cookie(
///     jar: CookieJar,
///     Path(id): Path<u32>,
///     req: Request,
///     next: Next,
/// ) -> impl IntoResponse {
///     if jar.get("token").is_some() {
///         next.run(req).await
///     } else {
///         StatusCode::UNAUTHORIZED.into_response()
///     }
/// }
///
/// Router::new()
///     .route("/item/{id}", get(handler))
///     .layer(from_fn(check_cookie));
/// ```
///
/// Closure middleware:
/// ```rust,ignore
/// let prefix = "/api/v1".to_string();
/// Router::new()
///     .layer(from_fn(move |req: Request, next: Next| {
///         let prefix = prefix.clone();
///         async move {
///             tracing::debug!("Request to prefix {}", prefix);
///             next.run(req).await
///         }
///     }));
/// ```
pub fn from_fn<F, T>(f: F) -> FromFnLayer<F, (), T>
where
    F: MiddlewareFn<(), T>,
{
    FromFnLayer {
        f,
        state: (),
        _marker: PhantomData,
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// from_fn_with_state
// ─────────────────────────────────────────────────────────────────────────────

/// Create a Tower middleware layer from an async function that has access to
/// shared application state through extractors.
///
/// The `state` provided here is used to resolve [`FromRequestParts<S>`]
/// extractors inside the middleware function — just as [`Router::with_state`]
/// provides state to handlers.
///
/// **Pass the same state object here as you pass to `.with_state()`** so
/// extractors like [`State<AppState>`], [`SignedCookieJar`], and
/// [`PrivateCookieJar`] work correctly.
///
/// ### Signature
///
/// ```text
/// async fn name([T1, T2, …,] req: Request, next: Next) -> impl IntoResponse
/// ```
///
/// Where each `Tₙ` implements `FromRequestParts<YourState>`.
///
/// ### Examples
///
/// State + typed header auth:
/// ```rust,ignore
/// use ajaya::{State, TypedHeader};
/// use ajaya::middleware::{from_fn_with_state, Next};
/// use headers::{Authorization, authorization::Bearer};
///
/// #[derive(Clone)]
/// struct AppState { api_keys: Vec<String> }
///
/// async fn require_api_key(
///     State(state): State<AppState>,
///     TypedHeader(auth): TypedHeader<Authorization<Bearer>>,
///     req: Request,
///     next: Next,
/// ) -> impl IntoResponse {
///     if state.api_keys.iter().any(|k| k == auth.token()) {
///         next.run(req).await
///     } else {
///         StatusCode::UNAUTHORIZED.into_response()
///     }
/// }
///
/// let state = AppState { api_keys: vec!["secret".into()] };
/// Router::new()
///     .route("/api", get(handler))
///     .layer(from_fn_with_state(state.clone(), require_api_key))
///     .with_state(state);
/// ```
///
/// Signed-cookie session:
/// ```rust,ignore
/// use ajaya::{SignedCookieJar, CookieKey, FromRef, Redirect};
///
/// #[derive(Clone)]
/// struct AppState { cookie_key: CookieKey }
///
/// impl FromRef<AppState> for CookieKey {
///     fn from_ref(s: &AppState) -> Self { s.cookie_key.clone() }
/// }
///
/// async fn session_required(
///     jar: SignedCookieJar,
///     req: Request,
///     next: Next,
/// ) -> impl IntoResponse {
///     match jar.get("user_id") {
///         Some(_) => next.run(req).await,
///         None    => Redirect::to("/login"),
///     }
/// }
///
/// let state = AppState { cookie_key: CookieKey::generate() };
/// Router::new()
///     .layer(from_fn_with_state(state.clone(), session_required))
///     .with_state(state);
/// ```
///
/// Role-based access control:
/// ```rust,ignore
/// #[derive(Clone)]
/// struct RbacState { allowed_roles: Vec<String> }
///
/// async fn require_role(
///     State(state): State<RbacState>,
///     req: Request,
///     next: Next,
/// ) -> impl IntoResponse {
///     let role = req.headers()
///         .get("x-user-role")
///         .and_then(|v| v.to_str().ok())
///         .unwrap_or("");
///
///     if state.allowed_roles.iter().any(|r| r == role) {
///         next.run(req).await
///     } else {
///         (StatusCode::FORBIDDEN, "Insufficient permissions").into_response()
///     }
/// }
/// ```
pub fn from_fn_with_state<S, F, T>(state: S, f: F) -> FromFnLayer<F, S, T>
where
    S: Clone + Send + Sync + 'static,
    F: MiddlewareFn<S, T>,
{
    FromFnLayer {
        f,
        state,
        _marker: PhantomData,
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// FromFnLayer
// ─────────────────────────────────────────────────────────────────────────────

/// Tower [`Layer`] produced by [`from_fn`] or [`from_fn_with_state`].
///
/// Apply it to a [`Router`] via `.layer(from_fn(my_fn))`.
/// You typically do not interact with this type directly.
pub struct FromFnLayer<F, S, T> {
    f: F,
    state: S,
    _marker: PhantomData<fn() -> T>,
}

impl<F: Clone, S: Clone, T> Clone for FromFnLayer<F, S, T> {
    fn clone(&self) -> Self {
        Self {
            f: self.f.clone(),
            state: self.state.clone(),
            _marker: PhantomData,
        }
    }
}

impl<F, S, T> std::fmt::Debug for FromFnLayer<F, S, T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("FromFnLayer")
            .field("f", &std::any::type_name::<F>())
            .field("state", &std::any::type_name::<S>())
            .finish()
    }
}

impl<F, S, T, Svc> Layer<Svc> for FromFnLayer<F, S, T>
where
    F: MiddlewareFn<S, T>,
    S: Clone + Send + Sync + 'static,
    Svc: Service<Request, Response = Response, Error = std::convert::Infallible>
        + Clone
        + Send
        + 'static,
    Svc::Future: Send + 'static,
{
    type Service = FromFnService<F, S, Svc, T>;

    fn layer(&self, inner: Svc) -> Self::Service {
        FromFnService {
            f: self.f.clone(),
            state: self.state.clone(),
            inner,
            _marker: PhantomData,
        }
    }
}

// ────────────────────────────────────────────────────���────────────────────────
// FromFnService
// ─────────────────────────────────────────────────────────────────────────────

/// Tower [`Service`] produced by [`FromFnLayer`].
///
/// You typically do not interact with this type directly.
pub struct FromFnService<F, S, Svc, T> {
    f: F,
    state: S,
    inner: Svc,
    _marker: PhantomData<fn() -> T>,
}

impl<F: Clone, S: Clone, Svc: Clone, T> Clone for FromFnService<F, S, Svc, T> {
    fn clone(&self) -> Self {
        Self {
            f: self.f.clone(),
            state: self.state.clone(),
            inner: self.inner.clone(),
            _marker: PhantomData,
        }
    }
}

impl<F, S, Svc, T> std::fmt::Debug for FromFnService<F, S, Svc, T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("FromFnService")
            .field("f", &std::any::type_name::<F>())
            .field("state", &std::any::type_name::<S>())
            .finish_non_exhaustive()
    }
}

impl<F, S, Svc, T> Service<Request> for FromFnService<F, S, Svc, T>
where
    F: MiddlewareFn<S, T>,
    S: Clone + Send + Sync + 'static,
    Svc: Service<Request, Response = Response, Error = std::convert::Infallible>
        + Clone
        + Send
        + 'static,
    Svc::Future: Send + 'static,
{
    type Response = Response;
    type Error = std::convert::Infallible;
    type Future = std::pin::Pin<
        Box<dyn std::future::Future<Output = Result<Response, Self::Error>> + Send + 'static>,
    >;

    #[inline]
    fn poll_ready(
        &mut self,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Result<(), Self::Error>> {
        self.inner.poll_ready(cx)
    }

    fn call(&mut self, req: Request) -> Self::Future {
        // Replace inner with a clone so the already-polled-ready slot stays valid.
        let cloned = self.inner.clone();
        let inner = std::mem::replace(&mut self.inner, cloned);
        let next = Next::new(BoxCloneService::new(inner));
        let f = self.f.clone();
        let state = self.state.clone();
        Box::pin(async move { Ok(f.call(req, state, next).await) })
    }
}
