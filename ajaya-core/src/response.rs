//! HTTP Response types.
//!
//! Provides the [`Response`] type alias and a [`ResponseBuilder`]
//! for ergonomic response construction.
//!
//! # Examples
//!
//! ```rust,ignore
//! use ajaya_core::response::ResponseBuilder;
//!
//! let response = ResponseBuilder::new()
//!     .status(200)
//!     .header("x-custom", "value")
//!     .text("Hello, Ajaya!");
//! ```

use bytes::Bytes;
use http::StatusCode;

use crate::Body;

/// Ajaya's HTTP response type.
///
/// A type alias for [`http::Response`] using Ajaya's [`Body`] type.
pub type Response<B = Body> = http::Response<B>;

/// Ergonomic builder for constructing HTTP responses.
///
/// Provides a fluent API for setting status codes, headers,
/// and creating typed response bodies (JSON, HTML, plain text).
///
/// # Examples
///
/// ```rust,ignore
/// // Plain text response
/// let res = ResponseBuilder::new().text("Hello!");
///
/// // JSON response
/// let res = ResponseBuilder::new()
///     .status(StatusCode::CREATED)
///     .json(&serde_json::json!({ "id": 1 }));
///
/// // HTML response
/// let res = ResponseBuilder::new().html("<h1>Hello</h1>");
/// ```
pub struct ResponseBuilder {
    inner: http::response::Builder,
}

impl ResponseBuilder {
    /// Create a new `ResponseBuilder` with default 200 OK status.
    pub fn new() -> Self {
        Self {
            inner: http::Response::builder().status(StatusCode::OK),
        }
    }

    /// Set the HTTP status code.
    pub fn status<T: Into<StatusCode>>(mut self, status: T) -> Self {
        self.inner = self.inner.status(status.into());
        self
    }

    /// Add a header to the response.
    pub fn header<K, V>(mut self, key: K, value: V) -> Self
    where
        K: TryInto<http::header::HeaderName>,
        K::Error: Into<http::Error>,
        V: TryInto<http::header::HeaderValue>,
        V::Error: Into<http::Error>,
    {
        self.inner = self.inner.header(key, value);
        self
    }

    /// Build the response with the given body.
    pub fn body(self, body: impl Into<Body>) -> Response {
        self.inner.body(body.into()).expect("valid response")
    }

    /// Build a JSON response.
    ///
    /// Serializes `data` as JSON, sets `Content-Type: application/json`,
    /// and returns the response.
    pub fn json<T: serde::Serialize>(self, data: &T) -> Response {
        let json_bytes = serde_json::to_vec(data).expect("valid JSON serialization");
        self.header(http::header::CONTENT_TYPE, "application/json")
            .body(Body::from_bytes(Bytes::from(json_bytes)))
    }

    /// Build an HTML response.
    ///
    /// Sets `Content-Type: text/html; charset=utf-8`.
    pub fn html(self, html: impl Into<String>) -> Response {
        self.header(http::header::CONTENT_TYPE, "text/html; charset=utf-8")
            .body(Body::from(html.into()))
    }

    /// Build a plain text response.
    ///
    /// Sets `Content-Type: text/plain; charset=utf-8`.
    pub fn text(self, text: impl Into<String>) -> Response {
        self.header(http::header::CONTENT_TYPE, "text/plain; charset=utf-8")
            .body(Body::from(text.into()))
    }

    /// Build a response with an empty body.
    pub fn empty(self) -> Response {
        self.body(Body::empty())
    }
}

impl Default for ResponseBuilder {
    fn default() -> Self {
        Self::new()
    }
}

/// Create a redirect response.
///
/// # Variants
///
/// - `Redirect::to(uri)` — 303 See Other
/// - `Redirect::permanent(uri)` — 301 Moved Permanently
/// - `Redirect::temporary(uri)` — 307 Temporary Redirect
pub struct Redirect;

impl Redirect {
    /// 303 See Other redirect.
    pub fn to(uri: &str) -> Response {
        ResponseBuilder::new()
            .status(StatusCode::SEE_OTHER)
            .header(http::header::LOCATION, uri)
            .empty()
    }

    /// 301 Moved Permanently redirect.
    pub fn permanent(uri: &str) -> Response {
        ResponseBuilder::new()
            .status(StatusCode::MOVED_PERMANENTLY)
            .header(http::header::LOCATION, uri)
            .empty()
    }

    /// 307 Temporary Redirect.
    pub fn temporary(uri: &str) -> Response {
        ResponseBuilder::new()
            .status(StatusCode::TEMPORARY_REDIRECT)
            .header(http::header::LOCATION, uri)
            .empty()
    }
}
