//! # arvik-hyper
//!
//! Hyper 1.x server integration for the Arvik web framework.
//!
//! This crate provides:
//! - TCP listener and connection management
//! - Hyper service integration
//! - Handler-based serving ([`serve()`](crate::serve()))
//! - Method router-based serving ([`serve_router`])
//! - Graceful shutdown support
//!
//! # Quick Start
//!
//! ```rust,ignore
//! use arvik_hyper::serve;
//!
//! async fn hello() -> &'static str { "Hello!" }
//!
//! #[tokio::main]
//! async fn main() {
//!     serve("0.0.0.0:8080", hello).await.unwrap();
//! }
//! ```

pub mod config;
pub mod graceful;
pub mod runtime;
pub mod serve;
pub mod server;

pub use config::ServerConfig;
pub use graceful::{ConnectionInfo, ShutdownConfig, default_shutdown_signal};
pub use runtime::RuntimeConfig;
#[cfg(feature = "runtime-metrics")]
pub use runtime::RuntimeMetricsHandle;
pub use serve::{
    serve, serve_app, serve_handler_with_config, serve_handler_with_config_and_graceful_shutdown,
    serve_handler_with_graceful_shutdown, serve_router, serve_router_with_config,
    serve_router_with_config_and_graceful_shutdown, serve_router_with_graceful_shutdown,
    serve_service, serve_service_with_config, serve_service_with_config_and_graceful_shutdown,
    serve_service_with_graceful_shutdown, serve_with_config,
    serve_with_config_and_graceful_shutdown, serve_with_graceful_shutdown,
};
#[cfg(feature = "http2")]
pub use serve::{
    serve_h2c, serve_h2c_with_config, serve_h2c_with_config_and_graceful_shutdown,
    serve_h2c_with_graceful_shutdown,
};
#[cfg(feature = "native-tls")]
pub use serve::{
    serve_native_tls, serve_native_tls_with_config,
    serve_native_tls_with_config_and_graceful_shutdown, serve_native_tls_with_graceful_shutdown,
};
#[cfg(feature = "tls")]
pub use serve::{
    serve_tls, serve_tls_with_config, serve_tls_with_config_and_graceful_shutdown,
    serve_tls_with_graceful_shutdown,
};
pub use server::Server;
