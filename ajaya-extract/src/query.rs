//! Query string extractor.
//!
//! Deserializes the URI query string into a typed value using serde.
//!
//! # Examples
//!
//! ```rust,ignore
//! use ajaya::Query;
//! use serde::Deserialize;
//!
//! #[derive(Deserialize)]
//! struct SearchParams {
//!     q: String,
//!     page: Option<u32>,
//! }
//!
//! async fn search(Query(params): Query<SearchParams>) -> String {
//!     format!("Searching for: {} (page {})", params.q, params.page.unwrap_or(1))
//! }
//! ```

use ajaya_core::extract::FromRequestParts;
use ajaya_core::request_parts::RequestParts;
use serde::de::DeserializeOwned;

use crate::rejection::QueryRejection;

/// Query string extractor.
///
/// Parses the URI query string (e.g., `?key=value&other=123`)
/// and deserializes it into `T` using `serde_urlencoded`.
#[derive(Debug, Clone)]
pub struct Query<T>(pub T);

impl<S, T> FromRequestParts<S> for Query<T>
where
    T: DeserializeOwned + Send,
    S: Send + Sync,
{
    type Rejection = QueryRejection;

    async fn from_request_parts(
        parts: &mut RequestParts,
        _state: &S,
    ) -> Result<Self, Self::Rejection> {
        let query = parts.uri().query().unwrap_or("");

        let value = serde_urlencoded::from_str(query)
            .map_err(|e| QueryRejection::DeserializationFailed(e.to_string()))?;

        Ok(Query(value))
    }
}
