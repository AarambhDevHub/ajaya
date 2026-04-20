//! Convenience serve functions.
//!
//! One-liners to start the Ajaya server.

use ajaya_core::handler::Handler;
use ajaya_router::layer::BoxCloneService;
use ajaya_router::{MethodRouter, Router};

use crate::Server;

/// Start the server with a bare handler (no routing).
pub async fn serve<H, T>(
    addr: &str,
    handler: H,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>>
where
    H: Handler<T> + Clone + Send + Sync + 'static,
    T: 'static,
{
    Server::bind(addr).await?.serve(handler).await
}

/// Start the server with a [`MethodRouter`].
pub async fn serve_router(
    addr: &str,
    router: MethodRouter,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    Server::bind(addr).await?.serve_method_router(router).await
}

/// Start the server with a [`Router`] — the standard entry point.
///
/// Calls [`Router::into_service`] internally, so all `.layer()`,
/// `.route_layer()`, and `.with_state()` configurations are applied.
pub async fn serve_app(
    addr: &str,
    router: Router,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    Server::bind(addr).await?.serve_app(router).await
}

/// Start the server with a pre-built Tower [`BoxCloneService`].
///
/// Useful when you've manually composed middleware via `router.into_service()`.
pub async fn serve_service(
    addr: &str,
    service: BoxCloneService,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    Server::bind(addr).await?.serve_service(service).await
}
