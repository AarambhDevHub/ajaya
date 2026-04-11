//! Framework error type.
//!
//! Provides the core `Error` type for the Ajaya framework,
//! with HTTP status code and optional public message.
//!
//! # Future (v0.0.5)
//! - `IntoResponse` implementation for `Error`
//! - `Result<T, E>` blanket `IntoResponse` impl
//! - Internal error detail hiding

use std::fmt;

/// Ajaya's framework error type.
///
/// Wraps an inner error with an associated HTTP status code
/// and optional public-facing message.
///
/// # Example (future)
/// ```rust,ignore
/// use ajaya_core::Error;
/// use http::StatusCode;
///
/// let err = Error::new("database connection failed")
///     .with_status(StatusCode::INTERNAL_SERVER_ERROR);
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
