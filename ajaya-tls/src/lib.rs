//! # ajaya-tls
//!
//! TLS / HTTPS support for the Ajaya web framework.
//!
//! This crate will provide:
//! - `RustlsConfig` — rustls-based TLS (no OpenSSL dependency)
//! - `NativeTlsConfig` — native-tls backend (OpenSSL/SChannel/SecureTransport)
//! - Self-signed certificate generation for development
//! - TLS certificate hot-reload without server restart
//!
//! **Status:** Stub — implementation coming in v0.6.x
