//! URI and HTTP Version extractors.
//!
//! The `FromRequestParts` impls for `http::Uri` and `http::Version`
//! live in `ajaya-core` due to Rust's orphan rule. To use them:
//!
//! ```rust,ignore
//! use http::{Uri, Version};
//!
//! async fn handler(uri: Uri, version: Version) -> String {
//!     format!("URI: {uri}, Version: {version:?}")
//! }
//! ```
