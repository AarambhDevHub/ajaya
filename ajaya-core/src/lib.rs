//! # ajaya-core
//!
//! Core traits and types for the Ajaya web framework.
//!
//! This crate provides the foundational abstractions:
//! - [`Request`] — HTTP request wrapper
//! - [`Response`] — HTTP response type alias
//! - [`Body`] — Unified request/response body
//! - [`ResponseBuilder`] — Ergonomic response construction
//! - [`IntoResponse`] — Trait for handler return types
//! - [`Handler`] — Trait for request handlers
//! - [`MethodFilter`] — HTTP method matching
//! - [`Json`] — JSON response type
//! - [`Html`] — HTML response type
//! - [`Error`] — Framework error type

pub mod body;
pub mod error;
pub mod handler;
pub mod into_response;
pub mod method_filter;
pub mod request;
pub mod response;

// Re-exports
pub use body::Body;
pub use error::Error;
pub use handler::Handler;
pub use into_response::{Html, IntoResponse, Json};
pub use method_filter::MethodFilter;
pub use request::Request;
pub use response::{Redirect, Response, ResponseBuilder};
