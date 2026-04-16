//! Multipart form data extractor for file uploads.
//!
//! Wraps the [`multer`] crate for streaming multipart parsing.
//!
//! # Examples
//!
//! ```rust,ignore
//! use ajaya::Multipart;
//!
//! async fn upload(mut multipart: Multipart) -> &'static str {
//!     while let Some(mut field) = multipart.next_field().await.unwrap() {
//!         let name = field.name().unwrap_or("unknown").to_string();
//!         let file_name = field.file_name().map(|s| s.to_string());
//!
//!         // Read all bytes at once
//!         let data = field.bytes().await.unwrap();
//!         println!("Field `{name}`: {} bytes", data.len());
//!
//!         // Or stream chunks:
//!         // while let Some(chunk) = field.chunk().await.unwrap() { ... }
//!     }
//!     "Upload complete"
//! }
//! ```

use std::pin::Pin;
use std::task::{Context, Poll};

use ajaya_core::body::Body;
use ajaya_core::extract::FromRequest;
use ajaya_core::request::Request;
use bytes::Bytes;
use futures_util::Stream;
use http_body::Body as HttpBody;

use crate::rejection::MultipartRejection;

/// Multipart form data extractor.
///
/// Validates `Content-Type: multipart/form-data` and provides
/// an async iterator over the fields in the multipart body.
pub struct Multipart {
    inner: multer::Multipart<'static>,
}

impl Multipart {
    /// Get the next field from the multipart stream.
    ///
    /// Returns `None` when all fields have been consumed.
    pub async fn next_field(&mut self) -> Result<Option<Field>, MultipartError> {
        self.inner
            .next_field()
            .await
            .map(|opt| opt.map(|f| Field { inner: f }))
            .map_err(MultipartError)
    }
}

/// A single field from a multipart request.
pub struct Field {
    inner: multer::Field<'static>,
}

impl Field {
    /// Returns the field name.
    pub fn name(&self) -> Option<&str> {
        self.inner.name()
    }

    /// Returns the file name, if this field is a file upload.
    pub fn file_name(&self) -> Option<&str> {
        self.inner.file_name()
    }

    /// Returns the content type of this field.
    pub fn content_type(&self) -> Option<&mime::Mime> {
        self.inner.content_type()
    }

    /// Read the entire field body into [`Bytes`].
    pub async fn bytes(self) -> Result<Bytes, MultipartError> {
        self.inner.bytes().await.map_err(MultipartError)
    }

    /// Read the entire field body as a UTF-8 [`String`].
    pub async fn text(self) -> Result<String, MultipartError> {
        self.inner.text().await.map_err(MultipartError)
    }

    /// Get the next chunk of data from this field.
    ///
    /// Returns `None` when the field data is fully consumed.
    pub async fn chunk(&mut self) -> Result<Option<Bytes>, MultipartError> {
        self.inner.chunk().await.map_err(MultipartError)
    }
}

/// Constraints for multipart parsing.
///
/// Use this to limit the size and number of fields
/// in a multipart request to prevent abuse.
#[derive(Debug, Clone)]
pub struct MultipartConstraints {
    /// Maximum number of fields (default: 100).
    pub max_fields: usize,
    /// Maximum size of a single field in bytes (default: 5 MB).
    pub max_field_size: u64,
    /// Maximum total size of all fields in bytes (default: 50 MB).
    pub max_total_size: u64,
}

impl Default for MultipartConstraints {
    fn default() -> Self {
        Self {
            max_fields: 100,
            max_field_size: 5 * 1024 * 1024,  // 5 MB
            max_total_size: 50 * 1024 * 1024, // 50 MB
        }
    }
}

impl MultipartConstraints {
    /// Create new constraints with default values.
    pub fn new() -> Self {
        Self::default()
    }

    /// Set the maximum number of fields.
    pub fn max_fields(mut self, max: usize) -> Self {
        self.max_fields = max;
        self
    }

    /// Set the maximum size per field in bytes.
    pub fn max_field_size(mut self, max: u64) -> Self {
        self.max_field_size = max;
        self
    }

    /// Set the maximum total size in bytes.
    pub fn max_total_size(mut self, max: u64) -> Self {
        self.max_total_size = max;
        self
    }
}

/// Error type for multipart operations.
#[derive(Debug)]
pub struct MultipartError(multer::Error);

impl std::fmt::Display for MultipartError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Multipart error: {}", self.0)
    }
}

impl std::error::Error for MultipartError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(&self.0)
    }
}

impl<S: Send + Sync> FromRequest<S> for Multipart {
    type Rejection = MultipartRejection;

    async fn from_request(req: Request, _state: &S) -> Result<Self, Self::Rejection> {
        // Extract boundary from Content-Type header
        let content_type = req
            .headers()
            .get(http::header::CONTENT_TYPE)
            .and_then(|v| v.to_str().ok())
            .ok_or(MultipartRejection::InvalidContentType)?;

        let boundary = multer::parse_boundary(content_type)
            .map_err(|_| MultipartRejection::MissingBoundary)?;

        // Convert body into a stream for multer
        let body = req.into_body();
        let stream = BodyStream::new(body);
        let multipart = multer::Multipart::new(stream, boundary);

        Ok(Multipart { inner: multipart })
    }
}

// ---------------------------------------------------------------------------
// Internal: adapt Body into a Stream<Item = Result<Bytes, multer::Error>>
// ---------------------------------------------------------------------------

struct BodyStream {
    body: Body,
}

impl BodyStream {
    fn new(body: Body) -> Self {
        Self { body }
    }
}

impl Stream for BodyStream {
    type Item = Result<Bytes, std::io::Error>;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        match HttpBody::poll_frame(Pin::new(&mut self.body), cx) {
            Poll::Ready(Some(Ok(frame))) => {
                match frame.into_data() {
                    Ok(data) => Poll::Ready(Some(Ok(data))),
                    Err(_) => {
                        // Trailers frame — skip and poll again
                        cx.waker().wake_by_ref();
                        Poll::Pending
                    }
                }
            }
            Poll::Ready(Some(Err(e))) => {
                Poll::Ready(Some(Err(std::io::Error::other(format!("{e}")))))
            }
            Poll::Ready(None) => Poll::Ready(None),
            Poll::Pending => Poll::Pending,
        }
    }
}
