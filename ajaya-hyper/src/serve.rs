//! Convenience serve functions.
//!
//! One-liners to start the Ajaya server.

use ajaya_core::handler::Handler;
use ajaya_router::MethodRouter;
use ajaya_router::Router;

use crate::Server;

/// Start the Ajaya HTTP server on the given address with a handler.
///
/// This is a convenience wrapper around [`Server::bind`] and [`Server::serve`].
///
/// # Example
///
/// ```rust,ignore
/// async fn hello() -> &'static str { "Hello!" }
///
/// ajaya_hyper::serve("0.0.0.0:8080", hello).await.unwrap();
/// ```
pub async fn serve<H, T>(
    addr: &str,
    handler: H,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>>
where
    H: Handler<T> + Clone + Send + Sync + 'static,
    T: 'static,
{
    let server = Server::bind(addr).await?;
    server.serve(handler).await
}

/// Start the Ajaya HTTP server on the given address with a [`MethodRouter`].
///
/// Dispatches requests based on HTTP method. Returns `405 Method Not Allowed`
/// for unregistered methods.
///
/// # Example
///
/// ```rust,ignore
/// use ajaya_router::{get, post};
///
/// async fn hello() -> &'static str { "Hello!" }
/// async fn create() -> &'static str { "Created!" }
///
/// let router = get(hello).post(create);
/// ajaya_hyper::serve_router("0.0.0.0:8080", router).await.unwrap();
/// ```
pub async fn serve_router(
    addr: &str,
    router: MethodRouter,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let server = Server::bind(addr).await?;
    server.serve_method_router(router).await
}

/// Start the Ajaya HTTP server on the given address with a [`Router`].
///
/// Dispatches requests based on path and HTTP method. Returns `404 Not Found`
/// for unmatched paths and `405 Method Not Allowed` for unmatched methods.
///
/// # Example
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
/// ajaya_hyper::serve_app("0.0.0.0:8080", app).await.unwrap();
/// ```
pub async fn serve_app(
    addr: &str,
    router: Router,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let server = Server::bind(addr).await?;
    server.serve_app(router).await
}
