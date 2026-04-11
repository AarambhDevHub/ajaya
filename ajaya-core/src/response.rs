//! HTTP Response types.
//!
//! Provides the `Response` type alias and a `ResponseBuilder`
//! for ergonomic response construction.
//!
//! # Future (v0.0.2)
//! - `ResponseBuilder` with `.status()`, `.header()`, `.json()`, `.html()`
//! - `IntoResponse` trait implementations

/// Ajaya's HTTP response type.
///
/// A type alias for [`http::Response`] using Ajaya's [`Body`](crate::Body) type.
pub type Response<B = crate::Body> = http::Response<B>;
