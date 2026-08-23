//! Default request-body size limit for buffering extractors.
//!
//! Extractors that buffer the whole request body ([`Bytes`](bytes::Bytes),
//! [`String`], [`Json`](https://docs.rs/arvik/latest/arvik/struct.Json.html),
//! [`Form`](https://docs.rs/arvik/latest/arvik/struct.Form.html)) enforce a
//! default limit of [`DEFAULT_BODY_LIMIT`] (2 MiB). Requests exceeding the
//! limit are rejected with `413 Payload Too Large` instead of being buffered
//! into memory.
//!
//! # Raising or disabling the limit
//!
//! Insert a [`DefaultBodyLimit`] extension into the request to override the
//! default — e.g. with the convenience
//! [`DefaultBodyLimitLayer`](https://docs.rs/arvik-middleware) from
//! `arvik-middleware`, or any middleware/mapping that inserts the extension:
//!
//! ```rust,ignore
//! use arvik_core::body_limit::DefaultBodyLimit;
//!
//! // Allow 16 MiB bodies on this route.
//! Router::new()
//!     .route("/upload", post(upload))
//!     .layer(DefaultBodyLimitLayer::max(16 * 1024 * 1024));
//! ```

use std::fmt;

/// Extension holding the maximum buffered-body size for this request.
///
/// Extractors resolve the effective limit as: the `DefaultBodyLimit`
/// extension if present, otherwise [`DEFAULT_BODY_LIMIT`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DefaultBodyLimit(pub usize);

impl DefaultBodyLimit {
    /// The limit applied when no extension is present (2 MiB).
    pub const DEFAULT: Self = Self(DEFAULT_BODY_LIMIT);

    /// Create a limit override of `limit` bytes.
    pub fn max(limit: usize) -> Self {
        Self(limit)
    }

    /// Disable the buffered-body limit entirely.
    ///
    /// Only sensible for routes behind other protections (e.g. an upstream
    /// proxy enforcing a size cap).
    pub fn disabled() -> Self {
        Self(usize::MAX)
    }
}

/// The default maximum request-body size buffering extractors accept: 2 MiB,
/// matching common framework defaults.
pub const DEFAULT_BODY_LIMIT: usize = 2 * 1024 * 1024;

/// Canonical error carried by streaming bodies that exceed their size limit
/// while being read (e.g. by [`crate::body::Body`] wrappers enforcing a cap).
///
/// Readers detect it via [`BodyLimitError::is_payload_too_large`] and reject
/// with `413`.
#[derive(Debug)]
pub struct BodyTooLarge;

impl fmt::Display for BodyTooLarge {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "request body exceeds the maximum allowed size")
    }
}

impl std::error::Error for BodyTooLarge {}

/// Error produced when reading a body under a size limit.
#[derive(Debug)]
pub enum BodyLimitError {
    /// The body exceeded the configured limit.
    TooLarge,
    /// The underlying body stream failed.
    Read(crate::body::BoxError),
}

impl BodyLimitError {
    /// True when this failure represents an over-limit body — either caught
    /// directly or surfaced by the underlying stream as [`BodyTooLarge`].
    pub fn is_payload_too_large(&self) -> bool {
        match self {
            Self::TooLarge => true,
            Self::Read(e) => e.is::<BodyTooLarge>(),
        }
    }
}

impl fmt::Display for BodyLimitError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::TooLarge => write!(f, "request body exceeds the maximum allowed size"),
            Self::Read(e) => write!(f, "failed to read request body: {e}"),
        }
    }
}

impl std::error::Error for BodyLimitError {}

impl From<crate::body::BoxError> for BodyLimitError {
    fn from(e: crate::body::BoxError) -> Self {
        Self::Read(e)
    }
}

/// Resolve the effective body limit from request extensions.
///
/// Returns the [`DefaultBodyLimit`] extension value if present, otherwise
/// [`DEFAULT_BODY_LIMIT`].
pub fn resolve_limit(extensions: &http::Extensions) -> usize {
    extensions
        .get::<DefaultBodyLimit>()
        .map(|l| l.0)
        .unwrap_or(DEFAULT_BODY_LIMIT)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolves_extension_over_default() {
        let mut ext = http::Extensions::new();
        assert_eq!(resolve_limit(&ext), DEFAULT_BODY_LIMIT);

        ext.insert(DefaultBodyLimit::max(64));
        assert_eq!(resolve_limit(&ext), 64);

        ext.insert(DefaultBodyLimit::disabled());
        assert_eq!(resolve_limit(&ext), usize::MAX);
    }
}
