//! JSON body extractor and response type.
//!
//! Parses the request body as JSON and validates the `Content-Type` header.
//! Also implements [`IntoResponse`] so `Json<T>` can be returned from handlers.
//!
//! # Examples
//!
//! ```rust,ignore
//! use ajaya::Json;
//! use serde::{Deserialize, Serialize};
//!
//! #[derive(Deserialize)]
//! struct CreateUser { name: String, email: String }
//!
//! #[derive(Serialize)]
//! struct UserResponse { id: u32, name: String }
//!
//! // As extractor (request body)
//! async fn create_user(Json(body): Json<CreateUser>) -> Json<UserResponse> {
//!     Json(UserResponse { id: 1, name: body.name })
//! }
//! ```

use ajaya_core::body::Body;
use ajaya_core::extract::FromRequest;
use ajaya_core::into_response::IntoResponse;
use ajaya_core::request::Request;
use ajaya_core::response::{Response, ResponseBuilder};
use bytes::Bytes;
use http::StatusCode;
use serde::de::DeserializeOwned;

use crate::rejection::JsonRejection;

/// JSON body extractor and response type.
///
/// When used as an extractor, parses the request body as JSON.
/// Requires `Content-Type: application/json`.
///
/// When returned from a handler, serializes the inner value as JSON
/// with `Content-Type: application/json`.
#[derive(Debug, Clone)]
pub struct Json<T>(pub T);

impl<S, T> FromRequest<S> for Json<T>
where
    T: DeserializeOwned,
    S: Send + Sync,
{
    type Rejection = JsonRejection;

    async fn from_request(req: Request, _state: &S) -> Result<Self, Self::Rejection> {
        // Validate Content-Type
        if !json_content_type(req.headers()) {
            return Err(JsonRejection::MissingJsonContentType);
        }

        // Read body
        let body_bytes = req
            .into_body()
            .to_bytes()
            .await
            .map_err(|e| JsonRejection::BodyReadFailed(e.to_string()))?;

        // Deserialize
        let value = serde_json::from_slice(&body_bytes)
            .map_err(|e| JsonRejection::DeserializationFailed(e.to_string()))?;

        Ok(Json(value))
    }
}

impl<T: serde::Serialize> IntoResponse for Json<T> {
    fn into_response(self) -> Response {
        match serde_json::to_vec(&self.0) {
            Ok(json_bytes) => ResponseBuilder::new()
                .header(http::header::CONTENT_TYPE, "application/json")
                .body(Body::from_bytes(Bytes::from(json_bytes))),
            Err(err) => {
                let body = format!("{{\"error\":\"JSON serialization failed: {}\"}}", err);
                ResponseBuilder::new()
                    .status(StatusCode::INTERNAL_SERVER_ERROR)
                    .header(http::header::CONTENT_TYPE, "application/json")
                    .body(Body::from(body))
            }
        }
    }
}

/// Check if the Content-Type header indicates JSON.
fn json_content_type(headers: &http::HeaderMap) -> bool {
    let content_type = match headers.get(http::header::CONTENT_TYPE) {
        Some(ct) => ct,
        None => return false,
    };

    let content_type = match content_type.to_str() {
        Ok(s) => s,
        Err(_) => return false,
    };

    let mime: mime::Mime = match content_type.parse() {
        Ok(m) => m,
        Err(_) => return false,
    };

    // Accept application/json and application/*+json (e.g., application/vnd.api+json)
    mime.type_() == mime::APPLICATION
        && (mime.subtype() == mime::JSON
            || mime.suffix().is_some_and(|suffix| suffix == mime::JSON))
}
