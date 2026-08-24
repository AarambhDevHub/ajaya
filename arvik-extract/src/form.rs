//! URL-encoded form body extractor.
//!
//! Parses the request body as `application/x-www-form-urlencoded`.
//!
//! # Examples
//!
//! ```rust,ignore
//! use arvik::Form;
//! use serde::Deserialize;
//!
//! #[derive(Deserialize)]
//! struct LoginForm { username: String, password: String }
//!
//! async fn login(Form(form): Form<LoginForm>) -> String {
//!     format!("Logging in as: {}", form.username)
//! }
//! ```

use arvik_core::extract::FromRequest;
use arvik_core::request::Request;
use serde::de::DeserializeOwned;

use crate::rejection::FormRejection;

/// URL-encoded form body extractor.
///
/// Parses the request body as `application/x-www-form-urlencoded`
/// and deserializes it into `T` using `serde_urlencoded`.
///
/// Validates the `Content-Type` header before parsing.
#[derive(Debug, Clone)]
pub struct Form<T>(pub T);

impl<S, T> FromRequest<S> for Form<T>
where
    T: DeserializeOwned,
    S: Send + Sync,
{
    type Rejection = FormRejection;

    async fn from_request(req: Request, _state: &S) -> Result<Self, Self::Rejection> {
        // Validate Content-Type
        if !form_content_type(req.headers()) {
            return Err(FormRejection::InvalidContentType);
        }

        // Read body under the configured size limit (2 MiB by default)
        let limit = arvik_core::body_limit::resolve_limit(req.extensions());
        let body_bytes = match req.into_body().to_bytes_limited(limit).await {
            Ok(bytes) => bytes,
            Err(e) if e.is_payload_too_large() => return Err(FormRejection::PayloadTooLarge),
            Err(e) => return Err(FormRejection::BodyReadFailed(e.to_string())),
        };

        // Deserialize
        let value = serde_urlencoded::from_bytes(&body_bytes)
            .map_err(|e| FormRejection::DeserializationFailed(e.to_string()))?;

        Ok(Form(value))
    }
}

/// Check if Content-Type is application/x-www-form-urlencoded.
fn form_content_type(headers: &http::HeaderMap) -> bool {
    let content_type = match headers.get(http::header::CONTENT_TYPE) {
        Some(ct) => ct,
        None => return false,
    };

    let content_type = match content_type.to_str() {
        Ok(s) => s,
        Err(_) => return false,
    };

    // Fast path for the exact constant.
    if content_type.eq_ignore_ascii_case("application/x-www-form-urlencoded") {
        return true;
    }

    // Parameterized variants need a real parse.
    match content_type.parse::<mime::Mime>() {
        Ok(mime) => mime.type_() == mime::APPLICATION && mime.subtype() == "x-www-form-urlencoded",
        Err(_) => false,
    }
}

#[cfg(test)]
mod body_limit_tests {
    use super::*;
    use arvik_core::Body;
    use arvik_core::body_limit::{DEFAULT_BODY_LIMIT, DefaultBodyLimit};

    fn form_request(body: Body) -> Request {
        Request::new(
            http::Request::builder()
                .method(http::Method::POST)
                .uri("/")
                .header(
                    http::header::CONTENT_TYPE,
                    "application/x-www-form-urlencoded",
                )
                .body(body)
                .unwrap(),
        )
    }

    #[tokio::test]
    async fn oversized_form_is_rejected_with_413() {
        let big = "k=v&".repeat(DEFAULT_BODY_LIMIT / 2);
        let req = form_request(Body::from_bytes(big.into_bytes().into()));
        match Form::<std::collections::HashMap<String, String>>::from_request(req, &()).await {
            Err(FormRejection::PayloadTooLarge) => {}
            other => panic!("expected PayloadTooLarge, got {:?}", other.err()),
        }
    }

    #[tokio::test]
    async fn form_within_limit_extracts() {
        let req = form_request(Body::from_bytes(b"user=darshan".to_vec().into()));
        let Form(value) = Form::<std::collections::HashMap<String, String>>::from_request(req, &())
            .await
            .unwrap();
        assert_eq!(value["user"], "darshan");
    }

    #[tokio::test]
    async fn default_body_limit_extension_tightens_form_limit() {
        let mut req = form_request(Body::from_bytes(b"user=darshan".to_vec().into()));
        req.extensions_mut().insert(DefaultBodyLimit::max(4));
        match Form::<std::collections::HashMap<String, String>>::from_request(req, &()).await {
            Err(FormRejection::PayloadTooLarge) => {}
            other => panic!("expected PayloadTooLarge, got {:?}", other.err()),
        }
    }
}
