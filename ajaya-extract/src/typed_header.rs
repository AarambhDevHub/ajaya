//! Typed header extractor.
//!
//! Parses a specific HTTP header into a strongly-typed value
//! using the [`headers`] crate.
//!
//! # Examples
//!
//! ```rust,ignore
//! use ajaya::TypedHeader;
//! use headers::ContentType;
//!
//! async fn handler(TypedHeader(ct): TypedHeader<ContentType>) -> String {
//!     format!("Content-Type: {ct}")
//! }
//! ```

use ajaya_core::extract::FromRequestParts;
use ajaya_core::request_parts::RequestParts;
use headers::Header;

use crate::rejection::TypedHeaderRejection;

/// Typed header extractor.
///
/// Uses the [`headers`] crate to parse a specific header from the
/// request. The type `T` must implement [`headers::Header`].
#[derive(Debug, Clone)]
pub struct TypedHeader<T>(pub T);

impl<S, T> FromRequestParts<S> for TypedHeader<T>
where
    T: Header + Send,
    S: Send + Sync,
{
    type Rejection = TypedHeaderRejection;

    async fn from_request_parts(
        parts: &mut RequestParts,
        _state: &S,
    ) -> Result<Self, Self::Rejection> {
        let header_name = T::name();
        let values = parts.headers().get_all(header_name);

        // Collect into a Vec of HeaderValues for decode
        let mut iter = values.iter();

        T::decode(&mut iter).map(TypedHeader).map_err(|e| {
            // Check if the header is missing or malformed
            if parts.headers().get(header_name).is_none() {
                TypedHeaderRejection::Missing(header_name.to_string())
            } else {
                TypedHeaderRejection::DecodeFailed(e.to_string())
            }
        })
    }
}
