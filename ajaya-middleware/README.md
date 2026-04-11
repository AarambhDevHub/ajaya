# ajaya-middleware

[![Crates.io](https://img.shields.io/crates/v/ajaya-middleware.svg)](https://crates.io/crates/ajaya-middleware)
[![Docs.rs](https://docs.rs/ajaya-middleware/badge.svg)](https://docs.rs/ajaya-middleware)
[![License](https://img.shields.io/badge/license-MIT%2FApache--2.0-blue.svg)](../LICENSE-MIT)

**Built-in middleware for the Ajaya web framework.**

This crate will provide Tower-compatible middleware layers for common web application needs.

## Planned Features (v0.4.x)

- `CorsLayer` — Full CORS spec implementation
- `CompressionLayer` — gzip, brotli, zstd, deflate
- `TimeoutLayer` — Request timeout enforcement
- `RateLimitLayer` — Token bucket / sliding window rate limiting
- `TraceLayer` — Structured request/response logging
- `RequestIdLayer` — UUID per request
- `SecurityHeadersLayer` — OWASP security headers
- `CatchPanicLayer` — Panic recovery
- `RequestBodyLimitLayer` — Body size limits

## Status

**v0.0.1** — Stub. Implementation coming in v0.4.x.

## License

Licensed under either of [Apache License, Version 2.0](../LICENSE-APACHE) or [MIT License](../LICENSE-MIT) at your option.
