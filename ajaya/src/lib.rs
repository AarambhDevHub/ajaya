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
//! ## Extractors
//!
//! ```rust,ignore
//! use ajaya::{Router, get, post, Json, Path, Query, State};
//! use serde::{Deserialize, Serialize};
//!
//! #[derive(Clone)]
//! struct AppState { db_url: String }
//!
//! #[derive(Deserialize)]
//! struct CreateUser { name: String }
//!
//! #[derive(Serialize)]
//! struct User { id: u32, name: String }
//!
//! async fn get_user(Path(id): Path<u32>) -> Json<User> {
//!     Json(User { id, name: "Alice".into() })
//! }
//!
//! async fn create_user(
//!     State(state): State<AppState>,
//!     Json(body): Json<CreateUser>,
//! ) -> Json<User> {
//!     Json(User { id: 1, name: body.name })
//! }
//! ```
//!
//! ## Error Handling
//!
//! ```rust,ignore
//! use ajaya::{Router, get, Json, Error};
//!
//! async fn handler() -> Result<Json<serde_json::Value>, Error> {
//!     let data = serde_json::json!({ "name": "Ajaya" });
//!     Ok(Json(data))
//! }
//! ```

// ---------------------------------------------------------------------------
// Core types
// ---------------------------------------------------------------------------
pub use ajaya_core::Body;
pub use ajaya_core::Error;
pub use ajaya_core::ErrorResponse;
pub use ajaya_core::Handler;
pub use ajaya_core::Html;
pub use ajaya_core::IntoResponse;
pub use ajaya_core::MethodFilter;
pub use ajaya_core::Redirect;
pub use ajaya_core::Request;
pub use ajaya_core::RequestParts;
pub use ajaya_core::Response;
pub use ajaya_core::ResponseBuilder;
pub use ajaya_core::StreamBody;

// IntoResponseParts system
pub use ajaya_core::AppendHeaders; // 0.3.2
pub use ajaya_core::IntoResponseParts; // 0.3.2
pub use ajaya_core::ResponseParts; // 0.3.2

// Extractor traits
pub use ajaya_core::FromRequest;
pub use ajaya_core::FromRequestParts;

// ---------------------------------------------------------------------------
// Router types
// ---------------------------------------------------------------------------
pub use ajaya_router::MethodRouter;
pub use ajaya_router::PathParams;
pub use ajaya_router::Router;
pub use ajaya_router::{any, delete, get, head, on, options, patch, post, put, trace_method};

// ---------------------------------------------------------------------------
// Extractors (from ajaya-extract)
// ---------------------------------------------------------------------------

// Path & Query
pub use ajaya_extract::Path;
pub use ajaya_extract::Query;
pub use ajaya_extract::RawPathParams;

// Headers
pub use ajaya_extract::TypedHeader;
// HeaderMap is from http crate — users get it via `use http::HeaderMap`

// Request metadata
pub use ajaya_extract::ConnectInfo;
pub use ajaya_extract::Extension;
pub use ajaya_extract::MatchedPath;
pub use ajaya_extract::OriginalUri;

// Body extractors — Json from extract (has both FromRequest + IntoResponse)
pub use ajaya_extract::Form;
pub use ajaya_extract::Json;

// State
pub use ajaya_extract::FromRef;
pub use ajaya_extract::State;

// Cookies  ← 0.3.3
pub use ajaya_extract::CookieJar;
pub use ajaya_extract::PrivateCookieJar;
pub use ajaya_extract::SignedCookieJar;
// Re-export cookie::Key so users don't need a direct cookie dep
pub use cookie::Cookie;
pub use cookie::Key as CookieKey;

// Multipart
pub use ajaya_extract::Field;
pub use ajaya_extract::Multipart;
pub use ajaya_extract::MultipartConstraints;

// Rejections (for custom error handling)
pub use ajaya_extract::rejection;

// ---------------------------------------------------------------------------
// Server functionality
// ---------------------------------------------------------------------------
pub use ajaya_hyper::Server;
pub use ajaya_hyper::{serve, serve_app, serve_router};

// ---------------------------------------------------------------------------
// Middleware  (0.4.x)
// ---------------------------------------------------------------------------
pub use ajaya_middleware::CorsLayer;

// Tower layer / service primitives (for custom middleware authors)
pub use ajaya_router::layer::{BoxCloneService, LayerFn};
