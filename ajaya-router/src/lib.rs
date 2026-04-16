//! # ajaya-router
//!
//! Routing for the Ajaya web framework.
//!
//! This crate provides:
//! - [`Router`] — Path-based HTTP router with radix trie lookup
//! - [`MethodRouter`] — HTTP method-based dispatch for a single route
//! - [`PathParams`] — Extracted path parameters from route matching
//! - Top-level constructor functions: [`get`], [`post`], [`put`], [`delete`], etc.
//!
//! ## Quick Start
//!
//! ```rust,ignore
//! use ajaya_router::{Router, get, post};
//!
//! async fn home() -> &'static str { "Home" }
//! async fn list_users() -> &'static str { "Users" }
//! async fn create_user() -> &'static str { "Created" }
//!
//! let app = Router::new()
//!     .route("/", get(home))
//!     .route("/users", get(list_users).post(create_user));
//! ```

pub mod method_router;
pub mod params;
pub mod router;
pub mod service;

pub use method_router::{
    MethodRouter, any, delete, get, head, on, options, patch, post, put, trace_method,
};
pub use params::PathParams;
pub use router::{MatchedPathExt, Router};
