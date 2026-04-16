//! Raw body and full request extractors.
//!
//! These extractors are implemented in `ajaya-core` due to Rust's
//! orphan rule (the types and traits are both in `ajaya-core`).
//! This module is kept as a placeholder for documentation purposes.
//!
//! To use these extractors, simply put the type as a handler parameter:
//!
//! ```rust,ignore
//! use ajaya::{Body, Request};
//!
//! async fn body_handler(body: Body) -> &'static str { "ok" }
//! async fn request_handler(req: Request) -> String {
//!     format!("{}", req.uri())
//! }
//! ```

/// Marker type for the body extractor module.
pub struct BodyExtractor;
