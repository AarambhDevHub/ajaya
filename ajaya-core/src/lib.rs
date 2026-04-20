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
//! - [`FromRequest`] — Trait for body-consuming extractors
//! - [`FromRequestParts`] — Trait for parts-only extractors
//! - [`RequestParts`] — Non-body request parts for extractors
//! - [`MethodFilter`] — HTTP method matching
//! - [`Json`] — JSON response type
//! - [`Html`] — HTML response type
//! - [`Error`] — Framework error type

pub mod body;
pub mod error;
pub mod extract;
pub mod handler;
pub mod into_response;
pub mod into_response_parts;
pub mod method_filter;
pub mod request;
pub mod request_parts;
pub mod response;
pub mod stream_body;

// Re-exports
pub use body::Body;
pub use error::{Error, ErrorResponse};
pub use extract::{FromRequest, FromRequestParts};
pub use handler::Handler;
pub use into_response::{Html, IntoResponse, Json};
pub use into_response_parts::{AppendHeaders, IntoResponseParts, ResponseParts};
pub use method_filter::MethodFilter;
pub use request::Request;
pub use request_parts::RequestParts;
pub use response::{Redirect, Response, ResponseBuilder};
pub use stream_body::StreamBody;
