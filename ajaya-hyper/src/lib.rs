//! # ajaya-hyper
//!
//! Hyper 1.x server integration for the Ajaya web framework.
//!
//! This crate provides:
//! - TCP listener and connection management
//! - Hyper service integration
//! - Graceful shutdown support (future)
//!
//! # Quick Start
//!
//! ```rust,ignore
//! use ajaya_hyper::serve;
//!
//! #[tokio::main]
//! async fn main() {
//!     serve("0.0.0.0:8080").await.unwrap();
//! }
//! ```

pub mod server;
pub mod serve;

pub use serve::serve;
pub use server::Server;
