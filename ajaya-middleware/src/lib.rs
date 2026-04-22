//! # ajaya-middleware
//!
//! Built-in Tower middleware layers for the Ajaya web framework.
//!
//! ## Function-based middleware
//!
//! The easiest way to write middleware: plain async functions that can use
//! any [`FromRequestParts`] extractor as a parameter.
//!
//! ```rust,ignore
//! use ajaya::middleware::{from_fn, from_fn_with_state, Next};
//! use ajaya::{Request, State, CookieJar};
//!
//! // Stateless — works with CookieJar, Path, Query, TypedHeader, Method, Uri, etc.
//! async fn log(req: Request, next: Next) -> impl IntoResponse {
//!     let path = req.uri().path().to_string();
//!     let res = next.run(req).await;
//!     tracing::info!("{} → {}", path, res.status());
//!     res
//! }
//!
//! // Stateful — works with State<S>, SignedCookieJar, PrivateCookieJar, etc.
//! async fn auth(State(s): State<AppState>, req: Request, next: Next) -> impl IntoResponse {
//!     // ...
//!     next.run(req).await
//! }
//!
//! Router::new()
//!     .route("/", get(handler))
//!     .layer(from_fn(log))
//!     .layer(from_fn_with_state(app_state, auth));
//! ```
//!
//! ## Available middleware
//!
//! | Item | Description | Status |
//! |---|---|---|
//! | [`from_fn`] | Middleware from an async function (with extractor support) | ✅ |
//! | [`from_fn_with_state`] | Same, with access to router state via extractors | ✅ |
//! | [`map_request`] | Transform the request only | ✅ |
//! | [`map_request_with_state`] | Transform the request with state access | ✅ |
//! | [`map_response`] | Transform the response only | ✅ |
//! | [`map_response_with_state`] | Transform the response with state access | ✅ |
//! | [`cors::CorsLayer`] | Full CORS spec implementation | ✅ 0.4.1 |
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
pub mod from_fn;
pub mod map_request;
pub mod map_response;
pub mod middleware_fn;
pub mod next;

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
