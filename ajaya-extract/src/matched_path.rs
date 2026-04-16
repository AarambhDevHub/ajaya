//! Matched path extractor.
//!
//! Extracts the route pattern that matched the current request.
//!
//! # Examples
//!
//! ```rust,ignore
//! use ajaya::MatchedPath;
//!
//! async fn handler(MatchedPath(pattern): MatchedPath) -> String {
//!     format!("Route pattern: {pattern}")
//! }
//! // For a request to /users/42, if the route is /users/{id},
//! // pattern will be "/users/{id}"
//! ```

use ajaya_core::extract::FromRequestParts;
use ajaya_core::into_response::IntoResponse;
use ajaya_core::request_parts::RequestParts;
use ajaya_core::response::{Response, ResponseBuilder};
use ajaya_router::MatchedPathExt;

/// The route pattern that matched the current request.
///
/// This is inserted into request extensions by the router during
/// dispatch. If the request hasn't been routed yet (e.g., in
/// outer middleware), this extractor will fail.
#[derive(Debug, Clone)]
pub struct MatchedPath(pub String);

/// Rejection for when no matched path is available.
#[derive(Debug)]
pub struct MatchedPathRejection;

impl std::fmt::Display for MatchedPathRejection {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "No matched path available")
    }
}

impl IntoResponse for MatchedPathRejection {
    fn into_response(self) -> Response {
        ResponseBuilder::new()
            .status(http::StatusCode::INTERNAL_SERVER_ERROR)
            .body(ajaya_core::Body::from(self.to_string()))
    }
}

impl<S: Send + Sync> FromRequestParts<S> for MatchedPath {
    type Rejection = MatchedPathRejection;

    async fn from_request_parts(
        parts: &mut RequestParts,
        _state: &S,
    ) -> Result<Self, Self::Rejection> {
        parts
            .extensions()
            .get::<MatchedPathExt>()
            .map(|ext| MatchedPath(ext.0.clone()))
            .ok_or(MatchedPathRejection)
    }
}
