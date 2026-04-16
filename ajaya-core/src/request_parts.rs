//! Framework-aware request parts.
//!
//! [`RequestParts`] combines the standard HTTP request parts with
//! framework-specific extensions. This is the input type for
//! [`FromRequestParts`] extractors that need access to headers,
//! URI, method, and framework extensions without consuming the body.

use http::Extensions;

/// The non-body parts of a [`Request`](crate::Request), enriched with
/// framework extensions.
///
/// Created by calling [`Request::into_request_parts()`](crate::Request::into_request_parts).
/// Extractors that implement [`FromRequestParts`](crate::extract::FromRequestParts)
/// receive a mutable reference to this type.
///
/// # Fields Accessible
///
/// - HTTP method, URI, version, headers (via `http::request::Parts`)
/// - Framework extensions (path params, matched path, custom data)
#[derive(Debug)]
pub struct RequestParts {
    /// The standard HTTP request parts (method, URI, version, headers, extensions).
    pub(crate) inner: http::request::Parts,
    /// Framework-level extensions (path params, middleware data, etc).
    pub(crate) extensions: Extensions,
}

impl RequestParts {
    /// Returns the HTTP method of the original request.
    #[inline]
    pub fn method(&self) -> &http::Method {
        &self.inner.method
    }

    /// Returns the URI of the original request.
    #[inline]
    pub fn uri(&self) -> &http::Uri {
        &self.inner.uri
    }

    /// Returns the HTTP version of the original request.
    #[inline]
    pub fn version(&self) -> http::Version {
        self.inner.version
    }

    /// Returns a reference to the request headers.
    #[inline]
    pub fn headers(&self) -> &http::HeaderMap {
        &self.inner.headers
    }

    /// Returns a mutable reference to the request headers.
    #[inline]
    pub fn headers_mut(&mut self) -> &mut http::HeaderMap {
        &mut self.inner.headers
    }

    /// Returns a reference to the framework extensions.
    ///
    /// These extensions are set by the router (path params, matched path)
    /// and middleware (current user, request ID, etc).
    #[inline]
    pub fn extensions(&self) -> &Extensions {
        &self.extensions
    }

    /// Returns a mutable reference to the framework extensions.
    #[inline]
    pub fn extensions_mut(&mut self) -> &mut Extensions {
        &mut self.extensions
    }

    /// Returns a reference to the HTTP-level extensions.
    #[inline]
    pub fn http_extensions(&self) -> &Extensions {
        &self.inner.extensions
    }

    /// Returns a mutable reference to the HTTP-level extensions.
    #[inline]
    pub fn http_extensions_mut(&mut self) -> &mut Extensions {
        &mut self.inner.extensions
    }
}
