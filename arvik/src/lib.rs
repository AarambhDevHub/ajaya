//! # Arvik — Fast, Typed, and Fearless Web Framework for Rust
//!
//! Arvik is a high-performance web framework built on Tokio and Hyper,
//! engineered to be the fastest and most ergonomic Rust web framework.
//!
//! ## Quick Start
//!
//! ```rust,ignore
//! use arvik::{Router, get, serve_app};
//!
//! async fn home() -> &'static str { "Hello from Arvik!" }
//! async fn about() -> &'static str { "About Arvik" }
//!
//! #[tokio::main]
//! async fn main() {
//!     let app = Router::new()
//!         .route("/", get(home))
//!         .route("/about", get(about));
//!     serve_app("0.0.0.0:8080", app).await.unwrap();
//! }
//! ```
//!
//! ## Extractors
//!
//! ```rust,ignore
//! use arvik::{Router, get, post, Json, Path, Query, State};
//! use serde::{Deserialize, Serialize};
//!
//! #[derive(Clone)]
//! struct AppState { db_url: String }
//!
//! #[derive(Deserialize)]
//! struct CreateUser { name: String }
//!
//! #[derive(Serialize)]
//! struct User { id: u32, name: String }
//!
//! async fn get_user(Path(id): Path<u32>) -> Json<User> {
//!     Json(User { id, name: "Alice".into() })
//! }
//!
//! async fn create_user(
//!     State(state): State<AppState>,
//!     Json(body): Json<CreateUser>,
//! ) -> Json<User> {
//!     Json(User { id: 1, name: body.name })
//! }
//! ```
//!
//! ## Error Handling
//!
//! ```rust,ignore
//! use arvik::{Router, get, Json, Error};
//!
//! async fn handler() -> Result<Json<serde_json::Value>, Error> {
//!     let data = serde_json::json!({ "name": "Arvik" });
//!     Ok(Json(data))
//! }
//! ```

// ---------------------------------------------------------------------------
// Core types
// ---------------------------------------------------------------------------
pub use arvik_core::Body;
pub use arvik_core::Error;
pub use arvik_core::ErrorResponse;
pub use arvik_core::Handler;
pub use arvik_core::Html;
pub use arvik_core::IntoResponse;
pub use arvik_core::MethodFilter;
pub use arvik_core::Redirect;
pub use arvik_core::Request;
pub use arvik_core::RequestParts;
pub use arvik_core::Response;
pub use arvik_core::ResponseBuilder;
pub use arvik_core::StreamBody;
pub use arvik_core::Trailers;

// IntoResponseParts system
pub use arvik_core::AppendHeaders; // 0.3.2
pub use arvik_core::IntoResponseParts; // 0.3.2
pub use arvik_core::Preload;
pub use arvik_core::PreloadLink;
pub use arvik_core::ResponseParts; // 0.3.2

// Extractor traits
pub use arvik_core::FromRequest;
pub use arvik_core::FromRequestParts;

// ---------------------------------------------------------------------------
// Router types
// ---------------------------------------------------------------------------
pub use arvik_router::MethodRouter;
pub use arvik_router::PathParams;
pub use arvik_router::Router;
pub use arvik_router::{any, delete, get, head, on, options, patch, post, put, trace_method};

#[doc(hidden)]
pub mod __private {
    pub use arvik_router::{RouteMeta, assert_no_route_conflicts, route_conflict_count};
}

// ---------------------------------------------------------------------------
// Proc macros (0.7.x)
// ---------------------------------------------------------------------------

#[cfg(feature = "macros")]
pub use arvik_macros::{
    any, collect_routes, debug_handler, delete, get, handler, head, options, patch, post, put,
    route, trace,
};

// ---------------------------------------------------------------------------
// Extractors (from arvik-extract)
// ---------------------------------------------------------------------------

// Path & Query
pub use arvik_extract::Path;
pub use arvik_extract::Query;
pub use arvik_extract::RawPathParams;

// Headers
pub use arvik_extract::TypedHeader;
// HeaderMap is from http crate — users get it via `use http::HeaderMap`

// Request metadata
pub use arvik_extract::ConnectInfo;
pub use arvik_extract::Extension;
pub use arvik_extract::MatchedPath;
pub use arvik_extract::OriginalUri;

// Body extractors — Json from extract (has both FromRequest + IntoResponse)
pub use arvik_extract::Form;
pub use arvik_extract::Json;

// State
pub use arvik_extract::FromRef;
pub use arvik_extract::State;

// Cookies  ← 0.3.3
pub use arvik_extract::CookieJar;
pub use arvik_extract::PrivateCookieJar;
pub use arvik_extract::SignedCookieJar;
// Re-export cookie::Key so users don't need a direct cookie dep
pub use cookie::Cookie;
pub use cookie::Key as CookieKey;

// Multipart
pub use arvik_extract::Field;
pub use arvik_extract::FieldMetadata;
pub use arvik_extract::FieldStream;
pub use arvik_extract::Multipart;
pub use arvik_extract::MultipartConfig;
pub use arvik_extract::MultipartConstraints;
pub use arvik_extract::MultipartError;
pub use arvik_extract::ProgressChunk;
pub use arvik_extract::ProgressStream;
pub use arvik_extract::TempFile;

// Rejections (for custom error handling)
pub use arvik_extract::rejection;

// ---------------------------------------------------------------------------
// Server functionality
// ---------------------------------------------------------------------------
pub use arvik_hyper::Server;
pub use arvik_hyper::ServerConfig;
pub use arvik_hyper::{ConnectionInfo, ShutdownConfig, default_shutdown_signal};
pub use arvik_hyper::{
    serve, serve_app, serve_handler_with_config, serve_handler_with_config_and_graceful_shutdown,
    serve_handler_with_graceful_shutdown, serve_router, serve_router_with_config,
    serve_router_with_config_and_graceful_shutdown, serve_router_with_graceful_shutdown,
    serve_service, serve_service_with_config, serve_service_with_config_and_graceful_shutdown,
    serve_service_with_graceful_shutdown, serve_with_config,
    serve_with_config_and_graceful_shutdown, serve_with_graceful_shutdown,
};
#[cfg(feature = "http2")]
pub use arvik_hyper::{
    serve_h2c, serve_h2c_with_config, serve_h2c_with_config_and_graceful_shutdown,
    serve_h2c_with_graceful_shutdown,
};
#[cfg(feature = "native-tls")]
pub use arvik_hyper::{
    serve_native_tls, serve_native_tls_with_config,
    serve_native_tls_with_config_and_graceful_shutdown, serve_native_tls_with_graceful_shutdown,
};
#[cfg(feature = "tls")]
pub use arvik_hyper::{
    serve_tls, serve_tls_with_config, serve_tls_with_config_and_graceful_shutdown,
    serve_tls_with_graceful_shutdown,
};

// ---------------------------------------------------------------------------
// Configuration and test utilities (0.7.x)
// ---------------------------------------------------------------------------

#[cfg(feature = "config")]
pub mod config {
    pub use arvik_config::*;
}

#[cfg(feature = "config")]
pub use arvik_config::{
    ArvikConfig, ArvikConfigBuilder, ConfigError, Http1Section, Http2Section, ServerSection,
    ShutdownSection, TlsBackend, TlsSection,
};

// ---------------------------------------------------------------------------
// Observability (0.8.x)
// ---------------------------------------------------------------------------

#[cfg(feature = "metrics")]
pub mod metrics {
    pub use arvik_observe::metrics::*;
}

#[cfg(feature = "metrics")]
pub use arvik_observe::metrics::{MetricsRegistry, PrometheusMetricsLayer};

#[cfg(feature = "opentelemetry")]
pub mod trace {
    pub use arvik_observe::trace::*;
}

#[cfg(feature = "opentelemetry")]
pub use arvik_observe::trace::{OtelConfig, OtelGuard, OtelLayer, Propagation};

#[cfg(feature = "health")]
pub mod health {
    pub use arvik_observe::health::*;
}

#[cfg(feature = "health")]
pub use arvik_observe::health::{HealthCheckResult, HealthRegistry};

#[cfg(feature = "test-utils")]
pub use arvik_test::{TestClient, TestRequestBuilder, TestResponse};

#[cfg(all(feature = "test-utils", feature = "ws"))]
pub use arvik_test::{TestWebSocket, TestWebSocketError};

// ---------------------------------------------------------------------------
// TLS support (0.6.x)
// ---------------------------------------------------------------------------

#[cfg(any(feature = "tls", feature = "native-tls"))]
pub mod tls {
    #[cfg(feature = "native-tls")]
    pub use arvik_tls::native;
    #[cfg(feature = "tls")]
    pub use arvik_tls::rustls;
    pub use arvik_tls::{TlsError, TlsFilePaths};
}

#[cfg(feature = "native-tls")]
pub use arvik_tls::native::NativeTlsConfig;
#[cfg(feature = "tls")]
pub use arvik_tls::rustls::RustlsConfig;

// ---------------------------------------------------------------------------
// Static file support (0.6.x)
// ---------------------------------------------------------------------------

#[cfg(feature = "embedded-static")]
pub use arvik_static::{
    Embed, EmbeddedFile, EmbeddedFileService, Filenames, Metadata, RustEmbed, utils,
};
#[cfg(feature = "static-files")]
pub use arvik_static::{ServeDir, ServeFile};

// ---------------------------------------------------------------------------
// Middleware  (0.4.x)
// ---------------------------------------------------------------------------

/// Function-based middleware and request/response transformers.
///
/// This module exposes everything you need to write middleware as plain
/// async functions — no `Service` or `Layer` trait implementations needed.
///
/// Any [`FromRequestParts`] extractor can be used as a middleware parameter,
/// including [`CookieJar`], [`TypedHeader`], [`Path`], [`Query`],
/// [`Extension`], [`State`], [`SignedCookieJar`], and all HTTP metadata types.
///
/// # Quick reference
///
/// | Function | Use case |
/// |---|---|
/// | [`from_fn`] | Middleware with extractors, no shared state |
/// | [`from_fn_with_state`] | Middleware with extractors + shared app state |
/// | [`map_request`] | Transform the incoming request only |
/// | [`map_request_with_state`] | Transform request with state access |
/// | [`map_response`] | Transform the outgoing response only |
/// | [`map_response_with_state`] | Transform response with state access |
///
/// See [`arvik_middleware::from_fn`] for detailed documentation and examples.
pub mod middleware {
    pub use arvik_middleware::{
        auth::RequireAuthorizationLayer,
        body_limit::RequestBodyLimitLayer,
        catch_panic::CatchPanicLayer,
        compression::{CompressionLayer, CompressionLevel, DecompressionLayer},
        csrf::{CsrfLayer, CsrfToken},
        from_fn::{FromFnLayer, FromFnService, from_fn, from_fn_with_state},
        map_body::{MapRequestBodyLayer, MapResponseBodyLayer},
        map_request::{
            MapRequestLayer, MapRequestService, MapRequestWithStateLayer,
            MapRequestWithStateService, map_request, map_request_with_state,
        },
        map_response::{
            MapResponseLayer, MapResponseService, MapResponseWithStateLayer,
            MapResponseWithStateService, map_response, map_response_with_state,
        },
        middleware_fn::MiddlewareFn,
        next::Next,
        rate_limit::{KeyExtractor, RateLimitLayer},
        request_id::{PropagateRequestIdLayer, RequestId, RequestIdLayer},
        security_headers::{
            SecurityHeadersLayer, SensitiveHeadersLayer, SetRequestHeaderLayer,
            SetResponseHeaderLayer,
        },
        timeout::TimeoutLayer,
        trace::{DefaultMakeSpan, LatencyUnit, TraceLayer},
    };
}

// ── Top-level convenience re-exports ─────────────────────────────────────────
// Add at the top-level of lib.rs (outside the middleware module):
pub use arvik_middleware::CorsLayer;
pub use arvik_middleware::auth::RequireAuthorizationLayer;
pub use arvik_middleware::body_limit::RequestBodyLimitLayer;
pub use arvik_middleware::catch_panic::CatchPanicLayer;
pub use arvik_middleware::compression::{CompressionLayer, CompressionLevel, DecompressionLayer};
pub use arvik_middleware::csrf::{CsrfLayer, CsrfToken};
pub use arvik_middleware::map_body::{MapRequestBodyLayer, MapResponseBodyLayer};
pub use arvik_middleware::rate_limit::{KeyExtractor, RateLimitLayer};
pub use arvik_middleware::request_id::{PropagateRequestIdLayer, RequestId, RequestIdLayer};
pub use arvik_middleware::security_headers::{
    SecurityHeadersLayer, SensitiveHeadersLayer, SetRequestHeaderLayer, SetResponseHeaderLayer,
};
pub use arvik_middleware::timeout::TimeoutLayer;
pub use arvik_middleware::trace::{DefaultMakeSpan, LatencyUnit, TraceLayer};

// Tower layer / service primitives (for custom middleware authors)
pub use arvik_router::layer::{BoxCloneService, LayerFn};

// ---------------------------------------------------------------------------
// WebSocket support (0.5.x) — feature = "ws"
// ---------------------------------------------------------------------------

/// WebSocket upgrade and messaging.
///
/// Enabled by the `ws` feature (on by default). Disable with:
/// ```toml
/// arvik = { version = "0.6", default-features = false }
/// ```
///
/// # Quick start
///
/// ```rust,ignore
/// use arvik::ws::{WebSocket, WebSocketUpgrade, Message};
///
/// async fn handler(ws: WebSocketUpgrade) -> impl IntoResponse {
///     ws.on_upgrade(|mut socket| async move {
///         // Ping/Pong handled automatically — no extra match arm needed
///         while let Some(Ok(msg)) = socket.recv().await {
///             socket.send(msg).await.ok();
///         }
///     })
/// }
/// ```
#[cfg(feature = "ws")]
pub mod ws {
    pub use arvik_ws::{
        CloseCode, CloseFrame, Message, Receiver, Sender, WebSocket, WebSocketConfig,
        WebSocketUpgrade, WebSocketUpgradeRejection, WsError,
    };
}

// Top-level convenience — most common types (feature = "ws")
#[cfg(feature = "ws")]
pub use arvik_ws::{Message as WsMessage, WebSocket, WebSocketUpgrade};

// ---------------------------------------------------------------------------
// Server-Sent Events (0.5.1) — feature = "sse"
// ---------------------------------------------------------------------------

/// Server-Sent Events streaming.
///
/// Enable via the `sse` feature flag:
///
/// ```toml
/// arvik = { version = "0.6", features = ["sse"] }
/// ```
///
/// # Quick start
///
/// ```rust,ignore
/// use arvik::sse::{Event, KeepAlive, Sse};
/// use std::{convert::Infallible, time::Duration};
/// use tokio_stream::StreamExt as _;
///
/// async fn clock() -> Sse<impl Stream<Item = Result<Event, Infallible>>> {
///     let stream = tokio_stream::wrappers::IntervalStream::new(
///         tokio::time::interval(Duration::from_secs(1)),
///     )
///     .enumerate()
///     .map(|(i, _)| Ok(Event::default().id(i.to_string()).data(i.to_string())));
///
///     Sse::new(stream).keep_alive(KeepAlive::new())
/// }
///
/// let app = Router::new().route("/events", get(clock));
/// ```
#[cfg(feature = "sse")]
pub mod sse {
    pub use arvik_sse::{Event, KeepAlive, Sse};
}

// Top-level convenience re-exports (feature = "sse")
#[cfg(feature = "sse")]
pub use arvik_sse::{Event as SseEvent, KeepAlive as SseKeepAlive, Sse};
