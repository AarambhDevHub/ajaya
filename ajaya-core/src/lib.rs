//! # ajaya-core
//!
//! Core traits and types for the Ajaya web framework.
//!
//! This crate provides the foundational abstractions:
//! - [`Request`] — HTTP request wrapper
//! - [`Response`] — HTTP response type
//! - [`Body`] — Unified request/response body
//! - [`Error`] — Framework error type

pub mod body;
pub mod error;
pub mod request;
pub mod response;

// Re-exports
pub use body::Body;
pub use error::Error;
pub use request::Request;
pub use response::Response;
