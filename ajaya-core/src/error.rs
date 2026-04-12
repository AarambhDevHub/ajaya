//! Framework error type.
//!
//! Provides the core [`Error`] type for the Ajaya framework,
//! with HTTP status code and optional public message.
//!
//! # Error Handling in Handlers
//!
//! Handlers can return `Result<impl IntoResponse, Error>` and use
//! the `?` operator for ergonomic error propagation:
//!
//! ```rust,ignore
//! use ajaya::{Error, Json};
//! use http::StatusCode;
//!
//! async fn handler() -> Result<Json<serde_json::Value>, Error> {
//!     let data = load_data().await
//!         .map_err(|e| Error::new(e).with_status(StatusCode::NOT_FOUND))?;
//!     Ok(Json(data))
//! }
//! ```

use std::fmt;

use bytes::Bytes;

use crate::into_response::IntoResponse;
use crate::response::{Response, ResponseBuilder};

/// Ajaya's framework error type.
///
/// Wraps an inner error with an associated HTTP status code
/// and optional public-facing message. Internal error details
/// are **never** leaked to clients — only the `public_message`
/// (or a generic status text) is included in the response body.
///
/// # JSON Error Response
///
/// When converted to a response, `Error` produces a JSON body:
///
/// ```json
/// {
///     "error": "Not Found",
///     "code": 404
/// }
/// ```
pub struct Error {
    inner: Box<dyn std::error::Error + Send + Sync>,
    status: http::StatusCode,
    public_message: Option<String>,
}

impl Error {
    /// Create a new `Error` from any error type.
    ///
    /// Defaults to `500 Internal Server Error`.
    pub fn new(err: impl Into<Box<dyn std::error::Error + Send + Sync>>) -> Self {
        Self {
            inner: err.into(),
            status: http::StatusCode::INTERNAL_SERVER_ERROR,
            public_message: None,
        }
    }

    /// Create an error with just a status code and message (no inner error).
    pub fn from_status(status: http::StatusCode) -> Self {
        let msg = status.canonical_reason().unwrap_or("Unknown Error");
        Self {
            inner: msg.into(),
            status,
            public_message: Some(msg.to_string()),
        }
    }

    /// Set the HTTP status code for this error.
    pub fn with_status(mut self, status: http::StatusCode) -> Self {
        self.status = status;
        self
    }

    /// Set a public-facing error message.
    ///
    /// This message is safe to include in HTTP responses.
    /// The inner error details are NOT exposed to clients.
    pub fn with_message(mut self, msg: impl Into<String>) -> Self {
        self.public_message = Some(msg.into());
        self
    }

    /// Returns the HTTP status code for this error.
    pub fn status(&self) -> http::StatusCode {
        self.status
    }

    /// Returns the public-facing message, if set.
    pub fn public_message(&self) -> Option<&str> {
        self.public_message.as_deref()
    }
}

impl fmt::Debug for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Error")
            .field("status", &self.status)
            .field("message", &self.public_message)
            .field("inner", &self.inner)
            .finish()
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if let Some(msg) = &self.public_message {
            write!(f, "{} ({})", msg, self.status)
        } else {
            write!(f, "{}", self.status)
        }
    }
}

impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(&*self.inner)
    }
}

impl IntoResponse for Error {
    fn into_response(self) -> Response {
        let status = self.status();
        let message = self
            .public_message()
            .unwrap_or_else(|| status.canonical_reason().unwrap_or("Internal Server Error"))
            .to_string();

        // JSON error body: { "error": "message", "code": 500 }
        let body = serde_json::json!({
            "error": message,
            "code": status.as_u16(),
        });

        let json_bytes = serde_json::to_vec(&body).expect("valid JSON serialization");

        ResponseBuilder::new()
            .status(status)
            .header(http::header::CONTENT_TYPE, "application/json")
            .body(crate::Body::from_bytes(Bytes::from(json_bytes)))
    }
}

// --- From impls for common error types ---

impl From<std::io::Error> for Error {
    fn from(err: std::io::Error) -> Self {
        Self::new(err)
    }
}

impl From<serde_json::Error> for Error {
    fn from(err: serde_json::Error) -> Self {
        Self::new(err).with_status(http::StatusCode::BAD_REQUEST)
    }
}

impl From<http::Error> for Error {
    fn from(err: http::Error) -> Self {
        Self::new(err)
    }
}

impl From<String> for Error {
    fn from(msg: String) -> Self {
        Self::new(msg.as_str()).with_message(msg)
    }
}

impl From<&'static str> for Error {
    fn from(msg: &'static str) -> Self {
        Self::new(msg).with_message(msg)
    }
}
