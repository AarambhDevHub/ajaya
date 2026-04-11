//! # ajaya-static
//!
//! Static file and directory serving for the Ajaya web framework.
//!
//! This crate will provide:
//! - `ServeDir` — serve a directory tree with MIME detection
//! - `ServeFile` — serve a single file
//! - ETag, Last-Modified, conditional GET (304)
//! - Range requests (206)
//! - Pre-compressed file support (.gz, .br)
//!
//! **Status:** Stub — implementation coming in v0.6.x
