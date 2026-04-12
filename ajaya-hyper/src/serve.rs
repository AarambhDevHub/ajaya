//! Convenience serve functions.
//!
//! One-liners to start the Ajaya server.

use ajaya_core::handler::Handler;
use ajaya_router::MethodRouter;

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
