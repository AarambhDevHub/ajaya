//! # Ajaya (अजय) — The Unconquerable Rust Web Framework
//!
//! Ajaya is a high-performance web framework built on Tokio and Hyper,
//! engineered to be the fastest and most ergonomic Rust web framework.
//!
//! ## Quick Start
//!
//! ```rust,ignore
//! use ajaya::{serve, get};
//!
//! async fn hello() -> &'static str {
//!     "Hello from Ajaya!"
//! }
//!
//! #[tokio::main]
//! async fn main() {
//!     serve("0.0.0.0:8080", hello).await.unwrap();
//! }
//! ```
//!
//! ## Method Routing
//!
//! ```rust,ignore
//! use ajaya::{get, post, serve_router};
//!
//! async fn hello() -> &'static str { "Hello!" }
//! async fn create() -> &'static str { "Created!" }
//!
//! #[tokio::main]
//! async fn main() {
//!     let router = get(hello).post(create);
//!     serve_router("0.0.0.0:8080", router).await.unwrap();
//! }
//! ```
//!
//! ## Error Handling
//!
//! ```rust,ignore
//! use ajaya::{get, serve_router, Json, Error};
//!
//! async fn handler() -> Result<Json<serde_json::Value>, Error> {
//!     let data = serde_json::json!({ "name": "Ajaya" });
//!     Ok(Json(data))
//! }
//! ```
//!
//! ## Feature Highlights
//!
//! - **Type-safe handlers** — any async fn → handler (v0.0.3+)
//! - **Method dispatch** — `get()`, `post()`, `put()`, `delete()` (v0.0.4+)
//! - **Error handling** — `Result<T, E>` in handlers with `?` (v0.0.5+)
//! - **JSON responses** — `Json<T>` for automatic serialization (v0.0.5+)
//! - **Zero-cost routing** via radix trie (coming in v0.1.x)
//! - **Type-safe extractors** for path, query, JSON, form (coming in v0.2.x)
//! - **Tower-compatible middleware** (coming in v0.4.x)
//! - **WebSocket & SSE** support (coming in v0.5.x)
//! - **Production-grade TLS** via rustls (coming in v0.6.x)

// Re-export core types
pub use ajaya_core::Body;
pub use ajaya_core::Error;
pub use ajaya_core::Handler;
pub use ajaya_core::Html;
pub use ajaya_core::IntoResponse;
pub use ajaya_core::Json;
pub use ajaya_core::MethodFilter;
pub use ajaya_core::Redirect;
pub use ajaya_core::Request;
pub use ajaya_core::Response;
pub use ajaya_core::ResponseBuilder;

// Re-export router types
pub use ajaya_router::MethodRouter;
pub use ajaya_router::{any, delete, get, head, on, options, patch, post, put, trace_method};

// Re-export server functionality
pub use ajaya_hyper::Server;
pub use ajaya_hyper::{serve, serve_router};
