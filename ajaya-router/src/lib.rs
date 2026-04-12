//! # ajaya-router
//!
//! Routing for the Ajaya web framework.
//!
//! This crate provides:
//! - [`MethodRouter`] — HTTP method-based dispatch for a single route
//! - Top-level constructor functions: [`get`], [`post`], [`put`], [`delete`], etc.
//!
//! ## Quick Start
//!
//! ```rust,ignore
//! use ajaya_router::{get, post};
//!
//! async fn hello() -> &'static str { "Hello!" }
//! async fn create() -> &'static str { "Created!" }
//!
//! let router = get(hello).post(create);
//! ```
//!
//! ## Roadmap
//!
//! - **v0.1.0** — Full `Router` with path-based routing
//! - **v0.1.1** — Radix trie router with zero-alloc lookup
//! - **v0.1.2** — Path parameters (`:id`)
//! - **v0.1.3** — Wildcard routes (`*path`)

pub mod method_router;

pub use method_router::{
    MethodRouter, any, delete, get, head, on, options, patch, post, put, trace_method,
};
