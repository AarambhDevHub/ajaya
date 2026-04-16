//! Original URI extractor.
//!
//! Extracts the original request URI before any path rewrites
//! performed by router nesting.
//!
//! # Examples
//!
//! ```rust,ignore
//! use ajaya::OriginalUri;
//!
//! async fn handler(OriginalUri(uri): OriginalUri) -> String {
//!     format!("Original URI: {uri}")
//! }
//! ```

use std::convert::Infallible;

use ajaya_core::extract::FromRequestParts;
use ajaya_core::request_parts::RequestParts;

/// The original request URI before any path rewrites.
///
/// If no `OriginalUri` has been set by the router (e.g., during
/// nesting), falls back to the current URI.
#[derive(Debug, Clone)]
pub struct OriginalUri(pub http::Uri);

impl<S: Send + Sync> FromRequestParts<S> for OriginalUri {
    type Rejection = Infallible;

    async fn from_request_parts(
        parts: &mut RequestParts,
        _state: &S,
    ) -> Result<Self, Self::Rejection> {
        let uri = parts
            .extensions()
            .get::<OriginalUri>()
            .map(|ou| ou.0.clone())
            .unwrap_or_else(|| parts.uri().clone());

        Ok(OriginalUri(uri))
    }
}
