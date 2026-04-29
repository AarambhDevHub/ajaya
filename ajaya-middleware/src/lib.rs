//! # ajaya-middleware
//!
//! Built-in Tower middleware layers for the Ajaya web framework.
//!
//! ## Function-based middleware (0.4.1)
//!
//! ```rust,ignore
//! use ajaya::middleware::{from_fn, from_fn_with_state, Next};
//! use ajaya::{Request, State};
//!
//! // Stateless — works with CookieJar, Path, Query, TypedHeader, Method, Uri, etc.
//! async fn log(req: Request, next: Next) -> impl IntoResponse {
//!     let path = req.uri().path().to_string();
//!     let res = next.run(req).await;
//!     tracing::info!("{} → {}", path, res.status());
//!     res
//! }
//! Router::new().layer(from_fn(log));
//! ```
//!
//! ## Available middleware
//!
//! | Layer | Description | Version |
//! |---|---|---|
//! | [`from_fn`] | Middleware from async fn with extractor support | ✅ 0.4.1 |
//! | [`from_fn_with_state`] | Same, with router state access | ✅ 0.4.1 |
//! | [`map_request`] | Transform request only | ✅ 0.4.1 |
//! | [`map_response`] | Transform response only | ✅ 0.4.1 |
//! | [`cors::CorsLayer`] | Full CORS spec | ✅ 0.4.1 |
//! | [`compression::CompressionLayer`] | gzip / br / zstd / deflate | ✅ 0.4.2 |
//! | [`compression::DecompressionLayer`] | Decompress request bodies | ✅ 0.4.2 |
//! | [`timeout::TimeoutLayer`] | 408 on slow handlers | ✅ 0.4.3 |
//! | [`request_id::RequestIdLayer`] | UUID per request | ✅ 0.4.4 |
//! | [`request_id::PropagateRequestIdLayer`] | Forward incoming x-request-id | ✅ 0.4.4 |
//! | [`trace::TraceLayer`] | Tracing span per request | ✅ 0.4.5 |
//! | [`security_headers::SecurityHeadersLayer`] | OWASP header suite | ✅ 0.4.6 |
//! | [`security_headers::SetResponseHeaderLayer`] | Set/override response header | ✅ 0.4.6 |
//! | [`security_headers::SetRequestHeaderLayer`] | Set/override request header | ✅ 0.4.6 |
//! | [`security_headers::SensitiveHeadersLayer`] | Redact headers in logs | ✅ 0.4.6 |
//! | [`rate_limit::RateLimitLayer`] | Token bucket rate limiting | ✅ 0.4.7 |
//! | [`auth::RequireAuthorizationLayer`] | Bearer / Basic / custom auth | ✅ 0.4.8 |
//! | [`body_limit::RequestBodyLimitLayer`] | 413 on oversized bodies | ✅ 0.4.9 |
//! | [`catch_panic::CatchPanicLayer`] | 500 on handler panics | ✅ 0.4.9 |
//! | [`map_body::MapRequestBodyLayer`] | Transform request body bytes | ✅ 0.4.10 |
//! | [`map_body::MapResponseBodyLayer`] | Transform response body bytes | ✅ 0.4.10 |
//! | [`csrf::CsrfLayer`] | CSRF double-submit cookie | ✅ 0.4.11 |

// ── 0.4.1 ────────────────────────────────────────────────────────────────────
pub mod auth;
pub mod body_limit;
pub mod catch_panic;
pub mod compression;
pub mod cors;
pub mod csrf;
pub mod from_fn;
pub mod map_body;
pub mod map_request;
pub mod map_response;
pub mod middleware_fn;
pub mod next;
pub mod rate_limit;
pub mod request_id;
pub mod security_headers;
pub mod timeout;
pub mod trace;

pub use cors::CorsLayer;
pub use from_fn::{FromFnLayer, FromFnService, from_fn, from_fn_with_state};
pub use map_request::{
    MapRequestLayer, MapRequestService, MapRequestWithStateLayer, MapRequestWithStateService,
    map_request, map_request_with_state,
};
pub use map_response::{
    MapResponseLayer, MapResponseService, MapResponseWithStateLayer, MapResponseWithStateService,
    map_response, map_response_with_state,
};
pub use middleware_fn::MiddlewareFn;
pub use next::Next;

pub use compression::{
    CompressionLayer, CompressionLevel, CompressionService, DecompressionLayer,
    DecompressionService,
};

pub use timeout::{TimeoutLayer, TimeoutService};

pub use request_id::{
    PropagateRequestIdLayer, PropagateRequestIdService, RequestId, RequestIdLayer, RequestIdService,
};

pub use trace::{DefaultMakeSpan, LatencyUnit, TraceLayer, TraceService};

pub use security_headers::{
    HeaderMode, SecurityHeadersLayer, SecurityHeadersService, SensitiveHeaders,
    SensitiveHeadersLayer, SensitiveHeadersService, SetRequestHeaderLayer, SetRequestHeaderService,
    SetResponseHeaderLayer, SetResponseHeaderService,
};

pub use rate_limit::{KeyExtractor, RateLimitLayer, RateLimitService};

pub use auth::{RequireAuthorizationLayer, RequireAuthorizationService};

pub use body_limit::{RequestBodyLimitLayer, RequestBodyLimitService};
pub use catch_panic::{CatchPanicLayer, CatchPanicService};

pub use map_body::{
    MapRequestBodyLayer, MapRequestBodyService, MapResponseBodyLayer, MapResponseBodyService,
};

pub use csrf::{CSRF_COOKIE_NAME, CSRF_HEADER_NAME, CsrfLayer, CsrfService, CsrfToken};
