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
use http::{HeaderMap, HeaderValue, StatusCode, header};
use http_body_util::BodyExt as _;

use crate::body::{Body, BoxError};
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
/// use arvik_core::{IntoResponse, Response};
///
/// struct XmlBody(String);
///
/// impl IntoResponse for XmlBody {
///     fn into_response(self) -> Response {
///         ResponseBuilder::new()
///             .header(http::header::CONTENT_TYPE, "application/xml")
///             .body(arvik_core::Body::from(self.0))
///     }
/// }
/// ```
pub trait IntoResponse {
    /// Convert this value into an HTTP [`Response`].
    fn into_response(self) -> Response;
}

/// Serialize a value into an Arvik JSON response body.
#[doc(hidden)]
pub fn serialize_json_body<T: serde::Serialize>(value: &T) -> Result<Body, serde_json::Error> {
    serde_json::to_vec(value).map(Body::from)
}

/// Build an `application/json` response from an already serialized body.
#[doc(hidden)]
#[inline]
pub fn json_body_response(body: Body) -> Response {
    let exact_len = body.exact_size_hint();
    let mut response = Response::new(body);
    let headers = response.headers_mut();
    headers.insert(
        header::CONTENT_TYPE,
        HeaderValue::from_static("application/json"),
    );
    insert_exact_content_length(headers, exact_len);
    response
}

#[inline]
fn insert_exact_content_length(headers: &mut HeaderMap, exact_len: Option<u64>) {
    if headers.contains_key(header::CONTENT_LENGTH) {
        return;
    }

    if let Some(len) = exact_len {
        let mut buf = itoa::Buffer::new();
        headers.insert(
            header::CONTENT_LENGTH,
            HeaderValue::from_str(buf.format(len)).expect("valid Content-Length"),
        );
    }
}

// ---------------------------------------------------------------------------
// Trailers
// ---------------------------------------------------------------------------

/// Attach HTTP trailers to a response body.
///
/// This stays intentionally small: trailers are appended after the wrapped
/// response body and are only useful for protocols that support trailers,
/// primarily HTTP/2. Intermediaries and HTTP/1 clients may ignore them.
#[derive(Debug, Clone)]
pub struct Trailers<R> {
    response: R,
    trailers: HeaderMap,
}

impl<R> Trailers<R> {
    /// Create a response wrapper with trailers.
    pub fn new(response: R, trailers: HeaderMap) -> Self {
        Self { response, trailers }
    }

    /// Return the wrapped response value.
    pub fn response(&self) -> &R {
        &self.response
    }

    /// Return the trailers.
    pub fn trailers(&self) -> &HeaderMap {
        &self.trailers
    }
}

impl<R: IntoResponse> IntoResponse for Trailers<R> {
    fn into_response(self) -> Response {
        let response = self.response.into_response();
        let (parts, body) = response.into_parts();
        let trailers = self.trailers;
        let body = body.with_trailers(std::future::ready(Some(Ok::<HeaderMap, BoxError>(
            trailers,
        ))));
        Response::from_parts(parts, Body::new(body))
    }
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
        let mut response = Response::new(Body::empty());
        *response.status_mut() = self;
        response
    }
}

// ---------------------------------------------------------------------------
// String types → text/plain
// ---------------------------------------------------------------------------

impl IntoResponse for String {
    fn into_response(self) -> Response {
        text_response(Body::from(self))
    }
}

impl IntoResponse for &'static str {
    fn into_response(self) -> Response {
        text_response(Body::from_static(self.as_bytes()))
    }
}

// ---------------------------------------------------------------------------
// Raw bytes → application/octet-stream
// ---------------------------------------------------------------------------

impl IntoResponse for Bytes {
    fn into_response(self) -> Response {
        let body = Body::from_bytes(self);
        let exact_len = body.exact_size_hint();
        let mut response = Response::new(body);
        let headers = response.headers_mut();
        headers.insert(
            header::CONTENT_TYPE,
            HeaderValue::from_static("application/octet-stream"),
        );
        insert_exact_content_length(headers, exact_len);
        response
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

#[cfg(test)]
#[allow(clippy::items_after_test_module)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn trailers_are_appended_after_body() {
        let mut trailers = HeaderMap::new();
        trailers.insert("x-checksum", "abc123".parse().unwrap());

        let response = Trailers::new("body", trailers).into_response();
        let collected = response.into_body().collect().await.unwrap();
        let collected_trailers = collected.trailers().cloned().unwrap();

        assert_eq!(collected_trailers.get("x-checksum").unwrap(), "abc123");
        assert_eq!(collected.to_bytes(), Bytes::from_static(b"body"));
    }

    #[tokio::test]
    async fn json_response_serializes_to_bytes() {
        let response = Json(serde_json::json!({ "name": "Arvik" })).into_response();
        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(
            response.headers().get(http::header::CONTENT_TYPE).unwrap(),
            "application/json"
        );
        assert_eq!(
            response
                .headers()
                .get(http::header::CONTENT_LENGTH)
                .unwrap(),
            "16"
        );
        assert_eq!(
            response.into_body().to_bytes().await.unwrap(),
            Bytes::from_static(br#"{"name":"Arvik"}"#)
        );
    }

    #[tokio::test]
    async fn json_response_handles_serialization_failure() {
        struct Fails;

        impl serde::Serialize for Fails {
            fn serialize<S>(&self, _serializer: S) -> Result<S::Ok, S::Error>
            where
                S: serde::Serializer,
            {
                Err(serde::ser::Error::custom("nope"))
            }
        }

        let response = Json(Fails).into_response();
        assert_eq!(response.status(), StatusCode::INTERNAL_SERVER_ERROR);
        assert_eq!(
            response
                .headers()
                .get(http::header::CONTENT_LENGTH)
                .unwrap(),
            "43"
        );
        assert_eq!(
            response.into_body().to_bytes().await.unwrap(),
            Bytes::from_static(br#"{"error":"Serialization failed","code":500}"#)
        );
    }

    #[tokio::test]
    async fn static_str_response_sets_text_headers_and_body() {
        let response = "Hello".into_response();
        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(
            response.headers().get(http::header::CONTENT_TYPE).unwrap(),
            "text/plain; charset=utf-8"
        );
        assert_eq!(
            response
                .headers()
                .get(http::header::CONTENT_LENGTH)
                .unwrap(),
            "5"
        );
        let hint = http_body::Body::size_hint(response.body());
        assert_eq!(hint.lower(), 5);
        assert_eq!(hint.upper(), Some(5));
        assert_eq!(
            response.into_body().to_bytes().await.unwrap(),
            Bytes::from_static(b"Hello")
        );
    }

    #[tokio::test]
    async fn bytes_response_sets_octet_stream() {
        let response = Bytes::from_static(b"abc").into_response();
        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(
            response.headers().get(http::header::CONTENT_TYPE).unwrap(),
            "application/octet-stream"
        );
        assert_eq!(
            response
                .headers()
                .get(http::header::CONTENT_LENGTH)
                .unwrap(),
            "3"
        );
        assert_eq!(
            response.into_body().to_bytes().await.unwrap(),
            Bytes::from_static(b"abc")
        );
    }

    #[tokio::test]
    async fn status_code_response_is_empty() {
        let response = StatusCode::NO_CONTENT.into_response();
        assert_eq!(response.status(), StatusCode::NO_CONTENT);
        assert!(response.into_body().to_bytes().await.unwrap().is_empty());
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
/// Also usable as a request body extractor — see `arvik-extract`.
///
/// # Examples
///
/// ```rust,ignore
/// use arvik::Json;
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
        match serialize_json_body(&self.0) {
            Ok(body) => json_body_response(body),
            Err(err) => {
                tracing::error!("JSON serialization failed: {err}");
                let mut response = json_body_response(Body::from_static(
                    br#"{"error":"Serialization failed","code":500}"#,
                ));
                *response.status_mut() = StatusCode::INTERNAL_SERVER_ERROR;
                response
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
/// use arvik::Html;
/// async fn handler() -> Html<String> {
///     Html("<h1>Hello from Arvik!</h1>".to_string())
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

#[inline]
fn text_response(body: Body) -> Response {
    let exact_len = body.exact_size_hint();
    let mut response = Response::new(body);
    let headers = response.headers_mut();
    headers.insert(
        header::CONTENT_TYPE,
        HeaderValue::from_static("text/plain; charset=utf-8"),
    );
    insert_exact_content_length(headers, exact_len);
    response
}
