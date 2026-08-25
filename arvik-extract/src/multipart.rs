//! Multipart form data extractor for file uploads.
//!
//! Wraps the [`multer`] crate for streaming multipart parsing. The extractor
//! validates `Content-Type: multipart/form-data`, enforces configurable size
//! and field-count limits, and exposes helpers for chunk streaming, progress
//! reporting, and secure temporary-file writes.
//!
//! # Examples
//!
//! ```rust,ignore
//! use arvik::Multipart;
//!
//! async fn upload(mut multipart: Multipart) -> &'static str {
//!     while let Some(mut field) = multipart.next_field().await.unwrap() {
//!         let name = field.name().unwrap_or("unknown").to_string();
//!         while let Some(chunk) = field.chunk().await.unwrap() {
//!             println!("Field `{name}`: {} bytes", chunk.len());
//!         }
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
//! Multer's streaming size limit fires the same rejection once the streamed
//! body grows past `max_total_size`.

use std::path::{Path, PathBuf};
use std::pin::Pin;
use std::task::{Context, Poll};

use arvik_core::body::Body;
use arvik_core::extract::FromRequest;
use arvik_core::into_response::IntoResponse;
use arvik_core::request::Request;
use arvik_core::response::{Response, ResponseBuilder};
use bytes::Bytes;
use futures_util::Stream;
use http::StatusCode;
use http_body::Body as HttpBody;
use tokio::io::AsyncWriteExt;

use crate::rejection::MultipartRejection;

// ---------------------------------------------------------------------------
// Public types
// ---------------------------------------------------------------------------

/// Multipart form data extractor.
///
/// Validates `Content-Type: multipart/form-data`, enforces size limits,
/// and provides an async iterator over the fields in the multipart body.
#[derive(Debug)]
pub struct Multipart {
    inner: multer::Multipart<'static>,
    config: MultipartConfig,
    fields_read: usize,
}

impl Multipart {
    /// Build a multipart extractor from a request and explicit constraints.
    ///
    /// This is primarily useful for custom extractors and tests. Regular
    /// handlers can continue to use `Multipart` directly as a handler
    /// parameter.
    pub async fn from_request_with_constraints(
        req: Request,
        constraints: MultipartConstraints,
    ) -> Result<Self, MultipartRejection> {
        Self::from_request_with_config(req, MultipartConfig::new().with_constraints(constraints))
            .await
    }

    /// Build a multipart extractor from a request and full multipart config.
    pub async fn from_request_with_config(
        req: Request,
        config: MultipartConfig,
    ) -> Result<Self, MultipartRejection> {
        let boundary = validate_multipart_request(&req, &config)?;
        let multer_constraints = build_multer_constraints(config.constraints);
        let body = req.into_body();
        let stream = BodyStream::new(body);
        let inner = multer::Multipart::with_constraints(stream, boundary, multer_constraints);

        Ok(Self {
            inner,
            config,
            fields_read: 0,
        })
    }

    /// Get the next field from the multipart stream.
    ///
    /// Returns `None` when all fields have been consumed.
    ///
    /// # Errors
    ///
    /// Returns [`MultipartError`] if the stream is malformed, a field
    /// exceeds the per-field size limit, the total stream exceeds the total
    /// size limit, or the field count limit is hit.
    pub async fn next_field(&mut self) -> Result<Option<Field>, MultipartError> {
        match self.inner.next_field().await {
            Ok(Some(field)) => {
                if self.fields_read >= self.config.constraints.max_fields {
                    return Err(MultipartError::TooManyFields {
                        limit: self.config.constraints.max_fields,
                    });
                }

                self.fields_read += 1;
                Ok(Some(Field::new(field, self.config.temp_dir.clone())))
            }
            Ok(None) => Ok(None),
            Err(e) => Err(MultipartError::from(e)),
        }
    }

    /// Return the config used for this multipart parser.
    pub fn config(&self) -> &MultipartConfig {
        &self.config
    }
}

/// A single field from a multipart request.
#[derive(Debug)]
pub struct Field {
    inner: multer::Field<'static>,
    metadata: FieldMetadata,
    temp_dir: Option<PathBuf>,
}

impl Field {
    fn new(inner: multer::Field<'static>, temp_dir: Option<PathBuf>) -> Self {
        let metadata = FieldMetadata::from_field(&inner);
        Self {
            inner,
            metadata,
            temp_dir,
        }
    }

    /// Returns the field name from the `Content-Disposition` header.
    pub fn name(&self) -> Option<&str> {
        self.metadata.name()
    }

    /// Returns the file name, if this field is a file upload.
    pub fn file_name(&self) -> Option<&str> {
        self.metadata.file_name()
    }

    /// Returns the content type of this field.
    pub fn content_type(&self) -> Option<&mime::Mime> {
        self.metadata.content_type()
    }

    /// Return metadata captured from this field's headers.
    pub fn metadata(&self) -> &FieldMetadata {
        &self.metadata
    }

    /// Read the entire field body into [`Bytes`].
    ///
    /// Buffers all chunks into memory. For large files, prefer streaming via
    /// [`Field::chunk`], [`Field::into_stream`], or [`Field::save_to_temp`].
    pub async fn bytes(self) -> Result<Bytes, MultipartError> {
        self.inner.bytes().await.map_err(MultipartError::from)
    }

    /// Read the entire field body as a UTF-8 [`String`].
    pub async fn text(self) -> Result<String, MultipartError> {
        self.inner.text().await.map_err(MultipartError::from)
    }

    /// Get the next chunk of data from this field.
    ///
    /// Returns `None` when the field's data is fully consumed.
    pub async fn chunk(&mut self) -> Result<Option<Bytes>, MultipartError> {
        self.inner.chunk().await.map_err(MultipartError::from)
    }

    /// Convert this field into a byte stream.
    pub fn into_stream(self) -> FieldStream {
        FieldStream { field: self }
    }

    /// Convert this field into a stream that reports cumulative progress.
    pub fn into_progress_stream(self) -> ProgressStream {
        ProgressStream {
            stream: self.into_stream(),
            bytes_read: 0,
        }
    }

    /// Stream this field to a secure temporary file.
    ///
    /// The returned [`TempFile`] removes the file on drop unless it is
    /// persisted with [`TempFile::persist`].
    pub async fn save_to_temp(self) -> Result<TempFile, MultipartError> {
        let dir = self.temp_dir.clone();
        self.save_to_temp_inner(dir, |_| {}).await
    }

    /// Stream this field to a secure temporary file in the provided directory.
    pub async fn save_to_temp_in<P>(self, dir: P) -> Result<TempFile, MultipartError>
    where
        P: AsRef<Path>,
    {
        self.save_to_temp_inner(Some(dir.as_ref().to_path_buf()), |_| {})
            .await
    }

    /// Stream this field to a secure temporary file and report written bytes.
    pub async fn save_to_temp_with_progress<F>(
        self,
        on_progress: F,
    ) -> Result<TempFile, MultipartError>
    where
        F: FnMut(u64) + Send,
    {
        let dir = self.temp_dir.clone();
        self.save_to_temp_inner(dir, on_progress).await
    }

    /// Stream this field to a secure temporary file in `dir` and report written bytes.
    pub async fn save_to_temp_in_with_progress<P, F>(
        self,
        dir: P,
        on_progress: F,
    ) -> Result<TempFile, MultipartError>
    where
        P: AsRef<Path>,
        F: FnMut(u64) + Send,
    {
        self.save_to_temp_inner(Some(dir.as_ref().to_path_buf()), on_progress)
            .await
    }

    async fn save_to_temp_inner<F>(
        mut self,
        dir: Option<PathBuf>,
        mut on_progress: F,
    ) -> Result<TempFile, MultipartError>
    where
        F: FnMut(u64) + Send,
    {
        let temp = match dir {
            Some(dir) => tempfile::Builder::new()
                .prefix("arvik-upload-")
                .tempfile_in(dir)
                .map_err(MultipartError::Io)?,
            None => tempfile::Builder::new()
                .prefix("arvik-upload-")
                .tempfile()
                .map_err(MultipartError::Io)?,
        };

        let std_file = temp.as_file().try_clone().map_err(MultipartError::Io)?;
        // Buffered: chunked uploads otherwise pay one blocking-pool write
        // syscall per network chunk (audit C15).
        let mut writer =
            tokio::io::BufWriter::with_capacity(64 * 1024, tokio::fs::File::from_std(std_file));
        let mut bytes_written = 0_u64;
        let metadata = self.metadata.clone();

        while let Some(chunk) = self.chunk().await? {
            writer.write_all(&chunk).await.map_err(MultipartError::Io)?;
            bytes_written += chunk.len() as u64;
            on_progress(bytes_written);
        }

        writer.flush().await.map_err(MultipartError::Io)?;

        Ok(TempFile {
            file: temp,
            metadata,
            bytes_written,
        })
    }
}

/// Stream of chunks from a multipart field.
#[derive(Debug)]
pub struct FieldStream {
    field: Field,
}

impl Stream for FieldStream {
    type Item = Result<Bytes, MultipartError>;

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        let this = self.get_mut();
        match Pin::new(&mut this.field.inner).poll_next(cx) {
            Poll::Ready(Some(Ok(bytes))) => Poll::Ready(Some(Ok(bytes))),
            Poll::Ready(Some(Err(e))) => Poll::Ready(Some(Err(MultipartError::from(e)))),
            Poll::Ready(None) => Poll::Ready(None),
            Poll::Pending => Poll::Pending,
        }
    }
}

/// Stream of chunks with cumulative upload progress.
#[derive(Debug)]
pub struct ProgressStream {
    stream: FieldStream,
    bytes_read: u64,
}

impl Stream for ProgressStream {
    type Item = Result<ProgressChunk, MultipartError>;

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        let this = self.get_mut();
        match Pin::new(&mut this.stream).poll_next(cx) {
            Poll::Ready(Some(Ok(bytes))) => {
                this.bytes_read += bytes.len() as u64;
                Poll::Ready(Some(Ok(ProgressChunk {
                    bytes,
                    bytes_read: this.bytes_read,
                })))
            }
            Poll::Ready(Some(Err(e))) => Poll::Ready(Some(Err(e))),
            Poll::Ready(None) => Poll::Ready(None),
            Poll::Pending => Poll::Pending,
        }
    }
}

/// A field chunk plus cumulative bytes read for that field.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProgressChunk {
    bytes: Bytes,
    bytes_read: u64,
}

impl ProgressChunk {
    /// Return the bytes in this chunk.
    pub fn bytes(&self) -> &Bytes {
        &self.bytes
    }

    /// Consume this value and return the bytes in this chunk.
    pub fn into_bytes(self) -> Bytes {
        self.bytes
    }

    /// Return the number of bytes in this chunk.
    pub fn len(&self) -> usize {
        self.bytes.len()
    }

    /// Returns `true` if this chunk contains no bytes.
    pub fn is_empty(&self) -> bool {
        self.bytes.is_empty()
    }

    /// Return the cumulative number of bytes read for this field.
    pub fn bytes_read(&self) -> u64 {
        self.bytes_read
    }
}

/// Metadata captured from a multipart field's headers.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FieldMetadata {
    name: Option<String>,
    file_name: Option<String>,
    content_type: Option<mime::Mime>,
}

impl FieldMetadata {
    fn from_field(field: &multer::Field<'_>) -> Self {
        Self {
            name: field.name().map(ToOwned::to_owned),
            file_name: field.file_name().map(ToOwned::to_owned),
            content_type: field.content_type().cloned(),
        }
    }

    /// Returns the field name from the `Content-Disposition` header.
    pub fn name(&self) -> Option<&str> {
        self.name.as_deref()
    }

    /// Returns the file name, if this field is a file upload.
    pub fn file_name(&self) -> Option<&str> {
        self.file_name.as_deref()
    }

    /// Returns the content type of this field.
    pub fn content_type(&self) -> Option<&mime::Mime> {
        self.content_type.as_ref()
    }
}

/// Secure temporary file written from a multipart field.
///
/// The file is deleted when this value is dropped. Call [`TempFile::persist`]
/// to keep the upload at a permanent path.
#[derive(Debug)]
pub struct TempFile {
    file: tempfile::NamedTempFile,
    metadata: FieldMetadata,
    bytes_written: u64,
}

impl TempFile {
    /// Return the current path of the temporary file.
    pub fn path(&self) -> &Path {
        self.file.path()
    }

    /// Return metadata captured from the uploaded field.
    pub fn metadata(&self) -> &FieldMetadata {
        &self.metadata
    }

    /// Return the number of bytes written to disk.
    pub fn bytes_written(&self) -> u64 {
        self.bytes_written
    }

    /// Persist the temporary file to a permanent path.
    pub fn persist<P>(self, path: P) -> Result<std::fs::File, MultipartError>
    where
        P: AsRef<Path>,
    {
        self.file
            .persist(path)
            .map_err(|err| MultipartError::Io(err.error))
    }

    /// Consume this wrapper and return the underlying named temporary file.
    pub fn into_named_temp_file(self) -> tempfile::NamedTempFile {
        self.file
    }
}

/// Constraints for multipart parsing.
///
/// Use this to limit the size and number of fields in a multipart request.
/// The defaults are conservative but generous:
///
/// | Limit | Default |
/// |---|---|
/// | Max fields | 100 |
/// | Max field size | 5 MB |
/// | Max total size | 50 MB |
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
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
            max_field_size: 5 * 1024 * 1024,
            max_total_size: 50 * 1024 * 1024,
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

/// Multipart parser configuration.
///
/// Insert this into request extensions from middleware to configure the
/// default `Multipart` extractor for a route or router.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct MultipartConfig {
    constraints: MultipartConstraints,
    temp_dir: Option<PathBuf>,
}

impl MultipartConfig {
    /// Create a config with default constraints and default temp directory.
    pub fn new() -> Self {
        Self::default()
    }

    /// Return the configured multipart constraints.
    pub fn constraints(&self) -> MultipartConstraints {
        self.constraints
    }

    /// Return the configured temporary directory, if any.
    pub fn temp_dir(&self) -> Option<&Path> {
        self.temp_dir.as_deref()
    }

    /// Replace the multipart constraints.
    pub fn with_constraints(mut self, constraints: MultipartConstraints) -> Self {
        self.constraints = constraints;
        self
    }

    /// Set the maximum number of fields.
    pub fn max_fields(mut self, max: usize) -> Self {
        self.constraints.max_fields = max;
        self
    }

    /// Set the maximum size per field in bytes.
    pub fn max_field_size(mut self, max: u64) -> Self {
        self.constraints.max_field_size = max;
        self
    }

    /// Set the maximum total body size in bytes.
    pub fn max_total_size(mut self, max: u64) -> Self {
        self.constraints.max_total_size = max;
        self
    }

    /// Use a specific directory for [`Field::save_to_temp`].
    pub fn with_temp_dir<P>(mut self, dir: P) -> Self
    where
        P: Into<PathBuf>,
    {
        self.temp_dir = Some(dir.into());
        self
    }
}

/// Error type for multipart field operations.
///
/// Returned from [`Multipart::next_field`], [`Field::bytes`], [`Field::text`],
/// [`Field::chunk`], [`Field::save_to_temp`], and field streams.
#[derive(Debug)]
pub enum MultipartError {
    /// Error returned by the streaming multipart parser.
    Parser(multer::Error),
    /// More fields were received than allowed by [`MultipartConstraints`].
    TooManyFields {
        /// Configured maximum field count.
        limit: usize,
    },
    /// Temporary-file or filesystem operation failed.
    Io(std::io::Error),
}

impl MultipartError {
    /// Returns `true` if this error was caused by a size limit being exceeded.
    pub fn is_size_exceeded(&self) -> bool {
        matches!(
            self,
            Self::Parser(
                multer::Error::FieldSizeExceeded { .. } | multer::Error::StreamSizeExceeded { .. }
            )
        )
    }

    /// Returns `true` if this error was caused by the field count limit.
    pub fn is_too_many_fields(&self) -> bool {
        matches!(self, Self::TooManyFields { .. })
    }

    /// Returns `true` if any configured multipart limit was exceeded.
    pub fn is_limit_exceeded(&self) -> bool {
        self.is_size_exceeded() || self.is_too_many_fields()
    }

    /// Returns `true` if this error was caused by temporary-file IO.
    pub fn is_io(&self) -> bool {
        matches!(self, Self::Io(_))
    }

    /// Return the HTTP status code that best represents this error.
    pub fn status_code(&self) -> StatusCode {
        match self {
            Self::Parser(
                multer::Error::FieldSizeExceeded { .. } | multer::Error::StreamSizeExceeded { .. },
            )
            | Self::TooManyFields { .. } => StatusCode::PAYLOAD_TOO_LARGE,
            Self::Io(_) => StatusCode::INTERNAL_SERVER_ERROR,
            _ => StatusCode::BAD_REQUEST,
        }
    }
}

impl From<multer::Error> for MultipartError {
    fn from(error: multer::Error) -> Self {
        Self::Parser(error)
    }
}

impl std::fmt::Display for MultipartError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Parser(error) => write!(f, "Multipart error: {error}"),
            Self::TooManyFields { limit } => {
                write!(f, "Multipart field count exceeded limit: {limit}")
            }
            Self::Io(error) => write!(f, "Multipart temporary file error: {error}"),
        }
    }
}

impl std::error::Error for MultipartError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Parser(error) => Some(error),
            Self::Io(error) => Some(error),
            Self::TooManyFields { .. } => None,
        }
    }
}

impl IntoResponse for MultipartError {
    fn into_response(self) -> Response {
        let status = self.status_code();
        let body = self.to_string();
        ResponseBuilder::new()
            .status(status)
            .header(http::header::CONTENT_TYPE, "text/plain; charset=utf-8")
            .body(arvik_core::Body::from(body))
    }
}

// ---------------------------------------------------------------------------
// FromRequest impl
// ---------------------------------------------------------------------------

impl<S: Send + Sync> FromRequest<S> for Multipart {
    type Rejection = MultipartRejection;

    async fn from_request(req: Request, _state: &S) -> Result<Self, Self::Rejection> {
        let config = if let Some(config) = req.extension::<MultipartConfig>() {
            config.clone()
        } else if let Some(constraints) = req.extension::<MultipartConstraints>() {
            MultipartConfig::new().with_constraints(*constraints)
        } else {
            MultipartConfig::default()
        };

        Self::from_request_with_config(req, config).await
    }
}

// ---------------------------------------------------------------------------
// Internal helpers
// ---------------------------------------------------------------------------

fn validate_multipart_request(
    req: &Request,
    config: &MultipartConfig,
) -> Result<String, MultipartRejection> {
    if let Some(content_length) = req
        .headers()
        .get(http::header::CONTENT_LENGTH)
        .and_then(|v| v.to_str().ok())
        .and_then(|s| s.parse::<u64>().ok())
    {
        if content_length > config.constraints.max_total_size {
            return Err(MultipartRejection::PayloadTooLarge);
        }
    }

    let content_type = req
        .headers()
        .get(http::header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .ok_or(MultipartRejection::InvalidContentType)?;

    extract_boundary(content_type)
}

/// Extract the `boundary` parameter without a full `mime::Mime` parse
/// (audit C15). Semantics mirror what multer got from the mime crate:
/// media type must be exactly `multipart/form-data` (no suffix), parameter
/// names are case-insensitive, values may be quoted, and the FIRST boundary
/// parameter wins.
fn extract_boundary(content_type: &str) -> Result<String, MultipartRejection> {
    let mut segments = content_type.split(';');
    if !segments
        .next()
        .unwrap_or("")
        .trim()
        .eq_ignore_ascii_case("multipart/form-data")
    {
        return Err(MultipartRejection::InvalidContentType);
    }

    let mut boundary = None;
    for param in segments {
        let Some(eq) = param.find('=') else {
            continue;
        };
        if !param[..eq].trim().eq_ignore_ascii_case("boundary") {
            continue;
        }
        let value = param[eq + 1..].trim();
        let value = value
            .strip_prefix('"')
            .and_then(|v| v.strip_suffix('"'))
            .unwrap_or(value);
        if boundary.is_none() && !value.is_empty() {
            boundary = Some(value.to_string());
        }
    }

    boundary.ok_or(MultipartRejection::MissingBoundary)
}

fn build_multer_constraints(constraints: MultipartConstraints) -> multer::Constraints {
    multer::Constraints::new().size_limit(
        multer::SizeLimit::new()
            .whole_stream(constraints.max_total_size)
            .per_field(constraints.max_field_size),
    )
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
        loop {
            match HttpBody::poll_frame(Pin::new(&mut self.body), cx) {
                Poll::Ready(Some(Ok(frame))) => match frame.into_data() {
                    Ok(data) => return Poll::Ready(Some(Ok(data))),
                    Err(_) => continue,
                },
                Poll::Ready(Some(Err(e))) => {
                    return Poll::Ready(Some(Err(std::io::Error::other(format!("{e}")))));
                }
                Poll::Ready(None) => return Poll::Ready(None),
                Poll::Pending => return Poll::Pending,
            }
        }
    }
}

#[cfg(test)]
mod boundary_tests {
    use super::*;

    #[test]
    fn extracts_plain_quoted_and_case_variants() {
        assert_eq!(
            extract_boundary("multipart/form-data; boundary=XyZ123").unwrap(),
            "XyZ123"
        );
        assert_eq!(
            extract_boundary("multipart/form-data; boundary=\"quoted-value\"").unwrap(),
            "quoted-value"
        );
        assert_eq!(
            extract_boundary("MULTIPART/FORM-DATA; BOUNDARY=upper").unwrap(),
            "upper"
        );
        // Whitespace around the parameter and value is tolerated.
        assert_eq!(
            extract_boundary("multipart/form-data ;  boundary =  spaced ").unwrap(),
            "spaced"
        );
        // First boundary parameter wins (mime-crate semantics).
        assert_eq!(
            extract_boundary("multipart/form-data; boundary=first; boundary=second").unwrap(),
            "first"
        );
        // Other parameters are skipped.
        assert_eq!(
            extract_boundary("multipart/form-data; charset=utf-8; boundary=late").unwrap(),
            "late"
        );
    }

    #[test]
    fn rejects_non_multipart_and_missing_boundary() {
        assert!(matches!(
            extract_boundary("application/json; boundary=x"),
            Err(MultipartRejection::InvalidContentType)
        ));
        // Suffixed subtypes were rejected by multer too.
        assert!(matches!(
            extract_boundary("multipart/form-data+xml; boundary=x"),
            Err(MultipartRejection::InvalidContentType)
        ));
        assert!(matches!(
            extract_boundary("multipart/form-data"),
            Err(MultipartRejection::MissingBoundary)
        ));
        assert!(matches!(
            extract_boundary("multipart/form-data; boundary="),
            Err(MultipartRejection::MissingBoundary)
        ));
    }
}
