//! The [`IntoResponse`] trait and implementations.
//!
//! Types implementing `IntoResponse` can be returned from handlers
//! and will be automatically converted into HTTP responses.
//!
//! ## Implemented Types
//!
//! ### Basic types
//! - `Response` — identity
//! - `StatusCode` — empty body with that status
//! - `String` / `&'static str` — `text/plain`
//! - `Bytes` / `Vec<u8>` — `application/octet-stream`
//! - `()` — 200 OK, empty body
//! - `Result<T, E>` — delegates to the `Ok` or `Err` variant
//! - `Infallible` — unreachable
//!
//! ### Rich types
//! - [`Json<T>`] — `application/json`
//! - [`Html<T>`] — `text/html`
//!
//! ### Tuple types
//! - `(StatusCode, T)`
//! - `([(K,V); N], T)` — headers from const array
//! - `(StatusCode, [(K,V); N], T)`
//! - `(impl IntoResponseParts, T)` — any single header set + body
//! - `(P1, P2, T)` — two header sets + body (both must be IntoResponseParts)
//!
//! ### Setting HeaderMap headers (0.3.2+)
//!
//! `http::HeaderMap` implements `IntoResponseParts`, so you can write:
//!
//! ```rust,ignore
//! use http::HeaderMap;
//! // (HeaderMap, body) works via the IntoResponseParts blanket impl:
//! async fn handler() -> impl IntoResponse {
//!     let mut headers = HeaderMap::new();
//!     headers.insert(http::header::CACHE_CONTROL, "no-store".parse().unwrap());
//!     (headers, Json(data))
//! }
//!
//! // (StatusCode, HeaderMap, body) — use AppendHeaders for the three-tuple:
//! async fn handler2() -> impl IntoResponse {
//!     (StatusCode::CREATED, AppendHeaders([(LOCATION, "/users/1")]), Json(user))
//! }
//! ```

use bytes::Bytes;
use http::StatusCode;

use crate::body::Body;
// IntoResponseParts is imported only for the blanket impls below.
use crate::into_response_parts::{IntoResponseParts, apply_parts};
use crate::response::{Response, ResponseBuilder};

// ---------------------------------------------------------------------------
// Core trait
// ---------------------------------------------------------------------------

/// Trait for types that can be converted into an HTTP [`Response`].
///
/// Implement this for your own types to return them from handlers.
///
/// # Example
///
/// ```rust,ignore
/// use ajaya_core::{IntoResponse, Response};
///
/// struct XmlBody(String);
///
/// impl IntoResponse for XmlBody {
///     fn into_response(self) -> Response {
///         ResponseBuilder::new()
///             .header(http::header::CONTENT_TYPE, "application/xml")
///             .body(ajaya_core::Body::from(self.0))
///     }
/// }
/// ```
pub trait IntoResponse {
    /// Convert this value into an HTTP [`Response`].
    fn into_response(self) -> Response;
}

// ---------------------------------------------------------------------------
// Identity
// ---------------------------------------------------------------------------

impl IntoResponse for Response {
    #[inline]
    fn into_response(self) -> Response {
        self
    }
}

// ---------------------------------------------------------------------------
// StatusCode → empty body with that status
// ---------------------------------------------------------------------------

impl IntoResponse for StatusCode {
    #[inline]
    fn into_response(self) -> Response {
        ResponseBuilder::new().status(self).empty()
    }
}

// ---------------------------------------------------------------------------
// String types → text/plain
// ---------------------------------------------------------------------------

impl IntoResponse for String {
    fn into_response(self) -> Response {
        ResponseBuilder::new()
            .header(http::header::CONTENT_TYPE, "text/plain; charset=utf-8")
            .body(Body::from(self))
    }
}

impl IntoResponse for &'static str {
    fn into_response(self) -> Response {
        ResponseBuilder::new()
            .header(http::header::CONTENT_TYPE, "text/plain; charset=utf-8")
            .body(Body::from(self))
    }
}

// ---------------------------------------------------------------------------
// Raw bytes → application/octet-stream
// ---------------------------------------------------------------------------

impl IntoResponse for Bytes {
    fn into_response(self) -> Response {
        ResponseBuilder::new()
            .header(http::header::CONTENT_TYPE, "application/octet-stream")
            .body(Body::from(self))
    }
}

impl IntoResponse for Vec<u8> {
    fn into_response(self) -> Response {
        Bytes::from(self).into_response()
    }
}

// ---------------------------------------------------------------------------
// Unit → 200 OK empty body
// ---------------------------------------------------------------------------

impl IntoResponse for () {
    #[inline]
    fn into_response(self) -> Response {
        StatusCode::OK.into_response()
    }
}

// ---------------------------------------------------------------------------
// Infallible → unreachable
// ---------------------------------------------------------------------------

impl IntoResponse for std::convert::Infallible {
    fn into_response(self) -> Response {
        match self {}
    }
}

// ---------------------------------------------------------------------------
// Result<T, E>
// ---------------------------------------------------------------------------

impl<T: IntoResponse, E: IntoResponse> IntoResponse for Result<T, E> {
    #[inline]
    fn into_response(self) -> Response {
        match self {
            Ok(v) => v.into_response(),
            Err(e) => e.into_response(),
        }
    }
}

// ---------------------------------------------------------------------------
// (StatusCode, T) — override status
//
// NOTE: StatusCode does NOT implement IntoResponseParts, so this impl is
// disjoint from the blanket (P: IntoResponseParts, R) below.  No conflict.
// ---------------------------------------------------------------------------

impl<T: IntoResponse> IntoResponse for (StatusCode, T) {
    fn into_response(self) -> Response {
        let (status, body) = self;
        let mut r = body.into_response();
        *r.status_mut() = status;
        r
    }
}

// ---------------------------------------------------------------------------
// ([(K,V); N], T) — headers from a const array
//
// NOTE: arrays do NOT implement IntoResponseParts, so this impl is
// disjoint from the blanket (P: IntoResponseParts, R) below.  No conflict.
// ---------------------------------------------------------------------------

impl<K, V, T, const N: usize> IntoResponse for ([(K, V); N], T)
where
    K: TryInto<http::header::HeaderName>,
    K::Error: std::fmt::Debug,
    V: TryInto<http::header::HeaderValue>,
    V::Error: std::fmt::Debug,
    T: IntoResponse,
{
    fn into_response(self) -> Response {
        let (headers, body) = self;
        let mut r = body.into_response();
        for (key, value) in headers {
            if let (Ok(name), Ok(val)) = (key.try_into(), value.try_into()) {
                r.headers_mut().insert(name, val);
            }
        }
        r
    }
}

// ---------------------------------------------------------------------------
// (StatusCode, [(K,V); N], T) — status + headers from const array
// ---------------------------------------------------------------------------

impl<K, V, T, const N: usize> IntoResponse for (StatusCode, [(K, V); N], T)
where
    K: TryInto<http::header::HeaderName>,
    K::Error: std::fmt::Debug,
    V: TryInto<http::header::HeaderValue>,
    V::Error: std::fmt::Debug,
    T: IntoResponse,
{
    fn into_response(self) -> Response {
        let (status, headers, body) = self;
        let mut r = body.into_response();
        *r.status_mut() = status;
        for (key, value) in headers {
            if let (Ok(name), Ok(val)) = (key.try_into(), value.try_into()) {
                r.headers_mut().insert(name, val);
            }
        }
        r
    }
}

// ---------------------------------------------------------------------------
// (P: IntoResponseParts, R: IntoResponse) — generic header set + body
//
// This is the PRIMARY extensibility point (0.3.2).  Any type that implements
// IntoResponseParts — including http::HeaderMap, CookieJar, AppendHeaders,
// and user-defined types — can be prepended to any response body.
//
// Examples that use this blanket:
//   (HeaderMap,    Json(data))
//   (CookieJar,    "ok")
//   (AppendHeaders([...]), Html(html))
//
// Why this does NOT conflict with (StatusCode, T):
//   StatusCode does not implement IntoResponseParts.
//
// Why this does NOT conflict with ([(K,V);N], T):
//   Fixed-size arrays do not implement IntoResponseParts.
// ---------------------------------------------------------------------------

impl<P, R> IntoResponse for (P, R)
where
    P: IntoResponseParts,
    R: IntoResponse,
{
    fn into_response(self) -> Response {
        let (parts, body) = self;
        apply_parts(parts, body.into_response())
    }
}

// ---------------------------------------------------------------------------
// (P1, P2, R) — two IntoResponseParts sets + body  (0.3.2)
//
// Handles patterns like:
//   (security_headers, cookie_jar, Json(data))
//   (AppendHeaders([...]), CookieJar, "ok")
//
// (StatusCode, AppendHeaders([...]), body) ALSO hits this impl because
// StatusCode does not implement IntoResponseParts, so it falls through to
// the compiler looking for a concrete three-tuple impl.  We provide the
// dedicated (StatusCode, P, R) impl below exactly for that case.
// ---------------------------------------------------------------------------

impl<P1, P2, R> IntoResponse for (P1, P2, R)
where
    P1: IntoResponseParts,
    P2: IntoResponseParts,
    R: IntoResponse,
{
    fn into_response(self) -> Response {
        let (p1, p2, body) = self;
        let r = body.into_response();
        let r = apply_parts(p1, r);
        apply_parts(p2, r)
    }
}

// ---------------------------------------------------------------------------
// (StatusCode, P: IntoResponseParts, R) — status + header set + body (0.3.2)
//
// Enables the three-tuple pattern when the first element is a status code:
//
//   (StatusCode::CREATED, AppendHeaders([(LOCATION, "/users/1")]), Json(user))
//   (StatusCode::OK,      CookieJar,                               "ok")
//
// This is disjoint from (P1, P2, R) because StatusCode does not implement
// IntoResponseParts.  Rust's coherence checker accepts both.
// ---------------------------------------------------------------------------

impl<P, R> IntoResponse for (StatusCode, P, R)
where
    P: IntoResponseParts,
    R: IntoResponse,
{
    fn into_response(self) -> Response {
        let (status, parts, body) = self;
        let mut r = body.into_response();
        *r.status_mut() = status;
        apply_parts(parts, r)
    }
}

// ---------------------------------------------------------------------------
// Json<T> — application/json response
// ---------------------------------------------------------------------------

/// JSON response type.
///
/// Serializes `T` as JSON with `Content-Type: application/json`.
/// Also usable as a request body extractor — see `ajaya-extract`.
///
/// # Examples
///
/// ```rust,ignore
/// use ajaya::Json;
///
/// async fn handler() -> Json<serde_json::Value> {
///     Json(serde_json::json!({ "status": "ok" }))
/// }
///
/// async fn fallible() -> Result<Json<MyType>, Error> {
///     Ok(Json(load().await?))
/// }
/// ```
pub struct Json<T>(pub T);

impl<T: serde::Serialize> IntoResponse for Json<T> {
    fn into_response(self) -> Response {
        match serde_json::to_vec(&self.0) {
            Ok(bytes) => ResponseBuilder::new()
                .header(http::header::CONTENT_TYPE, "application/json")
                .body(Body::from_bytes(Bytes::from(bytes))),
            Err(err) => {
                tracing::error!("JSON serialization failed: {err}");
                ResponseBuilder::new()
                    .status(StatusCode::INTERNAL_SERVER_ERROR)
                    .header(http::header::CONTENT_TYPE, "application/json")
                    .body(Body::from(r#"{"error":"Serialization failed","code":500}"#))
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Html<T> — text/html response
// ---------------------------------------------------------------------------

/// HTML response type.
///
/// Sets `Content-Type: text/html; charset=utf-8`.
///
/// # Example
///
/// ```rust,ignore
/// use ajaya::Html;
/// async fn handler() -> Html<String> {
///     Html("<h1>Hello from Ajaya!</h1>".to_string())
/// }
/// ```
pub struct Html<T>(pub T);

impl<T: Into<String>> IntoResponse for Html<T> {
    fn into_response(self) -> Response {
        ResponseBuilder::new()
            .header(http::header::CONTENT_TYPE, "text/html; charset=utf-8")
            .body(Body::from(self.0.into()))
    }
}
