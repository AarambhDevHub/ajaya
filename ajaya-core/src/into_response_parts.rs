//! The [`IntoResponseParts`] trait for appending response headers.
//!
//! Types implementing `IntoResponseParts` can be prepended to any
//! `IntoResponse` value in a tuple to inject extra headers into the
//! response without touching the body:
//!
//! ```rust,ignore
//! use ajaya::{AppendHeaders, IntoResponse};
//! use http::header::CACHE_CONTROL;
//!
//! async fn cached() -> impl IntoResponse {
//!     (AppendHeaders([(CACHE_CONTROL, "max-age=3600")]), "Cached body")
//! }
//! ```
//!
//! # Implementing `IntoResponseParts`
//!
//! ```rust,ignore
//! use ajaya_core::into_response_parts::{IntoResponseParts, ResponseParts};
//!
//! struct MyParts {
//!     correlation_id: String,
//! }
//!
//! impl IntoResponseParts for MyParts {
//!     type Error = std::convert::Infallible;
//!
//!     fn into_response_parts(self, mut parts: ResponseParts) -> Result<ResponseParts, Self::Error> {
//!         parts.headers_mut().insert(
//!             "x-correlation-id",
//!             self.correlation_id.parse().unwrap(),
//!         );
//!         Ok(parts)
//!     }
//! }
//! ```

use std::convert::Infallible;

use http::HeaderMap;

use crate::into_response::IntoResponse;
use crate::response::Response;

// ---------------------------------------------------------------------------
// ResponseParts
// ---------------------------------------------------------------------------

/// Accumulates additional response headers before they are applied
/// to the final [`Response`].
///
/// Obtained by the framework when processing `IntoResponseParts` values
/// in tuple responses. You typically don't construct this directly.
#[derive(Debug, Default)]
pub struct ResponseParts {
    headers: HeaderMap,
}

impl ResponseParts {
    /// Create an empty `ResponseParts`.
    #[inline]
    pub fn new() -> Self {
        Self::default()
    }

    /// Get a reference to the accumulated headers.
    #[inline]
    pub fn headers(&self) -> &HeaderMap {
        &self.headers
    }

    /// Get a mutable reference to the accumulated headers.
    #[inline]
    pub fn headers_mut(&mut self) -> &mut HeaderMap {
        &mut self.headers
    }

    /// Apply these parts to an existing [`Response`], appending all
    /// accumulated headers.
    pub(crate) fn apply_to(self, mut response: Response) -> Response {
        let headers = response.headers_mut();
        for (key, value) in self.headers {
            if let Some(k) = key {
                headers.append(k, value);
            }
        }
        response
    }
}

// ---------------------------------------------------------------------------
// IntoResponseParts trait
// ---------------------------------------------------------------------------

/// Trait for types that can append headers to a response without touching
/// the body.
///
/// Implement this for cookie jars, security headers, custom header sets,
/// or any type that needs to inject headers into a response.
///
/// `IntoResponseParts` types can be used as the first element(s) in a
/// tuple response:
///
/// ```rust,ignore
/// (my_parts, Json(data))           // (P, R)
/// (parts_a, parts_b, Json(data))   // (P1, P2, R)
/// ```
pub trait IntoResponseParts {
    /// The error type returned if header injection fails.
    ///
    /// Use [`Infallible`] if your implementation can never fail.
    type Error: IntoResponse;

    /// Consume `self` and append headers into `parts`.
    ///
    /// Return the modified `parts` on success, or a response-compatible
    /// error on failure.
    fn into_response_parts(self, parts: ResponseParts) -> Result<ResponseParts, Self::Error>;
}

// ---------------------------------------------------------------------------
// AppendHeaders
// ---------------------------------------------------------------------------

/// Append an iterator of `(HeaderName, HeaderValue)` pairs to a response.
///
/// # Examples
///
/// ```rust,ignore
/// use ajaya::AppendHeaders;
/// use http::header::{CACHE_CONTROL, X_CONTENT_TYPE_OPTIONS};
///
/// async fn handler() -> impl IntoResponse {
///     (
///         AppendHeaders([
///             (CACHE_CONTROL, "no-store"),
///             (X_CONTENT_TYPE_OPTIONS, "nosniff"),
///         ]),
///         "Body",
///     )
/// }
/// ```
#[derive(Debug, Clone)]
pub struct AppendHeaders<I>(pub I);

impl<I, K, V> IntoResponseParts for AppendHeaders<I>
where
    I: IntoIterator<Item = (K, V)>,
    K: TryInto<http::header::HeaderName>,
    K::Error: std::fmt::Display,
    V: TryInto<http::header::HeaderValue>,
    V::Error: std::fmt::Display,
{
    type Error = Infallible;

    fn into_response_parts(self, mut parts: ResponseParts) -> Result<ResponseParts, Self::Error> {
        for (key, value) in self.0 {
            // Skip silently on invalid header name/value — mirrors
            // the existing array-tuple behaviour.
            let (Ok(name), Ok(val)) = (key.try_into(), value.try_into()) else {
                continue;
            };
            parts.headers_mut().append(name, val);
        }
        Ok(parts)
    }
}

// ---------------------------------------------------------------------------
// IntoResponseParts for HeaderMap
// ---------------------------------------------------------------------------

impl IntoResponseParts for HeaderMap {
    type Error = Infallible;

    fn into_response_parts(self, mut parts: ResponseParts) -> Result<ResponseParts, Self::Error> {
        for (key, value) in self {
            if let Some(k) = key {
                parts.headers_mut().append(k, value);
            }
        }
        Ok(parts)
    }
}

// ---------------------------------------------------------------------------
// Helper: apply IntoResponseParts to a Response
// ---------------------------------------------------------------------------

/// Apply a single `IntoResponseParts` value to a `Response`.
/// Returns an error response if `into_response_parts` fails.
pub(crate) fn apply_parts<P: IntoResponseParts>(parts_value: P, response: Response) -> Response {
    let acc = ResponseParts::new();
    match parts_value.into_response_parts(acc) {
        Ok(acc) => acc.apply_to(response),
        Err(e) => e.into_response(),
    }
}
