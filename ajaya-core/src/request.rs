//! HTTP Request wrapper.
//!
//! Provides a unified `Request` type that wraps `http::Request`
//! with additional framework extensions.
//!
//! # Future (v0.0.2)
//! - Typed extensions via `Extensions` map
//! - Convenience accessors for method, URI, headers
//! - Body consumption helpers

use http::Extensions;

/// Ajaya's HTTP request type.
///
/// Wraps [`http::Request`] with additional framework-specific
/// extensions and convenience methods.
///
/// # Example (future)
/// ```rust,ignore
/// async fn handler(req: Request) -> impl IntoResponse {
///     let method = req.method();
///     let uri = req.uri();
///     "Hello from Ajaya"
/// }
/// ```
pub struct Request<B = crate::Body> {
    inner: http::Request<B>,
    extensions: Extensions,
}

impl<B> Request<B> {
    /// Create a new `Request` from an `http::Request`.
    pub fn new(inner: http::Request<B>) -> Self {
        Self {
            extensions: Extensions::default(),
            inner,
        }
    }

    /// Returns a reference to the underlying `http::Request`.
    pub fn inner(&self) -> &http::Request<B> {
        &self.inner
    }

    /// Consumes this `Request`, returning the inner `http::Request`.
    pub fn into_inner(self) -> http::Request<B> {
        self.inner
    }

    /// Returns the HTTP method of this request.
    pub fn method(&self) -> &http::Method {
        self.inner.method()
    }

    /// Returns the URI of this request.
    pub fn uri(&self) -> &http::Uri {
        self.inner.uri()
    }

    /// Returns the headers of this request.
    pub fn headers(&self) -> &http::HeaderMap {
        self.inner.headers()
    }

    /// Returns a reference to the framework extensions.
    pub fn extensions(&self) -> &Extensions {
        &self.extensions
    }

    /// Returns a mutable reference to the framework extensions.
    pub fn extensions_mut(&mut self) -> &mut Extensions {
        &mut self.extensions
    }

    /// Returns a reference to the request body.
    pub fn body(&self) -> &B {
        self.inner.body()
    }

    /// Consumes the request and returns the body.
    pub fn into_body(self) -> B {
        self.inner.into_body()
    }
}
