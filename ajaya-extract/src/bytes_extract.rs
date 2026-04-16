//! Raw body bytes and string extractors.
//!
//! These extractors are implemented in `ajaya-core` due to Rust's
//! orphan rule. To use them, simply put the type as a handler parameter:
//!
//! ```rust,ignore
//! use bytes::Bytes;
//!
//! async fn bytes_handler(body: Bytes) -> String {
//!     format!("Received {} bytes", body.len())
//! }
//!
//! async fn string_handler(body: String) -> String {
//!     format!("Body: {body}")
//! }
//! ```
