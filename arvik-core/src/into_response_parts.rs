//! The [`IntoResponseParts`] trait for appending response headers.
//!
//! Types implementing `IntoResponseParts` can be prepended to any
//! `IntoResponse` value in a tuple to inject extra headers into the
//! response without touching the body:
//!
//! ```rust,ignore
//! use arvik::{AppendHeaders, IntoResponse};
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
//! use arvik_core::into_response_parts::{IntoResponseParts, ResponseParts};
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
use http::header::{HeaderValue, LINK};

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
/// use arvik::AppendHeaders;
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
            let (name, val) = match (key.try_into(), value.try_into()) {
                (Ok(name), Ok(val)) => (name, val),
                (Err(e), _) => {
                    tracing::warn!(
                        error = %e,
                        "AppendHeaders: invalid header name; header dropped"
                    );
                    continue;
                }
                (_, Err(e)) => {
                    tracing::warn!(
                        error = %e,
                        "AppendHeaders: invalid header value; header dropped"
                    );
                    continue;
                }
            };
            parts.headers_mut().append(name, val);
        }
        Ok(parts)
    }
}

// ---------------------------------------------------------------------------
// Preload / PreloadLink
// ---------------------------------------------------------------------------

/// A single `Link: rel=preload` response header value.
///
/// This is Arvik's supported replacement for HTTP/2 server push. Real push
/// promises are not exposed through Hyper mode because browser support is
/// deprecated and the Hyper service API does not provide a stable push surface.
#[derive(Debug, Clone, Eq, PartialEq)]
pub struct PreloadLink {
    href: String,
    as_value: Option<String>,
    mime_type: Option<String>,
    crossorigin: bool,
}

impl PreloadLink {
    /// Create a preload link for `href`.
    pub fn new(href: impl Into<String>) -> Self {
        Self {
            href: href.into(),
            as_value: None,
            mime_type: None,
            crossorigin: false,
        }
    }

    /// Set the preload `as` value, such as `script`, `style`, `font`, or `image`.
    #[must_use]
    pub fn as_type(mut self, as_value: impl Into<String>) -> Self {
        self.as_value = Some(as_value.into());
        self
    }

    /// Set the preload MIME `type` hint.
    #[must_use]
    pub fn mime_type(mut self, mime_type: impl Into<String>) -> Self {
        self.mime_type = Some(mime_type.into());
        self
    }

    /// Add the `crossorigin` parameter.
    #[must_use]
    pub fn crossorigin(mut self) -> Self {
        self.crossorigin = true;
        self
    }

    /// Convenience constructor for JavaScript assets.
    pub fn script(href: impl Into<String>) -> Self {
        Self::new(href).as_type("script")
    }

    /// Convenience constructor for CSS assets.
    pub fn style(href: impl Into<String>) -> Self {
        Self::new(href).as_type("style")
    }

    /// Convenience constructor for font assets.
    pub fn font(href: impl Into<String>) -> Self {
        Self::new(href).as_type("font").crossorigin()
    }

    /// Convenience constructor for image assets.
    pub fn image(href: impl Into<String>) -> Self {
        Self::new(href).as_type("image")
    }

    fn header_value(&self) -> Option<HeaderValue> {
        let mut value = format!("<{}>; rel=preload", self.href);
        if let Some(as_value) = &self.as_value {
            value.push_str("; as=");
            value.push_str(as_value);
        }
        if let Some(mime_type) = &self.mime_type {
            value.push_str("; type=\"");
            value.push_str(mime_type);
            value.push('"');
        }
        if self.crossorigin {
            value.push_str("; crossorigin");
        }
        HeaderValue::from_str(&value).ok()
    }
}

/// Append one or more `Link: rel=preload` headers to a response.
///
/// # Example
///
/// ```rust,ignore
/// async fn page() -> impl IntoResponse {
///     (
///         Preload([
///             PreloadLink::style("/app.css"),
///             PreloadLink::script("/app.js"),
///         ]),
///         Html("<main>Hello</main>"),
///     )
/// }
/// ```
#[derive(Debug, Clone)]
pub struct Preload<I>(pub I);

impl<I> IntoResponseParts for Preload<I>
where
    I: IntoIterator<Item = PreloadLink>,
{
    type Error = Infallible;

    fn into_response_parts(self, mut parts: ResponseParts) -> Result<ResponseParts, Self::Error> {
        for preload in self.0 {
            if let Some(value) = preload.header_value() {
                parts.headers_mut().append(LINK, value);
            }
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn preload_link_serializes_header_value() {
        let link = PreloadLink::font("/fonts/app.woff2").mime_type("font/woff2");
        assert_eq!(
            link.header_value().unwrap(),
            "</fonts/app.woff2>; rel=preload; as=font; type=\"font/woff2\"; crossorigin"
                .parse::<HeaderValue>()
                .unwrap(),
        );
    }

    #[test]
    fn preload_appends_link_headers() {
        let parts = Preload([
            PreloadLink::style("/app.css"),
            PreloadLink::script("/app.js"),
        ])
        .into_response_parts(ResponseParts::new())
        .unwrap();

        let values = parts.headers().get_all(LINK).iter().collect::<Vec<_>>();
        assert_eq!(values.len(), 2);
        assert_eq!(values[0], "</app.css>; rel=preload; as=style");
        assert_eq!(values[1], "</app.js>; rel=preload; as=script");
    }
}
