//! # Ajaya (अजय) — The Unconquerable Rust Web Framework
//!
//! Ajaya is a high-performance web framework built on Tokio and Hyper,
//! engineered to be the fastest and most ergonomic Rust web framework.
//!
//! ## Quick Start
//!
//! ```rust,ignore
//! use ajaya::serve;
//!
//! #[tokio::main]
//! async fn main() {
//!     serve("0.0.0.0:8080").await.unwrap();
//! }
//! ```
//!
//! ## Feature Highlights
//!
//! - **Zero-cost routing** via radix trie (coming in v0.1.x)
//! - **Type-safe extractors** for path, query, JSON, form (coming in v0.2.x)
//! - **Tower-compatible middleware** (coming in v0.4.x)
//! - **WebSocket & SSE** support (coming in v0.5.x)
//! - **Production-grade TLS** via rustls (coming in v0.6.x)

// Re-export core types
pub use ajaya_core::*;

// Re-export server functionality
pub use ajaya_hyper::serve;
pub use ajaya_hyper::Server;
