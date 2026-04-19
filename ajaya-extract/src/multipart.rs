//! Multipart form data extractor for file uploads.
//!
//! Wraps the [`multer`] crate for streaming multipart parsing.
//! Enforces configurable size and field-count limits, returning
//! `413 Payload Too Large` when the `Content-Length` header or
//! streamed body exceeds `MultipartConstraints::max_total_size`.
//!
//! # Examples
//!
//! ```rust,ignore
//! use ajaya::Multipart;
//!
//! async fn upload(mut multipart: Multipart) -> &'static str {
//!     while let Some(mut field) = multipart.next_field().await.unwrap() {
//!         let name = field.name().unwrap_or("unknown").to_string();
//!         let data = field.bytes().await.unwrap();
//!         println!("Field `{name}`: {} bytes", data.len());
//!     }
//!     "Upload complete"
//! }
//! ```
//!
//! # Size limits
//!
//! The default constraints are:
//! - max fields: 100
//! - max field size: 5 MB
//! - max total size: 50 MB
//!
//! A `Content-Length` header exceeding `max_total_size` is rejected
//! immediately with `413 Payload Too Large` before any body is read.
//! Multer's streaming size limit fires the same rejection once the
//! streamed body grows past `max_total_size`.

use std::pin::Pin;
use std::task::{Context, Poll};

use ajaya_core::body::Body;
use ajaya_core::extract::FromRequest;
use ajaya_core::request::Request;
use bytes::Bytes;
use futures_util::Stream;
use http_body::Body as HttpBody;

use crate::rejection::MultipartRejection;

// ---------------------------------------------------------------------------
// Public types
// ---------------------------------------------------------------------------

/// Multipart form data extractor.
///
/// Validates `Content-Type: multipart/form-data`, enforces size limits,
/// and provides an async iterator over the fields in the multipart body.
pub struct Multipart {
    inner: multer::Multipart<'static>,
    /// Remaining field budget. Starts at `constraints.max_fields` and
    /// decrements on each successful `next_field()` call.
    remaining_fields: usize,
}

impl Multipart {
    /// Get the next field from the multipart stream.
    ///
    /// Returns `None` when all fields have been consumed.
    ///
    /// # Errors
    ///
    /// Returns [`MultipartError`] if the stream is malformed, a field
    /// exceeds the per-field size limit, or the field count limit is hit.
    pub async fn next_field(&mut self) -> Result<Option<Field>, MultipartError> {
        if self.remaining_fields == 0 {
            return Err(MultipartError(multer::Error::IncompleteStream));
        }

        match self.inner.next_field().await {
            Ok(Some(f)) => {
                self.remaining_fields -= 1;
                Ok(Some(Field { inner: f }))
            }
            Ok(None) => Ok(None),
            Err(e) => Err(MultipartError(e)),
        }
    }
}

/// A single field from a multipart request.
pub struct Field {
    inner: multer::Field<'static>,
}

impl Field {
    /// Returns the field name from the `Content-Disposition` header.
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
    ///
    /// Buffers all chunks into memory. For large files, prefer
    /// streaming via [`Field::chunk`].
    pub async fn bytes(self) -> Result<Bytes, MultipartError> {
        self.inner.bytes().await.map_err(MultipartError)
    }

    /// Read the entire field body as a UTF-8 [`String`].
    pub async fn text(self) -> Result<String, MultipartError> {
        self.inner.text().await.map_err(MultipartError)
    }

    /// Get the next chunk of data from this field.
    ///
    /// Returns `None` when the field's data is fully consumed.
    pub async fn chunk(&mut self) -> Result<Option<Bytes>, MultipartError> {
        self.inner.chunk().await.map_err(MultipartError)
    }
}

/// Constraints for multipart parsing.
///
/// Use this to limit the size and number of fields in a multipart
/// request. The defaults are conservative but generous:
///
/// | Limit | Default |
/// |---|---|
/// | Max fields | 100 |
/// | Max field size | 5 MB |
/// | Max total size | 50 MB |
#[derive(Debug, Clone)]
pub struct MultipartConstraints {
    /// Maximum number of fields (default: 100).
    pub max_fields: usize,
    /// Maximum size of a single field in bytes (default: 5 MB).
    pub max_field_size: u64,
    /// Maximum total body size in bytes (default: 50 MB).
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
    /// Create constraints with default values.
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

    /// Set the maximum total body size in bytes.
    pub fn max_total_size(mut self, max: u64) -> Self {
        self.max_total_size = max;
        self
    }
}

/// Error type for multipart field operations.
///
/// Returned from [`Field::bytes`], [`Field::text`], and [`Field::chunk`].
/// Check [`MultipartError::is_size_exceeded`] to detect payloads that are
/// too large and return an appropriate client error.
#[derive(Debug)]
pub struct MultipartError(pub(crate) multer::Error);

impl MultipartError {
    /// Returns `true` if this error was caused by a size limit being exceeded.
    ///
    /// Handlers can use this to return a `413 Payload Too Large` response:
    ///
    /// ```rust,ignore
    /// if let Err(e) = field.bytes().await {
    ///     if e.is_size_exceeded() {
    ///         return StatusCode::PAYLOAD_TOO_LARGE.into_response();
    ///     }
    ///     return StatusCode::BAD_REQUEST.into_response();
    /// }
    /// ```
    pub fn is_size_exceeded(&self) -> bool {
        matches!(
            &self.0,
            multer::Error::FieldSizeExceeded { .. } | multer::Error::StreamSizeExceeded { .. }
        )
    }
}

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

// ---------------------------------------------------------------------------
// FromRequest impl
// ---------------------------------------------------------------------------

impl<S: Send + Sync> FromRequest<S> for Multipart {
    type Rejection = MultipartRejection;

    async fn from_request(req: Request, _state: &S) -> Result<Self, Self::Rejection> {
        let constraints = MultipartConstraints::default();

        // ── Early rejection via Content-Length ─────────────────────────────
        // If the client declares a body larger than our limit we can refuse
        // immediately without reading a single byte.
        if let Some(content_length) = req
            .headers()
            .get(http::header::CONTENT_LENGTH)
            .and_then(|v| v.to_str().ok())
            .and_then(|s| s.parse::<u64>().ok())
        {
            if content_length > constraints.max_total_size {
                return Err(MultipartRejection::PayloadTooLarge);
            }
        }

        // ── Validate Content-Type and extract boundary ──────────────────────
        let content_type = req
            .headers()
            .get(http::header::CONTENT_TYPE)
            .and_then(|v| v.to_str().ok())
            .ok_or(MultipartRejection::InvalidContentType)?;

        let boundary = multer::parse_boundary(content_type)
            .map_err(|_| MultipartRejection::MissingBoundary)?;

        // ── Build multer constraints ────────────────────────────────────────
        // Multer enforces per-field and total-stream size limits during
        // streaming, so violations are caught even without a Content-Length.
        let multer_constraints = multer::Constraints::new().size_limit(
            multer::SizeLimit::new()
                .whole_stream(constraints.max_total_size)
                .per_field(constraints.max_field_size),
        );

        // ── Wrap Body into a Stream for multer ─────────────────────────────
        let body = req.into_body();
        let stream = BodyStream::new(body);
        let inner = multer::Multipart::with_constraints(stream, boundary, multer_constraints);

        Ok(Multipart {
            inner,
            remaining_fields: constraints.max_fields,
        })
    }
}

// ---------------------------------------------------------------------------
// Internal: adapt Body into Stream<Item = Result<Bytes, io::Error>>
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
            Poll::Ready(Some(Ok(frame))) => match frame.into_data() {
                Ok(data) => Poll::Ready(Some(Ok(data))),
                Err(_) => {
                    // Trailers frame — skip, re-poll
                    cx.waker().wake_by_ref();
                    Poll::Pending
                }
            },
            Poll::Ready(Some(Err(e))) => {
                Poll::Ready(Some(Err(std::io::Error::other(format!("{e}")))))
            }
            Poll::Ready(None) => Poll::Ready(None),
            Poll::Pending => Poll::Pending,
        }
    }
}
