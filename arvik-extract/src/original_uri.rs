//! Original URI extractor.
//!
//! Extracts the original request URI before any path rewrites
//! performed by router nesting.
//!
//! # Examples
//!
//! ```rust,ignore
//! use arvik::OriginalUri;
//!
//! async fn handler(OriginalUri(uri): OriginalUri) -> String {
//!     format!("Original URI: {uri}")
//! }
//! ```

pub use arvik_core::OriginalUri;
