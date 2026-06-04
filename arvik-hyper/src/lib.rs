//! # arvik-hyper
//!
//! Hyper 1.x server integration for the Arvik web framework.
//!
//! This crate provides:
//! - TCP listener and connection management
//! - Hyper service integration
//! - Handler-based serving ([`serve`])
//! - Method router-based serving ([`serve_router`])
//! - Graceful shutdown support (future)
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
pub mod serve;
pub mod server;

pub use config::ServerConfig;
pub use serve::{
    serve, serve_app, serve_handler_with_config, serve_router, serve_router_with_config,
    serve_service, serve_service_with_config, serve_with_config,
};
#[cfg(feature = "http2")]
pub use serve::{serve_h2c, serve_h2c_with_config};
#[cfg(feature = "native-tls")]
pub use serve::{serve_native_tls, serve_native_tls_with_config};
#[cfg(feature = "tls")]
pub use serve::{serve_tls, serve_tls_with_config};
pub use server::Server;
