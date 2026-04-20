//! Framework error types.
//!
//! - [`Error`] — the base framework error with status + public message
//! - [`ErrorResponse`] — structured JSON error body builder
//!
//! # Usage
//!
//! ```rust,ignore
//! use ajaya::Error;
//! use http::StatusCode;
//!
//! async fn handler() -> Result<impl IntoResponse, Error> {
//!     let user = db.find().await
//!         .map_err(|e| Error::new(e).with_status(StatusCode::NOT_FOUND))?;
//!     Ok(Json(user))
//! }
//! ```
//!
//! # Custom error types
//!
//! For richer error handling, define your own error enum:
//!
//! ```rust,ignore
//! use ajaya::IntoResponse;
//!
//! #[derive(Debug, thiserror::Error)]
//! pub enum AppError {
//!     #[error("not found")]
//!     NotFound,
//!     #[error("unauthorized")]
//!     Unauthorized,
//!     #[error("database error: {0}")]
//!     Database(#[from] sqlx::Error),
//! }
//!
//! impl IntoResponse for AppError {
//!     fn into_response(self) -> Response {
//!         let (status, msg) = match &self {
//!             Self::NotFound      => (StatusCode::NOT_FOUND, "not found"),
//!             Self::Unauthorized  => (StatusCode::UNAUTHORIZED, "unauthorized"),
//!             Self::Database(_)   => (StatusCode::INTERNAL_SERVER_ERROR, "internal error"),
//!         };
//!         ErrorResponse::new(status).message(msg).into_response()
//!     }
//! }
//! ```

use std::fmt;

use bytes::Bytes;
use http::StatusCode;

use crate::Body;
use crate::into_response::IntoResponse;
use crate::response::{Response, ResponseBuilder};

// ---------------------------------------------------------------------------
// Error
// ---------------------------------------------------------------------------

/// Ajaya's framework error type.
///
/// Wraps an inner error with an HTTP status code and an optional
/// public-facing message. Internal details are **never** leaked to
/// clients — only the `public_message` (or the status's canonical
/// reason phrase) appears in responses.
///
/// # JSON response format
///
/// ```json
/// { "error": "Not Found", "code": 404 }
/// ```
pub struct Error {
    inner: Box<dyn std::error::Error + Send + Sync>,
    status: StatusCode,
    public_message: Option<String>,
}

impl Error {
    /// Create from any error, defaulting to 500 Internal Server Error.
    pub fn new(err: impl Into<Box<dyn std::error::Error + Send + Sync>>) -> Self {
        Self {
            inner: err.into(),
            status: StatusCode::INTERNAL_SERVER_ERROR,
            public_message: None,
        }
    }

    /// Create from a status code (no inner error).
    pub fn from_status(status: StatusCode) -> Self {
        let reason = status.canonical_reason().unwrap_or("Unknown Error");
        Self {
            inner: reason.into(),
            status,
            public_message: Some(reason.to_owned()),
        }
    }

    /// Override the HTTP status code.
    #[must_use]
    pub fn with_status(mut self, status: StatusCode) -> Self {
        self.status = status;
        self
    }

    /// Set a safe public-facing message (shown to clients).
    #[must_use]
    pub fn with_message(mut self, msg: impl Into<String>) -> Self {
        self.public_message = Some(msg.into());
        self
    }

    /// The HTTP status code.
    pub fn status(&self) -> StatusCode {
        self.status
    }

    /// The public-facing message, if set.
    pub fn public_message(&self) -> Option<&str> {
        self.public_message.as_deref()
    }

    /// The internal error (not exposed to clients).
    pub fn inner(&self) -> &(dyn std::error::Error + Send + Sync) {
        &*self.inner
    }
}

impl fmt::Debug for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Error")
            .field("status", &self.status)
            .field("public_message", &self.public_message)
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
            .to_owned();
        ErrorResponse::new(status).message(message).into_response()
    }
}

// --- From impls ---

impl From<std::io::Error> for Error {
    fn from(e: std::io::Error) -> Self {
        Self::new(e)
    }
}

impl From<serde_json::Error> for Error {
    fn from(e: serde_json::Error) -> Self {
        Self::new(e).with_status(StatusCode::BAD_REQUEST)
    }
}

impl From<http::Error> for Error {
    fn from(e: http::Error) -> Self {
        Self::new(e)
    }
}

impl From<String> for Error {
    fn from(msg: String) -> Self {
        Self::new(msg.clone()).with_message(msg)
    }
}

impl From<&'static str> for Error {
    fn from(msg: &'static str) -> Self {
        Self::new(msg).with_message(msg)
    }
}

// --- Box<dyn std::error::Error + Send + Sync> ---

impl From<Box<dyn std::error::Error + Send + Sync>> for Error {
    fn from(e: Box<dyn std::error::Error + Send + Sync>) -> Self {
        Self {
            inner: e,
            status: StatusCode::INTERNAL_SERVER_ERROR,
            public_message: None,
        }
    }
}

// ---------------------------------------------------------------------------
// ErrorResponse — structured JSON error builder  ← 0.3.4 addition
// ---------------------------------------------------------------------------

/// Builder for a standardised JSON error response body.
///
/// Produces:
///
/// ```json
/// {
///   "error":      "Not Found",
///   "code":       404,
///   "request_id": "a1b2c3d4"   // optional
/// }
/// ```
///
/// # Examples
///
/// ```rust,ignore
/// use ajaya::ErrorResponse;
/// use http::StatusCode;
///
/// // In a custom AppError IntoResponse impl:
/// ErrorResponse::new(StatusCode::NOT_FOUND)
///     .message("User not found")
///     .into_response()
///
/// // With a request ID (set by RequestIdLayer when you add it in 0.4.x):
/// ErrorResponse::new(StatusCode::FORBIDDEN)
///     .message("Access denied")
///     .request_id("req-abc-123")
///     .into_response()
/// ```
#[derive(Debug, Clone)]
pub struct ErrorResponse {
    status: StatusCode,
    message: Option<String>,
    request_id: Option<String>,
}

impl ErrorResponse {
    /// Create a new `ErrorResponse` with the given status code.
    pub fn new(status: StatusCode) -> Self {
        Self {
            status,
            message: None,
            request_id: None,
        }
    }

    /// Set the human-readable error message (safe to show to clients).
    ///
    /// Defaults to the status's canonical reason phrase if not set.
    #[must_use]
    pub fn message(mut self, msg: impl Into<String>) -> Self {
        self.message = Some(msg.into());
        self
    }

    /// Set the request ID (optional, for distributed tracing).
    #[must_use]
    pub fn request_id(mut self, id: impl Into<String>) -> Self {
        self.request_id = Some(id.into());
        self
    }

    /// Build the JSON body as raw bytes.
    fn build_body(&self) -> Vec<u8> {
        let msg = self
            .message
            .as_deref()
            .or_else(|| self.status.canonical_reason())
            .unwrap_or("Internal Server Error");

        let code = self.status.as_u16();

        if let Some(rid) = &self.request_id {
            serde_json::json!({
                "error":      msg,
                "code":       code,
                "request_id": rid,
            })
        } else {
            serde_json::json!({
                "error": msg,
                "code":  code,
            })
        }
        .to_string()
        .into_bytes()
    }
}

impl IntoResponse for ErrorResponse {
    fn into_response(self) -> Response {
        let status = self.status;
        let body = Bytes::from(self.build_body());
        ResponseBuilder::new()
            .status(status)
            .header(http::header::CONTENT_TYPE, "application/json")
            .body(Body::from_bytes(body))
    }
}
