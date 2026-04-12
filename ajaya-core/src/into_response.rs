//! The [`IntoResponse`] trait and implementations.
//!
//! Types implementing `IntoResponse` can be returned from handlers
//! and will be automatically converted into HTTP responses.
//!
//! # Implemented Types
//!
//! - `Response` — identity (pass through)
//! - `StatusCode` — empty body with status
//! - `String` — text/plain body
//! - `&'static str` — text/plain body
//! - `Bytes` — application/octet-stream body
//! - `Vec<u8>` — application/octet-stream body
//! - `(StatusCode, T)` — response with custom status
//! - `(StatusCode, [(K, V); N], T)` — response with status + headers
//! - `()` — 200 OK with empty body
//! - `Result<T, E>` — delegates to `Ok` or `Err` variant
//! - [`Json<T>`] — JSON response with `Content-Type: application/json`
//! - [`Html<T>`] — HTML response with `Content-Type: text/html`

use bytes::Bytes;
use http::StatusCode;

use crate::body::Body;
use crate::response::{Response, ResponseBuilder};

/// Trait for types that can be converted into an HTTP [`Response`].
///
/// Implement this trait for your own types to return them directly
/// from handlers. The framework provides blanket implementations
/// for many common types.
///
/// # Example
///
/// ```rust,ignore
/// use ajaya_core::IntoResponse;
/// use ajaya_core::Response;
///
/// struct MyResponse {
///     message: String,
/// }
///
/// impl IntoResponse for MyResponse {
///     fn into_response(self) -> Response {
///         self.message.into_response()
///     }
/// }
/// ```
pub trait IntoResponse {
    /// Convert this type into an HTTP [`Response`].
    fn into_response(self) -> Response;
}

// --- Identity ---

impl IntoResponse for Response {
    #[inline]
    fn into_response(self) -> Response {
        self
    }
}

// --- Status code alone → empty body with that status ---

impl IntoResponse for StatusCode {
    fn into_response(self) -> Response {
        ResponseBuilder::new().status(self).empty()
    }
}

// --- String types → text/plain ---

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

// --- Raw bytes → application/octet-stream ---

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

// --- Unit type → 200 OK empty body ---

impl IntoResponse for () {
    fn into_response(self) -> Response {
        StatusCode::OK.into_response()
    }
}

// --- (StatusCode, impl IntoResponse) → custom status ---

impl<T: IntoResponse> IntoResponse for (StatusCode, T) {
    fn into_response(self) -> Response {
        let (status, body) = self;
        let mut response = body.into_response();
        *response.status_mut() = status;
        response
    }
}

// --- (StatusCode, [(K, V); N], impl IntoResponse) → custom status + headers ---

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
        let mut response = body.into_response();
        *response.status_mut() = status;
        for (key, value) in headers {
            if let (Ok(name), Ok(val)) = (key.try_into(), value.try_into()) {
                response.headers_mut().insert(name, val);
            }
        }
        response
    }
}

// --- ([(K, V); N], impl IntoResponse) → headers only (keep 200 status) ---

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
        let mut response = body.into_response();
        for (key, value) in headers {
            if let (Ok(name), Ok(val)) = (key.try_into(), value.try_into()) {
                response.headers_mut().insert(name, val);
            }
        }
        response
    }
}

// --- Result<T, E> → delegates to Ok or Err variant ---

impl<T: IntoResponse, E: IntoResponse> IntoResponse for Result<T, E> {
    fn into_response(self) -> Response {
        match self {
            Ok(v) => v.into_response(),
            Err(e) => e.into_response(),
        }
    }
}

// --- Json<T> → JSON response ---

/// A JSON response.
///
/// Serializes the inner value as JSON and sets
/// `Content-Type: application/json`.
///
/// # Examples
///
/// ```rust,ignore
/// use ajaya::Json;
///
/// async fn handler() -> Json<serde_json::Value> {
///     Json(serde_json::json!({ "message": "Hello, Ajaya!" }))
/// }
///
/// // With Result for error handling:
/// async fn fallible() -> Result<Json<User>, Error> {
///     let user = load_user().await?;
///     Ok(Json(user))
/// }
/// ```
pub struct Json<T>(pub T);

impl<T: serde::Serialize> IntoResponse for Json<T> {
    fn into_response(self) -> Response {
        match serde_json::to_vec(&self.0) {
            Ok(json_bytes) => ResponseBuilder::new()
                .header(http::header::CONTENT_TYPE, "application/json")
                .body(Body::from_bytes(Bytes::from(json_bytes))),
            Err(err) => {
                // If serialization fails, return a 500 error
                let body = format!("{{\"error\":\"JSON serialization failed: {}\"}}", err);
                ResponseBuilder::new()
                    .status(StatusCode::INTERNAL_SERVER_ERROR)
                    .header(http::header::CONTENT_TYPE, "application/json")
                    .body(Body::from(body))
            }
        }
    }
}

// --- Html<T> → HTML response ---

/// An HTML response.
///
/// Sets `Content-Type: text/html; charset=utf-8`.
///
/// # Examples
///
/// ```rust,ignore
/// use ajaya::Html;
///
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
