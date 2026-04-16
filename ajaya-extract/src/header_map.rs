//! HeaderMap extractor.
//!
//! The `FromRequestParts` impl for `http::HeaderMap` lives in
//! `ajaya-core` due to Rust's orphan rule. To use it:
//!
//! ```rust,ignore
//! use http::HeaderMap;
//!
//! async fn handler(headers: HeaderMap) -> String {
//!     let ua = headers.get("user-agent")
//!         .and_then(|v| v.to_str().ok())
//!         .unwrap_or("unknown");
//!     format!("User-Agent: {ua}")
//! }
//! ```
