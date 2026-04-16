//! URL-encoded form body extractor.
//!
//! Parses the request body as `application/x-www-form-urlencoded`.
//!
//! # Examples
//!
//! ```rust,ignore
//! use ajaya::Form;
//! use serde::Deserialize;
//!
//! #[derive(Deserialize)]
//! struct LoginForm { username: String, password: String }
//!
//! async fn login(Form(form): Form<LoginForm>) -> String {
//!     format!("Logging in as: {}", form.username)
//! }
//! ```

use ajaya_core::extract::FromRequest;
use ajaya_core::request::Request;
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

        // Read body
        let body_bytes = req
            .into_body()
            .to_bytes()
            .await
            .map_err(|e| FormRejection::BodyReadFailed(e.to_string()))?;

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

    content_type
        .to_lowercase()
        .starts_with("application/x-www-form-urlencoded")
}
