//! HTTP method-based request dispatch.
//!
//! [`MethodRouter`] stores one handler per HTTP method and dispatches
//! incoming requests to the appropriate handler. Supports Tower middleware
//! via [`MethodRouter::layer`], which wraps every matched handler.
//!
//! # Layer ordering
//!
//! ```rust,ignore
//! get(handler)
//!     .layer(AuthLayer)     // innermost — runs last on request
//!     .layer(TraceLayer)    // outermost — runs first on request
//! ```

use std::convert::Infallible;
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;
use std::task::{Context, Poll};

use arvik_core::handler::{BoxFuture, ErasedHandler, Handler, into_erased};
use arvik_core::method_filter::MethodFilter;
use arvik_core::request::Request;
use arvik_core::response::{Response, ResponseBuilder};
use http::StatusCode;
use tower_layer::Layer;
use tower_service::Service;

use crate::layer::{BoxCloneService, LayerFn, apply_layers, into_layer_fn, oneshot};

// ── MethodRouter ────────────────────────────────────────────────────────────

/// Shared, type-erased handler list.
type HandlerList<S> = std::sync::Arc<Vec<(MethodFilter, Box<dyn ErasedHandler<S>>)>>;

/// Stores one handler per HTTP method for a single route.
///
/// Created via the top-level constructor functions [`get`], [`post`], etc.
/// Handlers can be chained and middleware layers attached:
///
/// ```rust,ignore
/// let route = get(get_handler)
///     .post(post_handler)
///     .delete(delete_handler)
///     .layer(RequireAuthLayer::new());
/// ```
pub struct MethodRouter<S = ()> {
    /// (method_filter, type-erased handler) pairs, shared behind an Arc so
    /// cloning a router for per-request dispatch is an O(1) refcount bump
    /// instead of deep-cloning every boxed handler.
    handlers: HandlerList<S>,
    /// Bitmask of all registered methods — used to build the `Allow` header.
    allow_methods: MethodFilter,
    /// Tower layers applied to each matched handler (innermost = first in vec).
    /// Behind an Arc so cloning the router stays allocation-free.
    layers: std::sync::Arc<Vec<LayerFn>>,
    /// Per-handler layer stacks folded ONCE after `with_state` binds the
    /// state (indexed like `handlers`; `None` = no layers → fast path).
    /// Valid only once the state is fixed (`S = ()`), hence the flag.
    baked: std::sync::OnceLock<std::sync::Arc<Vec<Option<BoxCloneService>>>>,
    /// True after `with_state` — the baked stacks above may be built.
    bakeable: bool,
}

/// Mutable access to a handler list that may be shared behind an Arc.
///
/// During router construction the Arc is almost always unique, so this is a
/// no-op there; sharing only appears once built routers get cloned.
fn handlers_mut<S>(
    handlers: &mut HandlerList<S>,
) -> &mut Vec<(MethodFilter, Box<dyn ErasedHandler<S>>)>
where
    S: Clone + Send + Sync + 'static,
{
    if std::sync::Arc::get_mut(handlers).is_none() {
        *handlers = std::sync::Arc::new(
            handlers
                .iter()
                .map(|(filter, handler)| (*filter, handler.clone_box()))
                .collect(),
        );
    }
    std::sync::Arc::get_mut(handlers).expect("just made unique")
}

impl<S: Clone + Send + Sync + 'static> MethodRouter<S> {
    /// Create an empty `MethodRouter` with no handlers.
    pub fn new() -> Self {
        Self {
            handlers: std::sync::Arc::new(Vec::new()),
            allow_methods: MethodFilter::NONE,
            layers: std::sync::Arc::new(Vec::new()),
            baked: std::sync::OnceLock::new(),
            bakeable: false,
        }
    }

    /// Register a handler for the given method filter.
    pub fn on<H, T>(mut self, filter: MethodFilter, handler: H) -> Self
    where
        H: Handler<T, S> + Clone + Send + Sync + 'static,
        T: 'static,
    {
        self.allow_methods |= filter;
        handlers_mut(&mut self.handlers).push((filter, into_erased(handler)));
        self.baked = std::sync::OnceLock::new();
        self
    }

    /// Merge another method router for the same path.
    ///
    /// This is primarily used by proc-macro collected routes so separate
    /// `#[get("/path")]` and `#[post("/path")]` handlers can share one route.
    pub fn merge(mut self, other: Self) -> Self {
        if (self.allow_methods.bits() & other.allow_methods.bits()) != 0 {
            panic!("Method route conflict: duplicate method for route");
        }

        self.allow_methods |= other.allow_methods;
        handlers_mut(&mut self.handlers)
            .extend(other.handlers.iter().map(|(f, h)| (*f, h.clone_box())));
        std::sync::Arc::make_mut(&mut self.layers).extend(other.layers.iter().cloned());
        self.baked = std::sync::OnceLock::new();
        self.bakeable = false;
        self
    }

    /// Register a GET handler.
    pub fn get<H, T>(self, handler: H) -> Self
    where
        H: Handler<T, S> + Clone + Send + Sync + 'static,
        T: 'static,
    {
        self.on(MethodFilter::GET, handler)
    }

    /// Register a POST handler.
    pub fn post<H, T>(self, handler: H) -> Self
    where
        H: Handler<T, S> + Clone + Send + Sync + 'static,
        T: 'static,
    {
        self.on(MethodFilter::POST, handler)
    }

    /// Register a PUT handler.
    pub fn put<H, T>(self, handler: H) -> Self
    where
        H: Handler<T, S> + Clone + Send + Sync + 'static,
        T: 'static,
    {
        self.on(MethodFilter::PUT, handler)
    }

    /// Register a DELETE handler.
    pub fn delete<H, T>(self, handler: H) -> Self
    where
        H: Handler<T, S> + Clone + Send + Sync + 'static,
        T: 'static,
    {
        self.on(MethodFilter::DELETE, handler)
    }

    /// Register a PATCH handler.
    pub fn patch<H, T>(self, handler: H) -> Self
    where
        H: Handler<T, S> + Clone + Send + Sync + 'static,
        T: 'static,
    {
        self.on(MethodFilter::PATCH, handler)
    }

    /// Register a HEAD handler.
    pub fn head<H, T>(self, handler: H) -> Self
    where
        H: Handler<T, S> + Clone + Send + Sync + 'static,
        T: 'static,
    {
        self.on(MethodFilter::HEAD, handler)
    }

    /// Register an OPTIONS handler.
    pub fn options<H, T>(self, handler: H) -> Self
    where
        H: Handler<T, S> + Clone + Send + Sync + 'static,
        T: 'static,
    {
        self.on(MethodFilter::OPTIONS, handler)
    }

    /// Apply a Tower middleware layer to **every handler on this route**.
    ///
    /// Layers are applied in the order they are added: the last call to
    /// `.layer()` produces the **outermost** wrapper (runs first on request).
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// use arvik::{get, post};
    /// use arvik_middleware::auth::RequireAuthLayer;
    ///
    /// let route = get(get_handler)
    ///     .post(create_handler)
    ///     .layer(RequireAuthLayer::bearer("secret"));
    /// ```
    ///
    /// # Bounds
    ///
    /// The layer and its resulting service must be:
    /// - `Clone + Send + Sync + 'static`
    /// - The service future must be `Send + 'static`
    pub fn layer<L>(mut self, layer: L) -> Self
    where
        L: Layer<BoxCloneService> + Clone + Send + Sync + 'static,
        L::Service: Service<Request, Response = Response, Error = Infallible>
            + Clone
            + Send
            + Sync
            + 'static,
        <L::Service as Service<Request>>::Future: Send + 'static,
    {
        std::sync::Arc::make_mut(&mut self.layers).push(into_layer_fn(layer));
        self.baked = std::sync::OnceLock::new();
        self
    }

    /// Dispatch the request to the matching method handler, applying any
    /// configured layers.
    ///
    /// HEAD requests are served by the GET handler when no dedicated HEAD
    /// handler is registered (RFC 9110 §9.3.2 — hyper strips the response
    /// body for HEAD).
    ///
    /// Returns `405 Method Not Allowed` (with an `Allow` header) if no handler
    /// matches the request method — including non-standard extension methods
    /// (e.g. `PURGE`) unless the route registered [`MethodFilter::EXTENSION`]
    /// or [`MethodFilter::ANY`].
    pub async fn call(&self, req: Request, state: S) -> Response {
        // Borrowed — no Method clone per request.
        let method = req.method();
        let method_filter = MethodFilter::from_method(method);

        // RFC 9110 §9.3.2: a HEAD response is a GET response without a body,
        // so fall back to the GET handler unless HEAD was explicitly bound.
        let head_falls_back_to_get =
            *method == http::Method::HEAD && !self.allow_methods.contains(MethodFilter::HEAD);

        // Baked per-handler layer stacks exist once the state is bound
        // (`with_state`): fold every layer exactly once instead of rebuilding
        // the whole stack per request.
        let baked = if self.bakeable && !self.layers.is_empty() {
            Some(self.baked.get_or_init(|| {
                std::sync::Arc::new(
                    self.handlers
                        .iter()
                        .map(|(_, handler)| {
                            let base = BoxCloneService::new(HandlerService {
                                handler: handler.clone_box(),
                                state: state.clone(),
                            });
                            Some(apply_layers(base, &self.layers))
                        })
                        .collect(),
                )
            }))
        } else {
            None
        };

        for (idx, (filter, handler)) in self.handlers.iter().enumerate() {
            let matched = filter.contains(method_filter)
                || (head_falls_back_to_get && filter.contains(MethodFilter::GET));
            if matched {
                if let Some(baked) = baked.as_ref().and_then(|stack| stack[idx].as_ref()) {
                    return oneshot(baked.clone(), req).await;
                }
                if self.layers.is_empty() {
                    // ── Fast path: no per-route layers ──────────────────────
                    let h = handler.clone_box();
                    return h.call(req, state).await;
                } else {
                    // ── Layered path (unbound generic router; rare) ─────────
                    let h = handler.clone_box();
                    let base = BoxCloneService::new(HandlerService {
                        handler: h,
                        state: state.clone(),
                    });
                    let svc = apply_layers(base, &self.layers);
                    return oneshot(svc, req).await;
                }
            }
        }

        // No handler matched — 405 with Allow header
        let allow = build_allow_header(self.allow_methods);
        ResponseBuilder::new()
            .status(StatusCode::METHOD_NOT_ALLOWED)
            .header(http::header::ALLOW, allow)
            .text_static("Method Not Allowed")
    }

    /// Append already-erased layers to this router's per-route stack.
    ///
    /// Used by [`crate::Router::nest`] / [`crate::Router::merge`] to preserve
    /// middleware attached inside a sub-router when its routes are flattened
    /// into the parent: appended layers wrap the ones registered earlier,
    /// matching the router-level (`layer` outside `route_layer`) ordering.
    pub(crate) fn extend_layers(&mut self, extra: impl IntoIterator<Item = LayerFn>) {
        let extra: Vec<LayerFn> = extra.into_iter().collect();
        if extra.is_empty() {
            return;
        }
        std::sync::Arc::make_mut(&mut self.layers).extend(extra);
        self.baked = std::sync::OnceLock::new();
    }

    /// Bind application state, converting `MethodRouter<S>` → `MethodRouter<()>`.
    ///
    /// After calling this, the method router is ready to be served directly
    /// or attached to a `Router` that is itself bound with `Router::with_state`.
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// use arvik::{get, State};
    ///
    /// async fn handler(State(s): State<AppState>) -> String { s.greeting }
    ///
    /// let method_router = get(handler).with_state(AppState { greeting: "hi".into() });
    /// ```
    pub fn with_state(self, state: S) -> MethodRouter<()> {
        let state = Arc::new(state);
        let handlers = std::sync::Arc::new(
            self.handlers
                .iter()
                .map(|(filter, handler)| {
                    let bound: Box<dyn ErasedHandler<()>> = Box::new(StateBound {
                        inner: handler.clone_box(),
                        state: Arc::clone(&state),
                    });
                    (*filter, bound)
                })
                .collect(),
        );

        MethodRouter {
            handlers,
            allow_methods: self.allow_methods,
            layers: self.layers, // LayerFn is state-independent — pass through
            baked: std::sync::OnceLock::new(),
            bakeable: true, // state is now fixed to () — stacks may be baked
        }
    }
}

impl<S: Clone + Send + Sync + 'static> Clone for MethodRouter<S> {
    fn clone(&self) -> Self {
        // handlers live behind an Arc: no per-handler clone_box here.
        Self {
            handlers: std::sync::Arc::clone(&self.handlers),
            allow_methods: self.allow_methods,
            layers: std::sync::Arc::clone(&self.layers),
            // Clones share handlers but not a fixed state binding, so they
            // rebuild their own stacks lazily only if re-bound via with_state.
            baked: std::sync::OnceLock::new(),
            bakeable: false,
        }
    }
}

impl<S: Clone + Send + Sync + 'static> Default for MethodRouter<S> {
    fn default() -> Self {
        Self::new()
    }
}

// ── HandlerService ───────────────────────────────────────────────────────────
//
// Wraps an ErasedHandler + its state as a Tower Service<Request>.
// This is the "leaf" service that MethodRouter::layer() layers compose around.

struct HandlerService<S> {
    handler: Box<dyn ErasedHandler<S>>,
    state: S,
}

impl<S: Clone + Send + Sync + 'static> Clone for HandlerService<S> {
    fn clone(&self) -> Self {
        Self {
            handler: self.handler.clone_box(),
            state: self.state.clone(),
        }
    }
}

impl<S: Clone + Send + Sync + 'static> Service<Request> for HandlerService<S> {
    type Response = Response;
    type Error = Infallible;
    type Future = Pin<Box<dyn Future<Output = Result<Response, Infallible>> + Send + 'static>>;

    fn poll_ready(&mut self, _: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        Poll::Ready(Ok(()))
    }

    fn call(&mut self, req: Request) -> Self::Future {
        let h = self.handler.clone_box();
        let s = self.state.clone();
        Box::pin(async move { Ok(h.call(req, s).await) })
    }
}

// ── StateBound ───────────────────────────────────────────────────────────────
//
// Identical to the one already in method_router; reproduced here to avoid
// a circular dep with router.rs.

pub(crate) struct StateBound<S> {
    pub(crate) inner: Box<dyn ErasedHandler<S>>,
    pub(crate) state: Arc<S>,
}

impl<S: Clone + Send + Sync + 'static> ErasedHandler<()> for StateBound<S> {
    fn clone_box(&self) -> Box<dyn ErasedHandler<()>> {
        Box::new(StateBound {
            inner: self.inner.clone_box(),
            state: Arc::clone(&self.state),
        })
    }

    fn call(self: Box<Self>, req: Request, _state: ()) -> BoxFuture<'static, Response> {
        let state = (*self.state).clone();
        self.inner.call(req, state)
    }
}

// ── Top-level constructor functions ─────────────────────────────────────────

macro_rules! route_fn {
    ($name:ident, $filter:expr, $doc:literal) => {
        #[doc = $doc]
        pub fn $name<H, T, S>(handler: H) -> MethodRouter<S>
        where
            H: Handler<T, S> + Clone + Send + Sync + 'static,
            T: 'static,
            S: Clone + Send + Sync + 'static,
        {
            MethodRouter::new().on($filter, handler)
        }
    };
}

route_fn!(
    get,
    MethodFilter::GET,
    "Create a [`MethodRouter`] with a GET handler."
);
route_fn!(
    post,
    MethodFilter::POST,
    "Create a [`MethodRouter`] with a POST handler."
);
route_fn!(
    put,
    MethodFilter::PUT,
    "Create a [`MethodRouter`] with a PUT handler."
);
route_fn!(
    delete,
    MethodFilter::DELETE,
    "Create a [`MethodRouter`] with a DELETE handler."
);
route_fn!(
    patch,
    MethodFilter::PATCH,
    "Create a [`MethodRouter`] with a PATCH handler."
);
route_fn!(
    head,
    MethodFilter::HEAD,
    "Create a [`MethodRouter`] with a HEAD handler."
);
route_fn!(
    options,
    MethodFilter::OPTIONS,
    "Create a [`MethodRouter`] with an OPTIONS handler."
);

/// Create a [`MethodRouter`] with a TRACE handler.
pub fn trace_method<H, T, S>(handler: H) -> MethodRouter<S>
where
    H: Handler<T, S> + Clone + Send + Sync + 'static,
    T: 'static,
    S: Clone + Send + Sync + 'static,
{
    MethodRouter::new().on(MethodFilter::TRACE, handler)
}

/// Create a [`MethodRouter`] that matches any HTTP method.
pub fn any<H, T, S>(handler: H) -> MethodRouter<S>
where
    H: Handler<T, S> + Clone + Send + Sync + 'static,
    T: 'static,
    S: Clone + Send + Sync + 'static,
{
    MethodRouter::new().on(MethodFilter::ANY, handler)
}

/// Create a [`MethodRouter`] with a handler for the given [`MethodFilter`].
pub fn on<H, T, S>(filter: MethodFilter, handler: H) -> MethodRouter<S>
where
    H: Handler<T, S> + Clone + Send + Sync + 'static,
    T: 'static,
    S: Clone + Send + Sync + 'static,
{
    MethodRouter::new().on(filter, handler)
}

// ── Internal helpers ─────────────────────────────────────────────────────────

fn build_allow_header(filter: MethodFilter) -> String {
    const PAIRS: &[(MethodFilter, &str)] = &[
        (MethodFilter::GET, "GET"),
        (MethodFilter::POST, "POST"),
        (MethodFilter::PUT, "PUT"),
        (MethodFilter::DELETE, "DELETE"),
        (MethodFilter::PATCH, "PATCH"),
        (MethodFilter::HEAD, "HEAD"),
        (MethodFilter::OPTIONS, "OPTIONS"),
        (MethodFilter::TRACE, "TRACE"),
    ];
    PAIRS
        .iter()
        .filter(|(f, _)| filter.contains(*f))
        .map(|(_, m)| *m)
        .collect::<Vec<_>>()
        .join(", ")
}

#[cfg(test)]
mod tests {
    use super::*;
    use arvik_core::Body;

    async fn get_handler() -> &'static str {
        "get-body"
    }

    async fn delete_handler() -> &'static str {
        "deleted"
    }

    async fn head_handler() -> &'static str {
        "head-body"
    }

    async fn any_handler() -> &'static str {
        "any-body"
    }

    fn request(method: &str) -> Request {
        Request::new(
            http::Request::builder()
                .method(method)
                .uri("/")
                .body(Body::empty())
                .unwrap(),
        )
    }

    async fn body_string(res: Response) -> String {
        res.into_body().to_string().await.unwrap()
    }

    #[tokio::test]
    async fn unknown_method_returns_405_not_the_registered_handler() {
        let router = MethodRouter::new().delete(delete_handler);
        let res = router.call(request("PURGE"), ()).await;
        assert_eq!(res.status(), StatusCode::METHOD_NOT_ALLOWED);
        assert_eq!(res.headers().get(http::header::ALLOW).unwrap(), "DELETE");
        assert_eq!(body_string(res).await, "Method Not Allowed");
    }

    #[tokio::test]
    async fn known_method_mismatch_still_returns_405() {
        let router = MethodRouter::new().delete(delete_handler);
        let res = router.call(request("POST"), ()).await;
        assert_eq!(res.status(), StatusCode::METHOD_NOT_ALLOWED);
    }

    #[tokio::test]
    async fn any_route_still_matches_extension_methods() {
        let router = MethodRouter::new().on(MethodFilter::ANY, any_handler);
        let res = router.call(request("PURGE"), ()).await;
        assert_eq!(res.status(), StatusCode::OK);
        assert_eq!(body_string(res).await, "any-body");
    }

    #[tokio::test]
    async fn head_falls_back_to_get_handler() {
        let router = MethodRouter::new().get(get_handler);
        let res = router.call(request("HEAD"), ()).await;
        assert_eq!(res.status(), StatusCode::OK);
        assert_eq!(body_string(res).await, "get-body");
    }

    #[tokio::test]
    async fn dedicated_head_handler_takes_precedence_over_get() {
        let router = MethodRouter::new().get(get_handler).head(head_handler);
        let res = router.call(request("HEAD"), ()).await;
        assert_eq!(res.status(), StatusCode::OK);
        assert_eq!(body_string(res).await, "head-body");
    }
}
