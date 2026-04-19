//! Extractor rejection types.
//!
//! When an extractor fails to extract its value from a request,
//! it returns a "rejection" — an error type that implements
//! [`IntoResponse`] so it can be automatically converted to an
//! appropriate HTTP error response.
//!
//! Each extractor has its own specific rejection type for maximum
//! clarity in error messages.

use ajaya_core::into_response::IntoResponse;
use ajaya_core::response::{Response, ResponseBuilder};
use http::StatusCode;

// ---------------------------------------------------------------------------
// Path rejection
// ---------------------------------------------------------------------------

/// Rejection type for [`Path`](crate::Path) extraction failures.
#[derive(Debug)]
pub enum PathRejection {
    /// No path parameters were found in the request extensions.
    MissingPathParams,
    /// Deserialization of path parameters failed.
    DeserializationFailed(String),
}

impl std::fmt::Display for PathRejection {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::MissingPathParams => write!(f, "No path parameters found"),
            Self::DeserializationFailed(msg) => write!(f, "Invalid path parameters: {msg}"),
        }
    }
}

impl IntoResponse for PathRejection {
    fn into_response(self) -> Response {
        ResponseBuilder::new()
            .status(StatusCode::BAD_REQUEST)
            .header(http::header::CONTENT_TYPE, "text/plain; charset=utf-8")
            .body(ajaya_core::Body::from(self.to_string()))
    }
}

// ---------------------------------------------------------------------------
// Query rejection
// ---------------------------------------------------------------------------

/// Rejection type for [`Query`](crate::Query) extraction failures.
#[derive(Debug)]
pub enum QueryRejection {
    /// No query string was present in the URI.
    MissingQueryString,
    /// Deserialization of query parameters failed.
    DeserializationFailed(String),
}

impl std::fmt::Display for QueryRejection {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::MissingQueryString => write!(f, "Missing query string"),
            Self::DeserializationFailed(msg) => write!(f, "Invalid query string: {msg}"),
        }
    }
}

impl IntoResponse for QueryRejection {
    fn into_response(self) -> Response {
        ResponseBuilder::new()
            .status(StatusCode::BAD_REQUEST)
            .header(http::header::CONTENT_TYPE, "text/plain; charset=utf-8")
            .body(ajaya_core::Body::from(self.to_string()))
    }
}

// ---------------------------------------------------------------------------
// Json rejection
// ---------------------------------------------------------------------------

/// Rejection type for [`Json`](crate::Json) extraction failures.
#[derive(Debug)]
pub enum JsonRejection {
    /// Request body could not be read.
    BodyReadFailed(String),
    /// The `Content-Type` header is not `application/json`.
    MissingJsonContentType,
    /// JSON deserialization failed.
    DeserializationFailed(String),
}

impl std::fmt::Display for JsonRejection {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::BodyReadFailed(msg) => write!(f, "Failed to read request body: {msg}"),
            Self::MissingJsonContentType => {
                write!(f, "Expected Content-Type: application/json")
            }
            Self::DeserializationFailed(msg) => write!(f, "Invalid JSON: {msg}"),
        }
    }
}

impl IntoResponse for JsonRejection {
    fn into_response(self) -> Response {
        let status = match &self {
            JsonRejection::MissingJsonContentType => StatusCode::UNSUPPORTED_MEDIA_TYPE,
            _ => StatusCode::BAD_REQUEST,
        };
        ResponseBuilder::new()
            .status(status)
            .header(http::header::CONTENT_TYPE, "text/plain; charset=utf-8")
            .body(ajaya_core::Body::from(self.to_string()))
    }
}

// ---------------------------------------------------------------------------
// Form rejection
// ---------------------------------------------------------------------------

/// Rejection type for [`Form`](crate::Form) extraction failures.
#[derive(Debug)]
pub enum FormRejection {
    /// Request body could not be read.
    BodyReadFailed(String),
    /// The `Content-Type` is not `application/x-www-form-urlencoded`.
    InvalidContentType,
    /// Form deserialization failed.
    DeserializationFailed(String),
}

impl std::fmt::Display for FormRejection {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::BodyReadFailed(msg) => write!(f, "Failed to read request body: {msg}"),
            Self::InvalidContentType => {
                write!(
                    f,
                    "Expected Content-Type: application/x-www-form-urlencoded"
                )
            }
            Self::DeserializationFailed(msg) => write!(f, "Invalid form data: {msg}"),
        }
    }
}

impl IntoResponse for FormRejection {
    fn into_response(self) -> Response {
        let status = match &self {
            FormRejection::InvalidContentType => StatusCode::UNSUPPORTED_MEDIA_TYPE,
            _ => StatusCode::BAD_REQUEST,
        };
        ResponseBuilder::new()
            .status(status)
            .header(http::header::CONTENT_TYPE, "text/plain; charset=utf-8")
            .body(ajaya_core::Body::from(self.to_string()))
    }
}

// ---------------------------------------------------------------------------
// TypedHeader rejection
// ---------------------------------------------------------------------------

/// Rejection type for [`TypedHeader`](crate::TypedHeader) extraction failures.
#[derive(Debug)]
pub enum TypedHeaderRejection {
    /// The header is missing from the request.
    Missing(String),
    /// The header value could not be decoded.
    DecodeFailed(String),
}

impl std::fmt::Display for TypedHeaderRejection {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Missing(name) => write!(f, "Missing header: {name}"),
            Self::DecodeFailed(msg) => write!(f, "Invalid header value: {msg}"),
        }
    }
}

impl IntoResponse for TypedHeaderRejection {
    fn into_response(self) -> Response {
        ResponseBuilder::new()
            .status(StatusCode::BAD_REQUEST)
            .header(http::header::CONTENT_TYPE, "text/plain; charset=utf-8")
            .body(ajaya_core::Body::from(self.to_string()))
    }
}

// ---------------------------------------------------------------------------
// Extension rejection
// ---------------------------------------------------------------------------

/// Rejection type for [`Extension`](crate::Extension) extraction failures.
#[derive(Debug)]
pub struct ExtensionRejection(pub String);

impl std::fmt::Display for ExtensionRejection {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Missing request extension: {}", self.0)
    }
}

impl IntoResponse for ExtensionRejection {
    fn into_response(self) -> Response {
        ResponseBuilder::new()
            .status(StatusCode::INTERNAL_SERVER_ERROR)
            .header(http::header::CONTENT_TYPE, "text/plain; charset=utf-8")
            .body(ajaya_core::Body::from(self.to_string()))
    }
}

// ---------------------------------------------------------------------------
// State rejection
// ---------------------------------------------------------------------------

/// Rejection type for [`State`](crate::State) extraction failures.
#[derive(Debug)]
pub struct StateRejection;

impl std::fmt::Display for StateRejection {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "State not configured. Use `.with_state(state)` on your Router"
        )
    }
}

impl IntoResponse for StateRejection {
    fn into_response(self) -> Response {
        ResponseBuilder::new()
            .status(StatusCode::INTERNAL_SERVER_ERROR)
            .header(http::header::CONTENT_TYPE, "text/plain; charset=utf-8")
            .body(ajaya_core::Body::from(self.to_string()))
    }
}

// ---------------------------------------------------------------------------
// Body rejection
// ---------------------------------------------------------------------------

/// Rejection type for raw body extraction failures.
#[derive(Debug)]
pub struct BodyRejection(pub String);

impl std::fmt::Display for BodyRejection {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Failed to read request body: {}", self.0)
    }
}

impl IntoResponse for BodyRejection {
    fn into_response(self) -> Response {
        ResponseBuilder::new()
            .status(StatusCode::BAD_REQUEST)
            .header(http::header::CONTENT_TYPE, "text/plain; charset=utf-8")
            .body(ajaya_core::Body::from(self.to_string()))
    }
}

// ---------------------------------------------------------------------------
// String rejection
// ---------------------------------------------------------------------------

/// Rejection type for [`String`] body extraction failures.
#[derive(Debug)]
pub enum StringRejection {
    /// Body read failed.
    BodyReadFailed(String),
    /// Body is not valid UTF-8.
    InvalidUtf8(String),
}

impl std::fmt::Display for StringRejection {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::BodyReadFailed(msg) => write!(f, "Failed to read request body: {msg}"),
            Self::InvalidUtf8(msg) => write!(f, "Request body is not valid UTF-8: {msg}"),
        }
    }
}

impl IntoResponse for StringRejection {
    fn into_response(self) -> Response {
        ResponseBuilder::new()
            .status(StatusCode::BAD_REQUEST)
            .header(http::header::CONTENT_TYPE, "text/plain; charset=utf-8")
            .body(ajaya_core::Body::from(self.to_string()))
    }
}

// ---------------------------------------------------------------------------
// Multipart rejection
// ---------------------------------------------------------------------------

/// Rejection type for [`Multipart`](crate::Multipart) extraction failures.
#[derive(Debug)]
pub enum MultipartRejection {
    /// The `Content-Type` is not `multipart/form-data`.
    InvalidContentType,
    /// Could not extract boundary from Content-Type header.
    MissingBoundary,
    /// Multipart parsing error.
    MultipartError(String),

    /// Request body exceeds the configured size limit.
    ///
    /// Returned when `Content-Length` exceeds `MultipartConstraints::max_total_size`,
    /// or when the streaming body grows past that threshold.
    PayloadTooLarge,
}

impl std::fmt::Display for MultipartRejection {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidContentType => write!(f, "Expected Content-Type: multipart/form-data"),
            Self::MissingBoundary => write!(f, "Missing multipart boundary in Content-Type"),
            Self::MultipartError(msg) => write!(f, "Multipart error: {msg}"),
            Self::PayloadTooLarge => write!(f, "Request payload exceeds the maximum allowed size"),
        }
    }
}

impl IntoResponse for MultipartRejection {
    fn into_response(self) -> Response {
        let status = match &self {
            MultipartRejection::InvalidContentType => StatusCode::UNSUPPORTED_MEDIA_TYPE,
            MultipartRejection::PayloadTooLarge => StatusCode::PAYLOAD_TOO_LARGE,
            _ => StatusCode::BAD_REQUEST,
        };
        ResponseBuilder::new()
            .status(status)
            .header(http::header::CONTENT_TYPE, "text/plain; charset=utf-8")
            .body(ajaya_core::Body::from(self.to_string()))
    }
}
