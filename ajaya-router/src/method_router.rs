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

use ajaya_core::handler::{BoxFuture, ErasedHandler, Handler, into_erased};
use ajaya_core::method_filter::MethodFilter;
use ajaya_core::request::Request;
use ajaya_core::response::{Response, ResponseBuilder};
use http::StatusCode;
use tower_layer::Layer;
use tower_service::Service;

use crate::layer::{BoxCloneService, LayerFn, apply_layers, into_layer_fn, oneshot};

// ── MethodRouter ────────────────────────────────────────────────────────────

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
    /// (method_filter, type-erased handler) pairs.
    handlers: Vec<(MethodFilter, Box<dyn ErasedHandler<S>>)>,
    /// Bitmask of all registered methods — used to build the `Allow` header.
    allow_methods: MethodFilter,
    /// Tower layers applied to each matched handler (innermost = first in vec).
    layers: Vec<LayerFn>,
}

impl<S: Clone + Send + Sync + 'static> MethodRouter<S> {
    /// Create an empty `MethodRouter` with no handlers.
    pub fn new() -> Self {
        Self {
            handlers: Vec::new(),
            allow_methods: MethodFilter::NONE,
            layers: Vec::new(),
        }
    }

    /// Register a handler for the given method filter.
    pub fn on<H, T>(mut self, filter: MethodFilter, handler: H) -> Self
    where
        H: Handler<T, S> + Clone + Send + Sync + 'static,
        T: 'static,
    {
        self.allow_methods |= filter;
        self.handlers.push((filter, into_erased(handler)));
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
    /// use ajaya::{get, post};
    /// use ajaya_middleware::auth::RequireAuthLayer;
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
        L::Service:
            Service<Request, Response = Response, Error = Infallible> + Clone + Send + 'static,
        <L::Service as Service<Request>>::Future: Send + 'static,
    {
        self.layers.push(into_layer_fn(layer));
        self
    }

    /// Dispatch the request to the matching method handler, applying any
    /// configured layers.
    ///
    /// Returns `405 Method Not Allowed` (with an `Allow` header) if no handler
    /// matches the request method.
    pub async fn call(&self, req: Request, state: S) -> Response {
        let method = req.method().clone();
        let method_filter = MethodFilter::from_method(&method);

        for (filter, handler) in &self.handlers {
            if filter.contains(method_filter) {
                if self.layers.is_empty() {
                    // ── Fast path: no per-route layers ──────────────────────
                    let h = handler.clone_box();
                    return h.call(req, state).await;
                } else {
                    // ── Layered path ─────────────────────────────────────────
                    // Wrap the handler in a Tower service so layers can compose
                    // around it with the standard Layer<S> protocol.
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
            .text("Method Not Allowed")
    }

    /// Bind application state, converting `MethodRouter<S>` → `MethodRouter<()>`.
    ///
    /// After calling this, the method router is ready to be served directly
    /// or attached to a [`Router`] that is itself bound with [`Router::with_state`].
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// use ajaya::{get, State};
    ///
    /// async fn handler(State(s): State<AppState>) -> String { s.greeting }
    ///
    /// let method_router = get(handler).with_state(AppState { greeting: "hi".into() });
    /// ```
    pub fn with_state(self, state: S) -> MethodRouter<()> {
        let state = Arc::new(state);
        let handlers = self
            .handlers
            .into_iter()
            .map(|(filter, handler)| {
                let bound: Box<dyn ErasedHandler<()>> = Box::new(StateBound {
                    inner: handler,
                    state: Arc::clone(&state),
                });
                (filter, bound)
            })
            .collect();

        MethodRouter {
            handlers,
            allow_methods: self.allow_methods,
            layers: self.layers, // LayerFn is state-independent — pass through
        }
    }
}

impl<S: Clone + Send + Sync + 'static> Clone for MethodRouter<S> {
    fn clone(&self) -> Self {
        Self {
            handlers: self
                .handlers
                .iter()
                .map(|(f, h)| (*f, h.clone_box()))
                .collect(),
            allow_methods: self.allow_methods,
            layers: self.layers.clone(), // Arc — O(n) cheap clone
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
