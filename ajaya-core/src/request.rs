//! HTTP Request wrapper.
//!
//! Provides a unified [`Request`] type that wraps [`http::Request`]
//! with additional framework extensions.
//!
//! # Examples
//!
//! ```rust,ignore
//! async fn handler(req: Request) -> impl IntoResponse {
//!     let method = req.method();
//!     let uri = req.uri();
//!     "Hello from Ajaya"
//! }
//! ```

use http::Extensions;

use crate::Body;

/// Ajaya's HTTP request type.
///
/// Wraps [`http::Request`] with additional framework-specific
/// extensions and convenience methods. The framework extensions
/// are stored separately from the HTTP extensions, allowing
/// middleware and extractors to attach typed data.
pub struct Request<B = Body> {
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

    /// Returns the HTTP version of this request.
    pub fn version(&self) -> http::Version {
        self.inner.version()
    }

    /// Returns the headers of this request.
    pub fn headers(&self) -> &http::HeaderMap {
        self.inner.headers()
    }

    /// Returns a mutable reference to the headers.
    pub fn headers_mut(&mut self) -> &mut http::HeaderMap {
        self.inner.headers_mut()
    }

    /// Returns a reference to the framework extensions.
    ///
    /// These are separate from the HTTP extensions and are used
    /// by the framework to pass typed data between middleware
    /// and handlers.
    pub fn extensions(&self) -> &Extensions {
        &self.extensions
    }

    /// Returns a mutable reference to the framework extensions.
    pub fn extensions_mut(&mut self) -> &mut Extensions {
        &mut self.extensions
    }

    /// Get a typed extension by reference.
    ///
    /// Returns `None` if the extension is not present.
    pub fn extension<T: Send + Sync + 'static>(&self) -> Option<&T> {
        self.extensions.get::<T>()
    }

    /// Returns a reference to the request body.
    pub fn body(&self) -> &B {
        self.inner.body()
    }

    /// Consumes the request and returns the body.
    pub fn into_body(self) -> B {
        self.inner.into_body()
    }

    /// Decompose the request into its parts and body.
    pub fn into_parts(self) -> (http::request::Parts, B) {
        self.inner.into_parts()
    }

    /// Map the body of this request using the provided closure.
    pub fn map_body<B2>(self, f: impl FnOnce(B) -> B2) -> Request<B2> {
        let extensions = self.extensions;
        let inner = self.inner.map(f);
        Request { inner, extensions }
    }
}

impl Request<Body> {
    /// Decompose this request into framework-aware [`RequestParts`] and a [`Body`].
    ///
    /// This is the primary method used by the handler system to split
    /// a request for extractor processing:
    ///
    /// 1. `FromRequestParts` extractors operate on `&mut RequestParts`
    /// 2. `FromRequest` extractors operate on the reconstructed `Request`
    ///
    /// Use [`from_request_parts()`](Request::from_request_parts) to reassemble.
    pub fn into_request_parts(self) -> (crate::request_parts::RequestParts, Body) {
        let extensions = self.extensions;
        let (http_parts, body) = self.inner.into_parts();
        (
            crate::request_parts::RequestParts {
                inner: http_parts,
                extensions,
            },
            body,
        )
    }

    /// Reconstruct a `Request` from [`RequestParts`] and a [`Body`].
    ///
    /// This is the inverse of [`into_request_parts()`](Request::into_request_parts).
    pub fn from_request_parts(parts: crate::request_parts::RequestParts, body: Body) -> Self {
        let inner = http::Request::from_parts(parts.inner, body);
        Self {
            inner,
            extensions: parts.extensions,
        }
    }

    /// Convert a Hyper incoming request into an Ajaya `Request`.
    ///
    /// This wraps the Hyper `Incoming` body into Ajaya's [`Body`] type,
    /// making it compatible with the framework's extractor and handler system.
    pub fn from_hyper(req: http::Request<hyper::body::Incoming>) -> Self {
        let (parts, incoming) = req.into_parts();
        let body = Body::new(incoming);
        let inner = http::Request::from_parts(parts, body);
        Self {
            extensions: Extensions::default(),
            inner,
        }
    }
}

impl<B> std::fmt::Debug for Request<B> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Request")
            .field("method", self.method())
            .field("uri", self.uri())
            .field("version", &self.version())
            .field("headers", self.headers())
            .finish()
    }
}
