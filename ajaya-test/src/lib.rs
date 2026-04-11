//! # ajaya-test
//!
//! In-process testing utilities for the Ajaya web framework.
//!
//! This crate will provide:
//! - `TestClient` — in-memory HTTP client (no network, no port)
//! - Request builder: `.get()`, `.post()`, `.put()`, `.delete()`
//! - Response assertions: `.status()`, `.text()`, `.json::<T>()`
//! - WebSocket test client
//! - Cookie jar for stateful test sessions
//!
//! **Status:** Stub — implementation coming in v0.7.x
