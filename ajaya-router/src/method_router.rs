//! HTTP method-based request dispatch.
//!
//! [`MethodRouter`] stores one handler per HTTP method and dispatches
//! incoming requests to the appropriate handler based on the request method.
//!
//! # Examples
//!
//! ```rust,ignore
//! use ajaya_router::{get, post};
//!
//! async fn hello() -> &'static str { "Hello!" }
//! async fn create() -> &'static str { "Created!" }
//!
//! let router = get(hello).post(create);
//! ```

use ajaya_core::handler::{ErasedHandler, Handler, into_erased};
use ajaya_core::method_filter::MethodFilter;
use ajaya_core::request::Request;
use ajaya_core::response::{Response, ResponseBuilder};
use http::StatusCode;

/// Stores a handler per HTTP method for a single route.
///
/// Created via the top-level constructor functions [`get`], [`post`],
/// [`put`], [`delete`], [`patch`], [`head`], [`options`], [`any`],
/// or via [`MethodRouter::on`].
///
/// Handlers can be chained:
///
/// ```rust,ignore
/// let router = get(get_handler)
///     .post(post_handler)
///     .delete(delete_handler);
/// ```
pub struct MethodRouter<S = ()> {
    /// Handlers keyed by method filter bitmask.
    handlers: Vec<(MethodFilter, Box<dyn ErasedHandler<S>>)>,
    /// Allow methods registered (for 405 response header).
    allow_methods: MethodFilter,
}

impl<S: Clone + Send + Sync + 'static> MethodRouter<S> {
    /// Create an empty `MethodRouter` with no handlers.
    pub fn new() -> Self {
        Self {
            handlers: Vec::new(),
            allow_methods: MethodFilter::NONE,
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

    /// Dispatch a request based on its HTTP method.
    ///
    /// Returns `405 Method Not Allowed` if no handler matches,
    /// with an `Allow` header listing registered methods.
    pub async fn call(&self, req: Request, state: S) -> Response {
        let method = req.method().clone();
        let method_filter = MethodFilter::from_method(&method);

        // Find matching handler
        for (filter, handler) in &self.handlers {
            if filter.contains(method_filter) {
                let handler = handler.clone_box();
                return handler.call(req, state).await;
            }
        }

        // No handler matched — return 405 Method Not Allowed
        let allow_header = build_allow_header(self.allow_methods);
        ResponseBuilder::new()
            .status(StatusCode::METHOD_NOT_ALLOWED)
            .header(http::header::ALLOW, allow_header)
            .text("Method Not Allowed")
    }
}

impl<S: Clone + Send + Sync + 'static> Default for MethodRouter<S> {
    fn default() -> Self {
        Self::new()
    }
}

// --- Top-level constructor functions ---

/// Create a [`MethodRouter`] with a GET handler.
pub fn get<H, T, S>(handler: H) -> MethodRouter<S>
where
    H: Handler<T, S> + Clone + Send + Sync + 'static,
    T: 'static,
    S: Clone + Send + Sync + 'static,
{
    MethodRouter::new().get(handler)
}

/// Create a [`MethodRouter`] with a POST handler.
pub fn post<H, T, S>(handler: H) -> MethodRouter<S>
where
    H: Handler<T, S> + Clone + Send + Sync + 'static,
    T: 'static,
    S: Clone + Send + Sync + 'static,
{
    MethodRouter::new().post(handler)
}

/// Create a [`MethodRouter`] with a PUT handler.
pub fn put<H, T, S>(handler: H) -> MethodRouter<S>
where
    H: Handler<T, S> + Clone + Send + Sync + 'static,
    T: 'static,
    S: Clone + Send + Sync + 'static,
{
    MethodRouter::new().put(handler)
}

/// Create a [`MethodRouter`] with a DELETE handler.
pub fn delete<H, T, S>(handler: H) -> MethodRouter<S>
where
    H: Handler<T, S> + Clone + Send + Sync + 'static,
    T: 'static,
    S: Clone + Send + Sync + 'static,
{
    MethodRouter::new().delete(handler)
}

/// Create a [`MethodRouter`] with a PATCH handler.
pub fn patch<H, T, S>(handler: H) -> MethodRouter<S>
where
    H: Handler<T, S> + Clone + Send + Sync + 'static,
    T: 'static,
    S: Clone + Send + Sync + 'static,
{
    MethodRouter::new().patch(handler)
}

/// Create a [`MethodRouter`] with a HEAD handler.
pub fn head<H, T, S>(handler: H) -> MethodRouter<S>
where
    H: Handler<T, S> + Clone + Send + Sync + 'static,
    T: 'static,
    S: Clone + Send + Sync + 'static,
{
    MethodRouter::new().head(handler)
}

/// Create a [`MethodRouter`] with an OPTIONS handler.
pub fn options<H, T, S>(handler: H) -> MethodRouter<S>
where
    H: Handler<T, S> + Clone + Send + Sync + 'static,
    T: 'static,
    S: Clone + Send + Sync + 'static,
{
    MethodRouter::new().options(handler)
}

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

/// Create a [`MethodRouter`] with a handler for the given method filter.
pub fn on<H, T, S>(filter: MethodFilter, handler: H) -> MethodRouter<S>
where
    H: Handler<T, S> + Clone + Send + Sync + 'static,
    T: 'static,
    S: Clone + Send + Sync + 'static,
{
    MethodRouter::new().on(filter, handler)
}

// --- Internal helpers ---

/// Build the `Allow` header value from a MethodFilter bitmask.
fn build_allow_header(filter: MethodFilter) -> String {
    let mut methods = Vec::new();
    if filter.contains(MethodFilter::GET) {
        methods.push("GET");
    }
    if filter.contains(MethodFilter::POST) {
        methods.push("POST");
    }
    if filter.contains(MethodFilter::PUT) {
        methods.push("PUT");
    }
    if filter.contains(MethodFilter::DELETE) {
        methods.push("DELETE");
    }
    if filter.contains(MethodFilter::PATCH) {
        methods.push("PATCH");
    }
    if filter.contains(MethodFilter::HEAD) {
        methods.push("HEAD");
    }
    if filter.contains(MethodFilter::OPTIONS) {
        methods.push("OPTIONS");
    }
    if filter.contains(MethodFilter::TRACE) {
        methods.push("TRACE");
    }
    methods.join(", ")
}
