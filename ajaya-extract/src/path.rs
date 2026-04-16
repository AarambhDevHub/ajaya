//! Type-safe path parameter extractor.
//!
//! Deserializes URL path parameters into a typed value using serde.
//!
//! # Examples
//!
//! ```rust,ignore
//! use ajaya::Path;
//!
//! // Single parameter
//! async fn get_user(Path(id): Path<u32>) -> String {
//!     format!("User #{id}")
//! }
//!
//! // Multiple parameters (tuple)
//! async fn get_post(Path((user_id, post_id)): Path<(u32, u32)>) -> String {
//!     format!("User #{user_id}, Post #{post_id}")
//! }
//!
//! // Named struct
//! #[derive(Deserialize)]
//! struct PostParams { user_id: u32, post_id: u32 }
//!
//! async fn get_post_named(Path(params): Path<PostParams>) -> String {
//!     format!("User #{}, Post #{}", params.user_id, params.post_id)
//! }
//! ```

use ajaya_core::extract::FromRequestParts;
use ajaya_core::request_parts::RequestParts;
use ajaya_router::PathParams;
use serde::de::DeserializeOwned;

use crate::path_de::PathDeserializer;
use crate::rejection::PathRejection;

/// Type-safe path parameter extractor.
///
/// Extracts path parameters captured by the router and deserializes
/// them into `T` using serde.
///
/// `T` can be:
/// - A single type (e.g., `Path<u32>`) for routes with one parameter
/// - A tuple (e.g., `Path<(u32, String)>`) for positional extraction
/// - A struct with `#[derive(Deserialize)]` for named extraction
#[derive(Debug, Clone)]
pub struct Path<T>(pub T);

impl<S, T> FromRequestParts<S> for Path<T>
where
    T: DeserializeOwned + Send,
    S: Send + Sync,
{
    type Rejection = PathRejection;

    async fn from_request_parts(
        parts: &mut RequestParts,
        _state: &S,
    ) -> Result<Self, Self::Rejection> {
        let path_params = parts
            .extensions()
            .get::<PathParams>()
            .ok_or(PathRejection::MissingPathParams)?;

        let pairs: Vec<(String, String)> = path_params
            .iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect();

        let deserializer = PathDeserializer::new(&pairs);
        let value = T::deserialize(deserializer)
            .map_err(|e| PathRejection::DeserializationFailed(e.into_message()))?;

        Ok(Path(value))
    }
}
