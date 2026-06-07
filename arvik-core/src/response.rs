//! HTTP Response types.
//!
//! Provides the [`Response`] type alias and a [`ResponseBuilder`]
//! for ergonomic response construction.
//!
//! # Examples
//!
//! ```rust,ignore
//! use arvik_core::response::ResponseBuilder;
//!
//! let response = ResponseBuilder::new()
//!     .status(200)
//!     .header("x-custom", "value")
//!     .text("Hello, Arvik!");
//! ```

use http::StatusCode;

use crate::Body;

/// Arvik's HTTP response type.
///
/// A type alias for [`http::Response`] using Arvik's [`Body`] type.
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

    fn content_length_if_absent(mut self, len: usize) -> Self {
        let has_content_length = self
            .inner
            .headers_ref()
            .is_some_and(|headers| headers.contains_key(http::header::CONTENT_LENGTH));

        if !has_content_length {
            self.inner = self
                .inner
                .header(http::header::CONTENT_LENGTH, content_length(len));
        }

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
            .content_length_if_absent(json_bytes.len())
            .body(Body::from(json_bytes))
    }

    /// Build an HTML response.
    ///
    /// Sets `Content-Type: text/html; charset=utf-8`.
    pub fn html(self, html: impl Into<String>) -> Response {
        let html = html.into();
        let len = html.len();
        self.header(http::header::CONTENT_TYPE, "text/html; charset=utf-8")
            .content_length_if_absent(len)
            .body(Body::from(html))
    }

    /// Build a plain text response.
    ///
    /// Sets `Content-Type: text/plain; charset=utf-8`.
    pub fn text(self, text: impl Into<String>) -> Response {
        let text = text.into();
        let len = text.len();
        self.header(http::header::CONTENT_TYPE, "text/plain; charset=utf-8")
            .content_length_if_absent(len)
            .body(Body::from(text))
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

fn content_length(len: usize) -> http::HeaderValue {
    let mut buf = itoa::Buffer::new();
    http::HeaderValue::from_str(buf.format(len)).expect("valid Content-Length")
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn response_builder_sets_content_length_for_fixed_text() {
        let response = ResponseBuilder::new().text("hello");
        assert_eq!(response.headers()[http::header::CONTENT_LENGTH], "5");
    }

    #[test]
    fn response_builder_preserves_existing_content_length() {
        let response = ResponseBuilder::new()
            .header(http::header::CONTENT_LENGTH, "99")
            .text("hello");
        assert_eq!(response.headers()[http::header::CONTENT_LENGTH], "99");
    }
}
