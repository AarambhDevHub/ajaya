//! # ajaya-hyper
//!
//! Hyper 1.x server integration for the Ajaya web framework.
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
//! use ajaya_hyper::serve;
//!
//! async fn hello() -> &'static str { "Hello!" }
//!
//! #[tokio::main]
//! async fn main() {
//!     serve("0.0.0.0:8080", hello).await.unwrap();
//! }
//! ```

pub mod serve;
pub mod server;

pub use serve::{serve, serve_app, serve_router};
pub use server::Server;
