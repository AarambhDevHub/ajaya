//! Path-based HTTP router backed by a radix trie.
//!
//! The [`Router`] maps URL paths to [`MethodRouter`]s using
//! the `matchit` radix trie for zero-allocation route lookup.
//!
//! # Examples
//!
//! ```rust,ignore
//! use ajaya_router::{Router, get, post};
//!
//! async fn home() -> &'static str { "Home" }
//! async fn list_users() -> &'static str { "Users" }
//! async fn get_user(req: Request) -> String {
//!     let params = req.extension::<PathParams>().unwrap();
//!     format!("User: {}", params.get("id").unwrap())
//! }
//!
//! let app = Router::new()
//!     .route("/", get(home))
//!     .route("/users", get(list_users))
//!     .route("/users/:id", get(get_user));
//! ```

use std::convert::Infallible;

use ajaya_core::handler::ErasedHandler;
use ajaya_core::request::Request;
use ajaya_core::response::{Response, ResponseBuilder};
use http::StatusCode;

use crate::method_router::MethodRouter;
use crate::params::PathParams;
use crate::service::ServiceHandler;

/// Path-based HTTP router.
///
/// Routes incoming requests to [`MethodRouter`]s based on the request
/// URI path using a radix trie (`matchit`) for efficient matching.
///
/// Supports:
/// - Static paths: `/`, `/users`, `/about`
/// - Path parameters: `/users/:id`, `/users/:id/posts/:post_id`
/// - Wildcard catchalls: `/files/*path`
///
/// Returns `404 Not Found` for unmatched paths.
///
/// # Examples
///
/// ```rust,ignore
/// use ajaya_router::{Router, get};
///
/// async fn home() -> &'static str { "Home" }
/// async fn about() -> &'static str { "About" }
///
/// let app = Router::new()
///     .route("/", get(home))
///     .route("/about", get(about));
/// ```
pub struct Router<S = ()> {
    inner: matchit::Router<usize>,
    routes: Vec<(String, MethodRouter<S>)>,
    fallback: Option<Box<dyn ErasedHandler<S>>>,
}

impl<S: Clone + Send + Sync + 'static> Router<S> {
    /// Create a new empty router.
    pub fn new() -> Self {
        Self {
            inner: matchit::Router::new(),
            routes: Vec::new(),
            fallback: None,
        }
    }

    /// Register a route for the given path pattern.
    ///
    /// The path must start with `/`. Supports:
    /// - Static: `/users`
    /// - Parameters: `/users/{id}`
    /// - Wildcards: `/files/{*path}`
    ///
    /// # Panics
    ///
    /// Panics if the path conflicts with an existing route.
    pub fn route(mut self, path: &str, method_router: MethodRouter<S>) -> Self {
        let idx = self.routes.len();
        if let Err(e) = self.inner.insert(path, idx) {
            panic!("Route conflict for `{path}`: {e}");
        }
        self.routes.push((path.to_string(), method_router));
        self
    }

    /// Compose routers by mounting a sub-router under a path prefix.
    ///
    /// All routes from `other` are prepended with `prefix` and
    /// inserted into this router (flatten strategy).
    ///
    /// # Panics
    ///
    /// Panics if any nested route conflicts with an existing route.
    pub fn nest(mut self, prefix: &str, other: Router<S>) -> Self {
        let prefix = prefix.trim_end_matches('/');

        for (path, method_router) in other.routes {
            let full_path = if path == "/" {
                format!("{prefix}/")
            } else {
                format!("{prefix}{path}")
            };
            let idx = self.routes.len();
            if let Err(e) = self.inner.insert(&full_path, idx) {
                panic!("Nested route conflict for `{full_path}`: {e}");
            }
            self.routes.push((full_path, method_router));
        }

        self
    }

    /// Merge all routes from another router into this one.
    ///
    /// # Panics
    ///
    /// Panics if any route in `other` conflicts with a route in `self`.
    pub fn merge(mut self, other: Router<S>) -> Self {
        for (path, method_router) in other.routes {
            let idx = self.routes.len();
            if let Err(e) = self.inner.insert(&path, idx) {
                panic!("Merge conflict for `{path}`: {e}");
            }
            self.routes.push((path, method_router));
        }

        // If other has a fallback and self doesn't, take it
        if self.fallback.is_none() {
            self.fallback = other.fallback;
        }

        self
    }

    /// Set a custom fallback handler for unmatched paths.
    ///
    /// By default, the router returns `404 Not Found` with a plain
    /// text body. Use this to customize the 404 response.
    pub fn fallback<H, T>(mut self, handler: H) -> Self
    where
        H: ajaya_core::handler::Handler<T, S> + Clone + Send + Sync + 'static,
        T: 'static,
    {
        self.fallback = Some(ajaya_core::handler::into_erased(handler));
        self
    }

    /// Register a Tower service at an exact path.
    ///
    /// The service receives the full request URI without modification.
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// use ajaya_router::Router;
    ///
    /// let app = Router::new()
    ///     .route_service("/metrics", prometheus_service);
    /// ```
    pub fn route_service<T>(self, path: &str, service: T) -> Self
    where
        T: tower_service::Service<Request, Response = Response, Error = Infallible>
            + Clone
            + Send
            + Sync
            + 'static,
        T::Future: Send + 'static,
    {
        let handler = ServiceHandler::new(service);
        self.route(path, crate::any(handler))
    }

    /// Mount a Tower service at a path prefix.
    ///
    /// Registers a wildcard catch-all route at `{prefix}/*__rest` so
    /// all sub-paths are forwarded to the service.
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// use ajaya_router::Router;
    ///
    /// let app = Router::new()
    ///     .nest_service("/grpc", tonic_service);
    /// ```
    pub fn nest_service<T>(self, prefix: &str, service: T) -> Self
    where
        T: tower_service::Service<Request, Response = Response, Error = Infallible>
            + Clone
            + Send
            + Sync
            + 'static,
        T::Future: Send + 'static,
    {
        let prefix = prefix.trim_end_matches('/');
        let wildcard_path = format!("{prefix}/*__rest");
        let handler = ServiceHandler::new(service);
        self.route(&wildcard_path, crate::any(handler))
    }

    /// Dispatch a request based on its URI path and HTTP method.
    ///
    /// 1. Match the path against the radix trie
    /// 2. If matched → extract path params into request extensions → delegate to `MethodRouter`
    /// 3. If not matched → return 404 or call fallback handler
    pub async fn call(&self, mut req: Request, state: S) -> Response {
        let path = req.uri().path().to_string();

        match self.inner.at(&path) {
            Ok(matched) => {
                let idx = *matched.value;

                // Extract path params from the match
                let matchit_params = matched.params;
                if !matchit_params.is_empty() {
                    let mut path_params = PathParams::new();
                    for (key, value) in matchit_params.iter() {
                        let decoded = percent_decode(value);
                        path_params.push(key.to_string(), decoded);
                    }
                    req.extensions_mut().insert(path_params);
                }

                // Delegate to the method router
                self.routes[idx].1.call(req, state).await
            }
            Err(_) => {
                // No route matched — use fallback or default 404
                if let Some(fallback) = &self.fallback {
                    let handler = fallback.clone_box();
                    return handler.call(req, state).await;
                }

                ResponseBuilder::new()
                    .status(StatusCode::NOT_FOUND)
                    .header(http::header::CONTENT_TYPE, "text/plain; charset=utf-8")
                    .text("Not Found")
            }
        }
    }
}

impl<S: Clone + Send + Sync + 'static> Default for Router<S> {
    fn default() -> Self {
        Self::new()
    }
}

/// Simple percent-decoding for URL path parameters.
fn percent_decode(input: &str) -> String {
    let mut result = Vec::with_capacity(input.len());
    let bytes = input.as_bytes();
    let mut i = 0;

    while i < bytes.len() {
        if bytes[i] == b'%' && i + 2 < bytes.len() {
            if let (Some(hi), Some(lo)) = (hex_val(bytes[i + 1]), hex_val(bytes[i + 2])) {
                result.push(hi << 4 | lo);
                i += 3;
                continue;
            }
        }
        result.push(bytes[i]);
        i += 1;
    }

    String::from_utf8(result).unwrap_or_else(|_| input.to_string())
}

fn hex_val(byte: u8) -> Option<u8> {
    match byte {
        b'0'..=b'9' => Some(byte - b'0'),
        b'a'..=b'f' => Some(byte - b'a' + 10),
        b'A'..=b'F' => Some(byte - b'A' + 10),
        _ => None,
    }
}

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
        let _router: Router<()> = Router::new()
            .route("/test", crate::get(handler))
            .route("/test", crate::get(handler));
    }

    #[test]
    fn test_static_routes() {
        async fn handler() -> &'static str {
            "test"
        }
        let _router: Router<()> = Router::new()
            .route("/", crate::get(handler))
            .route("/users", crate::get(handler))
            .route("/about", crate::get(handler));
    }

    #[test]
    fn test_param_routes() {
        async fn handler() -> &'static str {
            "test"
        }
        let _router: Router<()> = Router::new()
            .route("/users/{id}", crate::get(handler))
            .route("/users/{id}/posts/{post_id}", crate::get(handler));
    }

    #[test]
    fn test_wildcard_routes() {
        async fn handler() -> &'static str {
            "test"
        }
        let _router: Router<()> = Router::new()
            .route("/files/{*path}", crate::get(handler))
            .route("/", crate::get(handler));
    }

    #[test]
    fn test_nest() {
        async fn handler() -> &'static str {
            "test"
        }
        let sub = Router::new()
            .route("/", crate::get(handler))
            .route("/{id}", crate::get(handler));

        let _app: Router<()> = Router::new()
            .route("/", crate::get(handler))
            .nest("/users", sub);
    }

    #[test]
    fn test_merge() {
        async fn handler() -> &'static str {
            "test"
        }
        let api = Router::new().route("/users", crate::get(handler));
        let admin = Router::new().route("/admin", crate::get(handler));

        let _app: Router<()> = api.merge(admin);
    }

    #[test]
    #[should_panic(expected = "Merge conflict")]
    fn test_merge_conflict_panics() {
        async fn handler() -> &'static str {
            "test"
        }
        let a = Router::new().route("/users", crate::get(handler));
        let b = Router::new().route("/users", crate::get(handler));
        let _app: Router<()> = a.merge(b);
    }
}
