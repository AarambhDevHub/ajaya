//! # ajaya-middleware
//!
//! Built-in middleware for the Ajaya web framework.
//!
//! This crate will provide Tower-compatible middleware:
//! - `CorsLayer` — CORS handling
//! - `CompressionLayer` — gzip/brotli/zstd compression
//! - `TimeoutLayer` — request timeout
//! - `RateLimitLayer` — rate limiting
//! - `TraceLayer` — structured logging
//! - `CatchPanicLayer` — panic recovery
//! - Security header middleware
//!
//! **Status:** Stub — implementation coming in v0.4.x
