//! # ajaya-middleware
//!
//! Built-in Tower middleware layers for the Ajaya web framework.
//!
//! ## Available middleware
//!
//! | Layer | Feature | Status |
//! |---|---|---|
//! | [`cors::CorsLayer`] | CORS — preflight + response headers | ✅ 0.4.1 |
//! | `CompressionLayer` | gzip / br / zstd | ⏳ 0.4.2 |
//! | `TimeoutLayer` | Request timeout | ⏳ 0.4.3 |
//! | `RequestIdLayer` | UUID per request | ⏳ 0.4.4 |
//! | `TraceLayer` | Structured logging | ⏳ 0.4.5 |
//! | `SecurityHeadersLayer` | OWASP security headers | ⏳ 0.4.6 |
//! | `RateLimitLayer` | Token bucket rate limiting | ⏳ 0.4.7 |
//! | `RequireAuthLayer` | Bearer / Basic auth | ⏳ 0.4.8 |
//! | `CatchPanicLayer` | Panic recovery → 500 | ⏳ 0.4.9 |
//! | `RequestBodyLimitLayer` | Body size → 413 | ⏳ 0.4.9 |
//! | `CsrfLayer` | CSRF token | ⏳ 0.4.11 |

pub mod cors;

pub use cors::CorsLayer;
