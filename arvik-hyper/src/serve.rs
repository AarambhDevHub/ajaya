//! Convenience serve functions.
//!
//! One-liners to start the Arvik server.

use arvik_core::handler::Handler;
use arvik_router::layer::BoxCloneService;
use arvik_router::{MethodRouter, Router};

use crate::{Server, ServerConfig, ShutdownConfig};

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

/// Start the server with a bare handler and explicit low-level server tuning.
pub async fn serve_handler_with_config<H, T>(
    addr: &str,
    handler: H,
    config: ServerConfig,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>>
where
    H: Handler<T> + Clone + Send + Sync + 'static,
    T: 'static,
{
    Server::bind_with_config(addr, config)
        .await?
        .serve(handler)
        .await
}

/// Start the server with a bare handler and graceful shutdown.
pub async fn serve_handler_with_graceful_shutdown<H, T, F>(
    addr: &str,
    handler: H,
    signal: F,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>>
where
    H: Handler<T> + Clone + Send + Sync + 'static,
    T: 'static,
    F: std::future::Future<Output = ()> + Send,
{
    serve_handler_with_config_and_graceful_shutdown(
        addr,
        handler,
        ServerConfig::default(),
        ShutdownConfig::default(),
        signal,
    )
    .await
}

/// Start the server with a bare handler, explicit tuning, and graceful shutdown.
pub async fn serve_handler_with_config_and_graceful_shutdown<H, T, F>(
    addr: &str,
    handler: H,
    config: ServerConfig,
    shutdown_config: ShutdownConfig,
    signal: F,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>>
where
    H: Handler<T> + Clone + Send + Sync + 'static,
    T: 'static,
    F: std::future::Future<Output = ()> + Send,
{
    Server::bind_with_config(addr, config)
        .await?
        .serve_with_graceful_shutdown(handler, signal, shutdown_config)
        .await
}

/// Start the server with a [`MethodRouter`].
pub async fn serve_router(
    addr: &str,
    router: MethodRouter,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    Server::bind(addr).await?.serve_method_router(router).await
}

/// Start the server with a [`MethodRouter`] and explicit low-level server tuning.
pub async fn serve_router_with_config(
    addr: &str,
    router: MethodRouter,
    config: ServerConfig,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    Server::bind_with_config(addr, config)
        .await?
        .serve_method_router(router)
        .await
}

/// Start the server with a [`MethodRouter`] and graceful shutdown.
pub async fn serve_router_with_graceful_shutdown<F>(
    addr: &str,
    router: MethodRouter,
    signal: F,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>>
where
    F: std::future::Future<Output = ()> + Send,
{
    serve_router_with_config_and_graceful_shutdown(
        addr,
        router,
        ServerConfig::default(),
        ShutdownConfig::default(),
        signal,
    )
    .await
}

/// Start the server with a [`MethodRouter`], explicit tuning, and graceful shutdown.
pub async fn serve_router_with_config_and_graceful_shutdown<F>(
    addr: &str,
    router: MethodRouter,
    config: ServerConfig,
    shutdown_config: ShutdownConfig,
    signal: F,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>>
where
    F: std::future::Future<Output = ()> + Send,
{
    Server::bind_with_config(addr, config)
        .await?
        .serve_method_router_with_graceful_shutdown(router, signal, shutdown_config)
        .await
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

/// Start the server with a [`Router`] and explicit low-level server tuning.
pub async fn serve_with_config(
    router: Router,
    addr: &str,
    config: ServerConfig,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    Server::bind_with_config(addr, config)
        .await?
        .serve_app(router)
        .await
}

/// Start the server with a [`Router`] and graceful shutdown.
pub async fn serve_with_graceful_shutdown<F>(
    router: Router,
    addr: &str,
    signal: F,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>>
where
    F: std::future::Future<Output = ()> + Send,
{
    serve_with_config_and_graceful_shutdown(
        router,
        addr,
        ServerConfig::default(),
        ShutdownConfig::default(),
        signal,
    )
    .await
}

/// Start the server with a [`Router`], explicit tuning, and graceful shutdown.
pub async fn serve_with_config_and_graceful_shutdown<F>(
    router: Router,
    addr: &str,
    config: ServerConfig,
    shutdown_config: ShutdownConfig,
    signal: F,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>>
where
    F: std::future::Future<Output = ()> + Send,
{
    Server::bind_with_config(addr, config)
        .await?
        .serve_app_with_graceful_shutdown(router, signal, shutdown_config)
        .await
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

/// Start the server with a pre-built Tower service and explicit low-level server tuning.
pub async fn serve_service_with_config(
    addr: &str,
    service: BoxCloneService,
    config: ServerConfig,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    Server::bind_with_config(addr, config)
        .await?
        .serve_service(service)
        .await
}

/// Start the server with a pre-built Tower service and graceful shutdown.
pub async fn serve_service_with_graceful_shutdown<F>(
    addr: &str,
    service: BoxCloneService,
    signal: F,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>>
where
    F: std::future::Future<Output = ()> + Send,
{
    serve_service_with_config_and_graceful_shutdown(
        addr,
        service,
        ServerConfig::default(),
        ShutdownConfig::default(),
        signal,
    )
    .await
}

/// Start the server with a pre-built Tower service, explicit tuning, and graceful shutdown.
pub async fn serve_service_with_config_and_graceful_shutdown<F>(
    addr: &str,
    service: BoxCloneService,
    config: ServerConfig,
    shutdown_config: ShutdownConfig,
    signal: F,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>>
where
    F: std::future::Future<Output = ()> + Send,
{
    Server::bind_with_config(addr, config)
        .await?
        .serve_service_with_graceful_shutdown(service, signal, shutdown_config)
        .await
}

/// Start an HTTPS server with rustls and default low-level tuning.
#[cfg(feature = "tls")]
pub async fn serve_tls(
    router: Router,
    addr: &str,
    tls_config: arvik_tls::rustls::RustlsConfig,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    serve_tls_with_config(router, addr, tls_config, ServerConfig::default()).await
}

/// Start an HTTPS server with rustls and explicit low-level tuning.
#[cfg(feature = "tls")]
pub async fn serve_tls_with_config(
    router: Router,
    addr: &str,
    tls_config: arvik_tls::rustls::RustlsConfig,
    server_config: ServerConfig,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    Server::bind_with_config(addr, server_config)
        .await?
        .serve_tls_app(router, tls_config)
        .await
}

/// Start an HTTPS server with rustls and graceful shutdown.
#[cfg(feature = "tls")]
pub async fn serve_tls_with_graceful_shutdown<F>(
    router: Router,
    addr: &str,
    tls_config: arvik_tls::rustls::RustlsConfig,
    signal: F,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>>
where
    F: std::future::Future<Output = ()> + Send,
{
    serve_tls_with_config_and_graceful_shutdown(
        router,
        addr,
        tls_config,
        ServerConfig::default(),
        ShutdownConfig::default(),
        signal,
    )
    .await
}

/// Start an HTTPS server with rustls, explicit tuning, and graceful shutdown.
#[cfg(feature = "tls")]
pub async fn serve_tls_with_config_and_graceful_shutdown<F>(
    router: Router,
    addr: &str,
    tls_config: arvik_tls::rustls::RustlsConfig,
    server_config: ServerConfig,
    shutdown_config: ShutdownConfig,
    signal: F,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>>
where
    F: std::future::Future<Output = ()> + Send,
{
    Server::bind_with_config(addr, server_config)
        .await?
        .serve_tls_app_with_graceful_shutdown(router, tls_config, signal, shutdown_config)
        .await
}

/// Start an HTTPS server with native-tls and default low-level tuning.
#[cfg(feature = "native-tls")]
pub async fn serve_native_tls(
    router: Router,
    addr: &str,
    tls_config: arvik_tls::native::NativeTlsConfig,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    serve_native_tls_with_config(router, addr, tls_config, ServerConfig::default()).await
}

/// Start an HTTPS server with native-tls and explicit low-level tuning.
#[cfg(feature = "native-tls")]
pub async fn serve_native_tls_with_config(
    router: Router,
    addr: &str,
    tls_config: arvik_tls::native::NativeTlsConfig,
    server_config: ServerConfig,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    Server::bind_with_config(addr, server_config)
        .await?
        .serve_native_tls_app(router, tls_config)
        .await
}

/// Start an HTTPS server with native-tls and graceful shutdown.
#[cfg(feature = "native-tls")]
pub async fn serve_native_tls_with_graceful_shutdown<F>(
    router: Router,
    addr: &str,
    tls_config: arvik_tls::native::NativeTlsConfig,
    signal: F,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>>
where
    F: std::future::Future<Output = ()> + Send,
{
    serve_native_tls_with_config_and_graceful_shutdown(
        router,
        addr,
        tls_config,
        ServerConfig::default(),
        ShutdownConfig::default(),
        signal,
    )
    .await
}

/// Start an HTTPS server with native-tls, explicit tuning, and graceful shutdown.
#[cfg(feature = "native-tls")]
pub async fn serve_native_tls_with_config_and_graceful_shutdown<F>(
    router: Router,
    addr: &str,
    tls_config: arvik_tls::native::NativeTlsConfig,
    server_config: ServerConfig,
    shutdown_config: ShutdownConfig,
    signal: F,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>>
where
    F: std::future::Future<Output = ()> + Send,
{
    Server::bind_with_config(addr, server_config)
        .await?
        .serve_native_tls_app_with_graceful_shutdown(router, tls_config, signal, shutdown_config)
        .await
}

/// Start an HTTP/2 cleartext prior-knowledge server with default low-level tuning.
#[cfg(feature = "http2")]
pub async fn serve_h2c(
    router: Router,
    addr: &str,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    serve_h2c_with_config(router, addr, ServerConfig::default()).await
}

/// Start an HTTP/2 cleartext prior-knowledge server with explicit tuning.
#[cfg(feature = "http2")]
pub async fn serve_h2c_with_config(
    router: Router,
    addr: &str,
    server_config: ServerConfig,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    Server::bind_with_config(addr, server_config.http2_only(true))
        .await?
        .serve_app(router)
        .await
}

/// Start an HTTP/2 cleartext prior-knowledge server with graceful shutdown.
#[cfg(feature = "http2")]
pub async fn serve_h2c_with_graceful_shutdown<F>(
    router: Router,
    addr: &str,
    signal: F,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>>
where
    F: std::future::Future<Output = ()> + Send,
{
    serve_h2c_with_config_and_graceful_shutdown(
        router,
        addr,
        ServerConfig::default(),
        ShutdownConfig::default(),
        signal,
    )
    .await
}

/// Start an HTTP/2 cleartext prior-knowledge server with explicit tuning and graceful shutdown.
#[cfg(feature = "http2")]
pub async fn serve_h2c_with_config_and_graceful_shutdown<F>(
    router: Router,
    addr: &str,
    server_config: ServerConfig,
    shutdown_config: ShutdownConfig,
    signal: F,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>>
where
    F: std::future::Future<Output = ()> + Send,
{
    Server::bind_with_config(addr, server_config.http2_only(true))
        .await?
        .serve_app_with_graceful_shutdown(router, signal, shutdown_config)
        .await
}
