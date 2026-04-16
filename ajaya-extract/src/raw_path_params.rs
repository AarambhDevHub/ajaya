//! Raw (untyped) path parameters extractor.
//!
//! Returns the path parameters as a list of `(String, String)` pairs
//! without any deserialization. Useful when you need raw access.
//!
//! # Examples
//!
//! ```rust,ignore
//! use ajaya_extract::RawPathParams;
//!
//! async fn handler(RawPathParams(params): RawPathParams) -> String {
//!     let pairs: Vec<String> = params
//!         .iter()
//!         .map(|(k, v)| format!("{k}={v}"))
//!         .collect();
//!     pairs.join(", ")
//! }
//! ```

use std::convert::Infallible;

use ajaya_core::extract::FromRequestParts;
use ajaya_core::request_parts::RequestParts;
use ajaya_router::PathParams;

/// Raw path parameters as `(String, String)` pairs.
///
/// Does not perform any deserialization — returns the raw captured
/// key-value pairs from the router.
#[derive(Debug, Clone)]
pub struct RawPathParams(pub Vec<(String, String)>);

impl<S: Send + Sync> FromRequestParts<S> for RawPathParams {
    type Rejection = Infallible;

    async fn from_request_parts(
        parts: &mut RequestParts,
        _state: &S,
    ) -> Result<Self, Self::Rejection> {
        let params = parts
            .extensions()
            .get::<PathParams>()
            .map(|p| {
                p.iter()
                    .map(|(k, v)| (k.to_string(), v.to_string()))
                    .collect()
            })
            .unwrap_or_default();

        Ok(RawPathParams(params))
    }
}
