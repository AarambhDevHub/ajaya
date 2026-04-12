//! # Ajaya (अजय) — The Unconquerable Rust Web Framework
//!
//! Ajaya is a high-performance web framework built on Tokio and Hyper,
//! engineered to be the fastest and most ergonomic Rust web framework.
//!
//! ## Quick Start
//!
//! ```rust,ignore
//! use ajaya::{Router, get, serve_app};
//!
//! async fn home() -> &'static str { "Hello from Ajaya!" }
//! async fn about() -> &'static str { "About Ajaya" }
//!
//! #[tokio::main]
//! async fn main() {
//!     let app = Router::new()
//!         .route("/", get(home))
//!         .route("/about", get(about));
//!     serve_app("0.0.0.0:8080", app).await.unwrap();
//! }
//! ```
//!
//! ## Method Routing (single path)
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
//! use ajaya::{Router, get, serve_app, Json, Error};
//!
//! async fn handler() -> Result<Json<serde_json::Value>, Error> {
//!     let data = serde_json::json!({ "name": "Ajaya" });
//!     Ok(Json(data))
//! }
//! ```

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
pub use ajaya_router::PathParams;
pub use ajaya_router::Router;
pub use ajaya_router::{any, delete, get, head, on, options, patch, post, put, trace_method};

// Re-export server functionality
pub use ajaya_hyper::Server;
pub use ajaya_hyper::{serve, serve_app, serve_router};
