//! Path-based HTTP router backed by a radix trie.
//!
//! The [`Router`] maps URL paths to [`MethodRouter`]s using the `matchit`
//! radix trie. Tower middleware can be attached at three levels:
//!
//! | Method | Scope |
//! |---|---|
//! | [`Router::layer`] | All requests (including 404s) |
//! | [`Router::route_layer`] | Only requests that match a route |
//! | [`MethodRouter::layer`] | Only the matched HTTP method handler |
//!
//! # Building a service
//!
//! After calling [`Router::with_state`] you receive a `Router<()>`. Call
//! [`Router::into_service`] to obtain a [`BoxCloneService`] with all layers
//! baked in, ready to be passed to [`ajaya_hyper::Server::serve_service`].
//!
//! [`ajaya_hyper::serve_app`] calls `into_service()` internally.

use std::convert::Infallible;
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;
use std::task::{Context, Poll};

use ajaya_core::handler::ErasedHandler;
use ajaya_core::request::Request;
use ajaya_core::response::{Response, ResponseBuilder};
use http::StatusCode;
use tower_layer::Layer;
use tower_service::Service;

use crate::layer::{BoxCloneService, LayerFn, apply_layers, into_layer_fn, oneshot};
use crate::method_router::{MethodRouter, StateBound};
use crate::params::PathParams;
use crate::service::ServiceHandler;

/// Extension type inserted by the router to record which route pattern matched.
///
/// Used by the `MatchedPath` extractor in `ajaya-extract`.
#[derive(Debug, Clone)]
pub struct MatchedPathExt(pub String);

// ── Router ───────────────────────────────────────────────────────────────────

/// Path-based HTTP router with radix trie matching and Tower middleware support.
///
/// # Quick start
///
/// ```rust,ignore
/// use ajaya::{Router, get, post, State};
///
/// let app = Router::new()
///     .route("/", get(home))
///     .route("/users/{id}", get(get_user).post(create_user))
///     .layer(CorsLayer::permissive())         // all routes
///     .route_layer(TraceLayer::new_for_http()) // matched routes only
///     .with_state(AppState { /* ... */ });
///
/// ajaya::serve_app("0.0.0.0:8080", app).await?;
/// ```
pub struct Router<S = ()> {
    trie: matchit::Router<usize>,
    routes: Vec<(String, MethodRouter<S>)>,
    fallback: Option<Box<dyn ErasedHandler<S>>>,
    /// Layers applied to **all** requests (applied by `into_service`).
    layers: Vec<LayerFn>,
    /// Layers applied to **matched-route** requests only (applied per-dispatch).
    route_layers: Vec<LayerFn>,
}

impl<S: Clone + Send + Sync + 'static> Router<S> {
    /// Create a new, empty `Router`.
    pub fn new() -> Self {
        Self {
            trie: matchit::Router::new(),
            routes: Vec::new(),
            fallback: None,
            layers: Vec::new(),
            route_layers: Vec::new(),
        }
    }

    /// Register a route.
    ///
    /// # Panics
    ///
    /// Panics on route conflicts (detected at startup, not at runtime).
    pub fn route(mut self, path: &str, method_router: MethodRouter<S>) -> Self {
        let idx = self.routes.len();
        if let Err(e) = self.trie.insert(path, idx) {
            panic!("Route conflict for `{path}`: {e}");
        }
        self.routes.push((path.to_string(), method_router));
        self
    }

    /// Mount a sub-router under a path prefix (flatten strategy).
    pub fn nest(mut self, prefix: &str, other: Router<S>) -> Self {
        let prefix = prefix.trim_end_matches('/');
        for (path, method_router) in other.routes {
            let full = if path == "/" {
                format!("{prefix}/")
            } else {
                format!("{prefix}{path}")
            };
            let idx = self.routes.len();
            if let Err(e) = self.trie.insert(&full, idx) {
                panic!("Nested route conflict for `{full}`: {e}");
            }
            self.routes.push((full, method_router));
        }
        self
    }

    /// Merge all routes from another router into this one.
    pub fn merge(mut self, other: Router<S>) -> Self {
        for (path, method_router) in other.routes {
            let idx = self.routes.len();
            if let Err(e) = self.trie.insert(&path, idx) {
                panic!("Merge conflict for `{path}`: {e}");
            }
            self.routes.push((path, method_router));
        }
        if self.fallback.is_none() {
            self.fallback = other.fallback;
        }
        self
    }

    /// Set a custom fallback handler for unmatched paths (default: 404).
    pub fn fallback<H, T>(mut self, handler: H) -> Self
    where
        H: ajaya_core::handler::Handler<T, S> + Clone + Send + Sync + 'static,
        T: 'static,
    {
        self.fallback = Some(ajaya_core::handler::into_erased(handler));
        self
    }

    /// Mount a Tower service at an exact path.
    pub fn route_service<T>(self, path: &str, service: T) -> Self
    where
        T: Service<Request, Response = Response, Error = Infallible>
            + Clone
            + Send
            + Sync
            + 'static,
        T::Future: Send + 'static,
    {
        self.route(path, crate::any(ServiceHandler::new(service)))
    }

    /// Mount a Tower service at a path prefix (`{prefix}/*__rest`).
    pub fn nest_service<T>(self, prefix: &str, service: T) -> Self
    where
        T: Service<Request, Response = Response, Error = Infallible>
            + Clone
            + Send
            + Sync
            + 'static,
        T::Future: Send + 'static,
    {
        let prefix = prefix.trim_end_matches('/');
        let path = format!("{prefix}/*__rest");
        self.route(&path, crate::any(ServiceHandler::new(service)))
    }

    // ── Layer methods ────────────────────────────────────────────────────────

    /// Apply a Tower middleware layer to **all routes** (including 404/405 responses).
    ///
    /// The last call to `.layer()` produces the **outermost** wrapper, so it
    /// processes the request first.
    ///
    /// ```rust,ignore
    /// Router::new()
    ///     .route("/", get(handler))
    ///     .layer(CorsLayer::permissive())   // ← processes every request
    /// ```
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

    /// Apply a Tower middleware layer to **matched routes only**.
    ///
    /// Unlike [`layer`], this does not run on 404/405 responses.
    /// Processes the request **after** outer layers but **before** per-method layers.
    ///
    /// ```rust,ignore
    /// Router::new()
    ///     .route("/users/{id}", get(handler))
    ///     .route_layer(TraceLayer::new_for_http()) // only matched routes
    ///     .layer(CorsLayer::permissive())          // all routes
    /// ```
    pub fn route_layer<L>(mut self, layer: L) -> Self
    where
        L: Layer<BoxCloneService> + Clone + Send + Sync + 'static,
        L::Service:
            Service<Request, Response = Response, Error = Infallible> + Clone + Send + 'static,
        <L::Service as Service<Request>>::Future: Send + 'static,
    {
        self.route_layers.push(into_layer_fn(layer));
        self
    }

    // ── State binding ────────────────────────────────────────────────────────

    /// Bind application state, converting `Router<S>` → `Router<()>`.
    ///
    /// Call this once before passing your router to [`serve_app`] or
    /// [`into_service`].
    pub fn with_state(self, state: S) -> Router<()> {
        let state = Arc::new(state);

        let routes: Vec<(String, MethodRouter<()>)> = self
            .routes
            .into_iter()
            .map(|(path, mr)| (path, mr.with_state((*state).clone())))
            .collect();

        let fallback: Option<Box<dyn ErasedHandler<()>>> = self.fallback.map(|f| {
            Box::new(StateBound {
                inner: f,
                state: Arc::clone(&state),
            }) as Box<dyn ErasedHandler<()>>
        });

        Router {
            trie: self.trie,
            routes,
            fallback,
            layers: self.layers, // LayerFn is state-independent
            route_layers: self.route_layers,
        }
    }

    // ── Direct call (legacy / test path) ────────────────────────────────────

    /// Dispatch a request directly, **without** applying outer layers.
    ///
    /// This method exists for backward compatibility and internal testing.
    /// For production serving use [`into_service`] (via [`serve_app`]).
    pub async fn call(&self, mut req: Request, state: S) -> Response {
        let path = req.uri().path().to_string();

        match self.trie.at(&path) {
            Ok(matched) => {
                let idx = *matched.value;
                let pattern = self.routes[idx].0.clone();

                if !matched.params.is_empty() {
                    let mut pp = PathParams::new();
                    for (k, v) in matched.params.iter() {
                        pp.push(k.to_string(), percent_decode(v));
                    }
                    req.extensions_mut().insert(pp);
                }
                req.extensions_mut().insert(MatchedPathExt(pattern));

                self.routes[idx].1.call(req, state).await
            }
            Err(_) => {
                if let Some(fb) = &self.fallback {
                    return fb.clone_box().call(req, state).await;
                }
                not_found()
            }
        }
    }
}

// ── Router<()> — Tower service conversion ───────────────────────────────────

impl Router<()> {
    /// Convert this router into a Tower [`BoxCloneService`] with all
    /// configured layers applied.
    ///
    /// Layer ordering:
    /// - `layers` (outer): wrap everything including 404s
    /// - `route_layers` (inner): wrap matched routes only
    /// - `MethodRouter::layers`: wrap matched method handlers only
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// let svc = app.into_service();
    /// server.serve_service(svc).await?;
    /// ```
    pub fn into_service(self) -> BoxCloneService {
        let route_layers = self.route_layers;
        let outer_layers = self.layers;

        // Inner core: route dispatch + route_layers
        let inner = Arc::new(RouterInner {
            trie: self.trie,
            routes: self.routes,
            fallback: self.fallback,
            route_layers,
        });

        // Wrap in a clone-friendly Tower service
        let base = BoxCloneService::new(RouterService(inner));

        // Apply outer layers (first added = innermost)
        apply_layers(base, &outer_layers)
    }
}

impl<S: Clone + Send + Sync + 'static> Default for Router<S> {
    fn default() -> Self {
        Self::new()
    }
}

// ── RouterInner + RouterService ──────────────────────────────────────────────
//
// Split so the matchit trie (not Clone) can live behind an Arc.

struct RouterInner {
    trie: matchit::Router<usize>,
    routes: Vec<(String, MethodRouter<()>)>,
    fallback: Option<Box<dyn ErasedHandler<()>>>,
    route_layers: Vec<LayerFn>,
}

impl RouterInner {
    async fn dispatch(&self, mut req: Request) -> Response {
        let path = req.uri().path().to_string();

        match self.trie.at(&path) {
            Ok(matched) => {
                let idx = *matched.value;
                let pattern = self.routes[idx].0.clone();

                if !matched.params.is_empty() {
                    let mut pp = PathParams::new();
                    for (k, v) in matched.params.iter() {
                        pp.push(k.to_string(), percent_decode(v));
                    }
                    req.extensions_mut().insert(pp);
                }
                req.extensions_mut().insert(MatchedPathExt(pattern));

                if self.route_layers.is_empty() {
                    // ── Fast path: no route-level layers ─────────────────────
                    self.routes[idx].1.call(req, ()).await
                } else {
                    // ── Apply route_layers around the method router ───────────
                    let mr = self.routes[idx].1.clone();
                    let base = BoxCloneService::new(MethodRouterService(mr));
                    let svc = apply_layers(base, &self.route_layers);
                    oneshot(svc, req).await
                }
            }
            Err(_) => {
                if let Some(fb) = &self.fallback {
                    return fb.clone_box().call(req, ()).await;
                }
                not_found()
            }
        }
    }
}

// Tower service wrapper that shares RouterInner via Arc
struct RouterService(Arc<RouterInner>);

impl Clone for RouterService {
    fn clone(&self) -> Self {
        RouterService(Arc::clone(&self.0))
    }
}

impl Service<Request> for RouterService {
    type Response = Response;
    type Error = Infallible;
    type Future = Pin<Box<dyn Future<Output = Result<Response, Infallible>> + Send + 'static>>;

    fn poll_ready(&mut self, _: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        Poll::Ready(Ok(()))
    }

    fn call(&mut self, req: Request) -> Self::Future {
        let inner = Arc::clone(&self.0);
        Box::pin(async move { Ok(inner.dispatch(req).await) })
    }
}

// Tower service that wraps a MethodRouter<()> (used by route_layer path)
struct MethodRouterService(MethodRouter<()>);

impl Clone for MethodRouterService {
    fn clone(&self) -> Self {
        MethodRouterService(self.0.clone())
    }
}

impl Service<Request> for MethodRouterService {
    type Response = Response;
    type Error = Infallible;
    type Future = Pin<Box<dyn Future<Output = Result<Response, Infallible>> + Send + 'static>>;

    fn poll_ready(&mut self, _: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        Poll::Ready(Ok(()))
    }

    fn call(&mut self, req: Request) -> Self::Future {
        let mr = self.0.clone();
        Box::pin(async move { Ok(mr.call(req, ()).await) })
    }
}

// ── Helpers ──────────────────────────────────────────────────────────────────

#[inline]
fn not_found() -> Response {
    ResponseBuilder::new()
        .status(StatusCode::NOT_FOUND)
        .header(http::header::CONTENT_TYPE, "text/plain; charset=utf-8")
        .text("Not Found")
}

/// Percent-decode a URL path segment in-place (ASCII-only fast path).
fn percent_decode(input: &str) -> String {
    let bytes = input.as_bytes();
    if !bytes.contains(&b'%') {
        return input.to_string(); // fast path: nothing to decode
    }
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' && i + 2 < bytes.len() {
            if let (Some(hi), Some(lo)) = (hex_nibble(bytes[i + 1]), hex_nibble(bytes[i + 2])) {
                out.push(hi << 4 | lo);
                i += 3;
                continue;
            }
        }
        out.push(bytes[i]);
        i += 1;
    }
    String::from_utf8(out).unwrap_or_else(|_| input.to_string())
}

#[inline]
fn hex_nibble(b: u8) -> Option<u8> {
    match b {
        b'0'..=b'9' => Some(b - b'0'),
        b'a'..=b'f' => Some(b - b'a' + 10),
        b'A'..=b'F' => Some(b - b'A' + 10),
        _ => None,
    }
}

// ── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_percent_decode() {
        assert_eq!(percent_decode("hello%20world"), "hello world");
        assert_eq!(percent_decode("foo%2Fbar"), "foo/bar");
        assert_eq!(percent_decode("normal"), "normal");
        assert_eq!(percent_decode("100%25"), "100%");
    }

    #[test]
    #[should_panic(expected = "Route conflict")]
    fn test_duplicate_route_panics() {
        async fn handler() -> &'static str {
            "test"
        }
        let _: Router<()> = Router::new()
            .route("/test", crate::get(handler))
            .route("/test", crate::get(handler));
    }

    #[test]
    fn test_static_routes() {
        async fn h() -> &'static str {
            "ok"
        }
        let _: Router<()> = Router::new()
            .route("/", crate::get(h))
            .route("/users", crate::get(h))
            .route("/about", crate::get(h));
    }

    #[test]
    fn test_param_routes() {
        async fn h() -> &'static str {
            "ok"
        }
        let _: Router<()> = Router::new()
            .route("/users/{id}", crate::get(h))
            .route("/users/{id}/posts/{post_id}", crate::get(h));
    }

    #[test]
    fn test_wildcard_routes() {
        async fn h() -> &'static str {
            "ok"
        }
        let _: Router<()> = Router::new()
            .route("/files/{*path}", crate::get(h))
            .route("/", crate::get(h));
    }

    #[test]
    fn test_nest() {
        async fn h() -> &'static str {
            "ok"
        }
        let sub = Router::new()
            .route("/", crate::get(h))
            .route("/{id}", crate::get(h));
        let _: Router<()> = Router::new().route("/", crate::get(h)).nest("/users", sub);
    }

    #[test]
    fn test_merge() {
        async fn h() -> &'static str {
            "ok"
        }
        let api = Router::new().route("/users", crate::get(h));
        let admin = Router::new().route("/admin", crate::get(h));
        let _: Router<()> = api.merge(admin);
    }

    #[test]
    #[should_panic(expected = "Merge conflict")]
    fn test_merge_conflict_panics() {
        async fn h() -> &'static str {
            "ok"
        }
        let a = Router::new().route("/users", crate::get(h));
        let b = Router::new().route("/users", crate::get(h));
        let _: Router<()> = a.merge(b);
    }
}
