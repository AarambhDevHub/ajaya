//! JSON body extractor and response type.
//!
//! Parses the request body as JSON and validates the `Content-Type` header.
//! Also implements [`IntoResponse`] so `Json<T>` can be returned from handlers.
//!
//! # Examples
//!
//! ```rust,ignore
//! use arvik::Json;
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

use arvik_core::body::Body;
use arvik_core::extract::FromRequest;
use arvik_core::into_response::IntoResponse;
use arvik_core::request::Request;
use arvik_core::response::{Response, ResponseBuilder};
use bytes::{BufMut, BytesMut};
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

        // Read body under the configured size limit (2 MiB by default)
        let limit = arvik_core::body_limit::resolve_limit(req.extensions());
        let body_bytes = match req.into_body().to_bytes_limited(limit).await {
            Ok(bytes) => bytes,
            Err(e) if e.is_payload_too_large() => return Err(JsonRejection::PayloadTooLarge),
            Err(e) => return Err(JsonRejection::BodyReadFailed(e.to_string())),
        };

        // Deserialize
        let value = serde_json::from_slice(&body_bytes)
            .map_err(|e| JsonRejection::DeserializationFailed(e.to_string()))?;

        Ok(Json(value))
    }
}

impl<T: serde::Serialize> IntoResponse for Json<T> {
    fn into_response(self) -> Response {
        let mut writer = BytesMut::with_capacity(128).writer();
        match serde_json::to_writer(&mut writer, &self.0) {
            Ok(()) => ResponseBuilder::new()
                .header(http::header::CONTENT_TYPE, "application/json")
                .body(Body::from_bytes(writer.into_inner().freeze())),
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

#[cfg(test)]
mod body_limit_tests {
    use super::*;
    use arvik_core::Body;
    use arvik_core::body_limit::{DEFAULT_BODY_LIMIT, DefaultBodyLimit};

    fn json_request(body: Body) -> Request {
        Request::new(
            http::Request::builder()
                .method(http::Method::POST)
                .uri("/")
                .header(http::header::CONTENT_TYPE, "application/json")
                .body(body)
                .unwrap(),
        )
    }

    #[tokio::test]
    async fn oversized_json_is_rejected_with_413() {
        let big = format!("[{}]", "0,".repeat(DEFAULT_BODY_LIMIT / 2));
        let req = json_request(Body::from_bytes(big.into_bytes().into()));
        match Json::<serde_json::Value>::from_request(req, &()).await {
            Err(JsonRejection::PayloadTooLarge) => {}
            other => panic!("expected PayloadTooLarge, got {:?}", other.err()),
        }
    }

    #[tokio::test]
    async fn json_within_limit_extracts() {
        let req = json_request(Body::from_bytes(br#"{"ok":true}"#.to_vec().into()));
        let Json(value) = Json::<serde_json::Value>::from_request(req, &())
            .await
            .unwrap();
        assert_eq!(value["ok"], serde_json::Value::Bool(true));
    }

    #[tokio::test]
    async fn default_body_limit_extension_raises_json_limit() {
        let mut req = json_request(Body::from_bytes(br#"{"ok":1}"#.to_vec().into()));
        req.extensions_mut().insert(DefaultBodyLimit::max(4));
        match Json::<serde_json::Value>::from_request(req, &()).await {
            Err(JsonRejection::PayloadTooLarge) => {}
            other => panic!("expected PayloadTooLarge, got {:?}", other.err()),
        }
    }
}
