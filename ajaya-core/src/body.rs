//! Unified HTTP body type.
//!
//! Provides a `Body` type that wraps a boxed bytes stream,
//! suitable for both requests and responses.
//!
//! # Future (v0.0.2)
//! - `Body::empty()`, `Body::from_bytes()`, `Body::from_stream()`
//! - `Body::to_bytes()`, `Body::to_string()` async methods
//! - `LimitedBody` for request body size limits

use bytes::Bytes;
use http_body_util::Full;

/// Ajaya's unified HTTP body type.
///
/// Currently wraps [`Full<Bytes>`] for simplicity.
/// Will be replaced with a boxed stream body in v0.0.2.
pub type Body = Full<Bytes>;
