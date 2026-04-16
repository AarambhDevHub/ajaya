//! HTTP method extractor.
//!
//! The `FromRequestParts` impl for `http::Method` lives in
//! `ajaya-core` due to Rust's orphan rule. To use it:
//!
//! ```rust,ignore
//! use http::Method;
//!
//! async fn handler(method: Method) -> String {
//!     format!("Method: {method}")
//! }
//! ```

/// Marker type for the Method extractor module (for re-export).
pub struct MethodExtractor;
