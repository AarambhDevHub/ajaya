//! Compression and decompression middleware.
//!
//! [`CompressionLayer`] compresses response bodies based on the client's
//! `Accept-Encoding` header. Supports gzip, brotli, zstd, and deflate.
//!
//! [`DecompressionLayer`] decompresses request bodies based on the
//! `Content-Encoding` header.
//!
//! # Example
//!
//! ```rust,ignore
//! use arvik_middleware::compression::{CompressionLayer, CompressionLevel};
//!
//! let app = Router::new()
//!     .route("/api/data", get(large_handler))
//!     .layer(CompressionLayer::new()
//!         .gzip(true)
//!         .br(true)
//!         .zstd(true)
//!         .quality(CompressionLevel::Default));
//! ```

use std::convert::Infallible;
use std::future::Future;
use std::pin::Pin;
use std::task::{Context, Poll};

use arvik_core::{Body, Request, Response};
use async_compression::tokio::bufread::{
    BrotliDecoder, BrotliEncoder, DeflateDecoder, DeflateEncoder, GzipDecoder, GzipEncoder,
    ZstdDecoder, ZstdEncoder,
};
use http::header::{ACCEPT_ENCODING, CONTENT_ENCODING, CONTENT_LENGTH, VARY};
use http::{HeaderValue, StatusCode};
use tokio::io::AsyncReadExt;
use tokio_util::bytes::Bytes;
use tower_layer::Layer;
use tower_service::Service;

// ── Compression level ────────────────────────────────────────────────────────

/// The compression quality level.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum CompressionLevel {
    /// Fastest compression (lowest ratio).
    Fastest,
    /// Best compression ratio (slowest).
    Best,
    /// Default balance of speed and ratio.
    #[default]
    Default,
}

// ── Encoding selection ───────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Encoding {
    Gzip,
    Br,
    Zstd,
    Deflate,
}

impl Encoding {
    fn header_value(self) -> &'static str {
        match self {
            Encoding::Gzip => "gzip",
            Encoding::Br => "br",
            Encoding::Zstd => "zstd",
            Encoding::Deflate => "deflate",
        }
    }
}

// ── CompressionLayer ─────────────────────────────────────────────────────────

/// Tower layer that compresses response bodies.
#[derive(Debug, Clone)]
pub struct CompressionLayer {
    gzip: bool,
    br: bool,
    zstd: bool,
    deflate: bool,
    level: CompressionLevel,
    min_size: usize,
}

impl Default for CompressionLayer {
    fn default() -> Self {
        Self {
            gzip: true,
            br: true,
            zstd: true,
            deflate: true,
            level: CompressionLevel::Default,
            min_size: 1024,
        }
    }
}

impl CompressionLayer {
    /// Create a new `CompressionLayer` with all encodings enabled.
    pub fn new() -> Self {
        Self::default()
    }

    /// Enable or disable gzip compression.
    pub fn gzip(mut self, enable: bool) -> Self {
        self.gzip = enable;
        self
    }

    /// Enable or disable brotli compression.
    pub fn br(mut self, enable: bool) -> Self {
        self.br = enable;
        self
    }

    /// Enable or disable zstd compression.
    pub fn zstd(mut self, enable: bool) -> Self {
        self.zstd = enable;
        self
    }

    /// Enable or disable deflate compression.
    pub fn deflate(mut self, enable: bool) -> Self {
        self.deflate = enable;
        self
    }

    /// Set the compression quality level.
    pub fn quality(mut self, level: CompressionLevel) -> Self {
        self.level = level;
        self
    }

    /// Minimum response body size in bytes before compression is applied.
    /// Responses smaller than this are passed through uncompressed.
    /// Default: 1024 bytes.
    pub fn min_size(mut self, bytes: usize) -> Self {
        self.min_size = bytes;
        self
    }

    fn preferred_encoding(&self, accept_encoding: &str) -> Option<Encoding> {
        // Simple preference: zstd > br > gzip > deflate
        let lower = accept_encoding.to_lowercase();
        if self.zstd && lower.contains("zstd") {
            return Some(Encoding::Zstd);
        }
        if self.br && (lower.contains("br") || lower.contains("brotli")) {
            return Some(Encoding::Br);
        }
        if self.gzip && lower.contains("gzip") {
            return Some(Encoding::Gzip);
        }
        if self.deflate && lower.contains("deflate") {
            return Some(Encoding::Deflate);
        }
        None
    }
}

impl<S> Layer<S> for CompressionLayer {
    type Service = CompressionService<S>;

    fn layer(&self, inner: S) -> Self::Service {
        CompressionService {
            inner,
            config: self.clone(),
        }
    }
}

/// Tower service produced by [`CompressionLayer`].
#[derive(Clone)]
pub struct CompressionService<S> {
    inner: S,
    config: CompressionLayer,
}

impl<S> Service<Request> for CompressionService<S>
where
    S: Service<Request, Response = Response, Error = Infallible> + Clone + Send + 'static,
    S::Future: Send + 'static,
{
    type Response = Response;
    type Error = Infallible;
    type Future = Pin<Box<dyn Future<Output = Result<Response, Infallible>> + Send + 'static>>;

    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.inner.poll_ready(cx)
    }

    fn call(&mut self, req: Request) -> Self::Future {
        let config = self.config.clone();
        let accept_encoding = req
            .headers()
            .get(ACCEPT_ENCODING)
            .and_then(|v| v.to_str().ok())
            .unwrap_or("")
            .to_string();

        let cloned = self.inner.clone();
        let mut inner = std::mem::replace(&mut self.inner, cloned);

        Box::pin(async move {
            let response = inner.call(req).await?;

            // Don't compress if already encoded.
            if response.headers().contains_key(CONTENT_ENCODING) {
                return Ok(response);
            }

            // Don't compress non-compressible content types.
            if !should_compress(response.headers()) {
                return Ok(response);
            }

            let encoding = match config.preferred_encoding(&accept_encoding) {
                Some(e) => e,
                None => return Ok(response),
            };

            Ok(compress_response(response, encoding, &config).await)
        })
    }
}

async fn compress_response(
    response: Response,
    encoding: Encoding,
    config: &CompressionLayer,
) -> Response {
    let (mut parts, body) = response.into_parts();

    let body_bytes: Bytes = match body.to_bytes().await {
        Ok(b) => b,
        Err(_) => {
            return http::Response::builder()
                .status(StatusCode::INTERNAL_SERVER_ERROR)
                .body(Body::from("Compression error"))
                .unwrap();
        }
    };

    // Skip compression for small bodies.
    if body_bytes.len() < config.min_size {
        parts
            .headers
            .insert(VARY, HeaderValue::from_static("Accept-Encoding"));
        return http::Response::from_parts(parts, Body::from_bytes(body_bytes));
    }

    // FIX: `compress_bytes` always returns `Vec<u8>` (either compressed or
    // a copy of the original bytes as Vec).  Previously the fallback arm
    // returned `body_bytes` (type `Bytes`) while the Ok arm returned
    // `Vec<u8>`, causing a type mismatch.  Now both arms produce `Vec<u8>`.
    let compressed: Vec<u8> = match compress_bytes(&body_bytes, encoding).await {
        Ok(b) => b,
        Err(_) => body_bytes.to_vec(), // FIX: .to_vec() converts Bytes → Vec<u8>
    };

    parts.headers.insert(
        CONTENT_ENCODING,
        HeaderValue::from_static(encoding.header_value()),
    );
    parts
        .headers
        .insert(VARY, HeaderValue::from_static("Accept-Encoding"));
    // Remove Content-Length since compressed size differs.
    parts.headers.remove(CONTENT_LENGTH);

    let final_body = Body::from_bytes(Bytes::from(compressed));
    http::Response::from_parts(parts, final_body)
}

/// Compress `data` using the given encoding. Returns `Vec<u8>`.
async fn compress_bytes(data: &[u8], encoding: Encoding) -> std::io::Result<Vec<u8>> {
    // `std::io::Cursor<&[u8]>` is `Unpin` and satisfies `AsyncRead`.
    let cursor = std::io::Cursor::new(data);
    match encoding {
        Encoding::Gzip => {
            let mut enc = GzipEncoder::new(cursor);
            let mut out = Vec::new();
            enc.read_to_end(&mut out).await?;
            Ok(out)
        }
        Encoding::Br => {
            let mut enc = BrotliEncoder::new(cursor);
            let mut out = Vec::new();
            enc.read_to_end(&mut out).await?;
            Ok(out)
        }
        Encoding::Zstd => {
            let mut enc = ZstdEncoder::new(cursor);
            let mut out = Vec::new();
            enc.read_to_end(&mut out).await?;
            Ok(out)
        }
        Encoding::Deflate => {
            let mut enc = DeflateEncoder::new(cursor);
            let mut out = Vec::new();
            enc.read_to_end(&mut out).await?;
            Ok(out)
        }
    }
}

fn should_compress(headers: &http::HeaderMap) -> bool {
    let ct = match headers
        .get(http::header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
    {
        Some(ct) => ct,
        None => return false,
    };
    // Compress text, JSON, XML, etc. Skip already-compressed formats.
    ct.starts_with("text/")
        || ct.contains("json")
        || ct.contains("xml")
        || ct.contains("javascript")
        || ct.contains("css")
        || ct.starts_with("application/wasm")
        || ct.starts_with("image/svg")
}

// ── DecompressionLayer ───────────────────────────────────────────────────────

/// Tower layer that decompresses request bodies.
#[derive(Debug, Clone, Default)]
pub struct DecompressionLayer;

impl DecompressionLayer {
    /// Create a new `DecompressionLayer`.
    pub fn new() -> Self {
        Self
    }
}

impl<S> Layer<S> for DecompressionLayer {
    type Service = DecompressionService<S>;

    fn layer(&self, inner: S) -> Self::Service {
        DecompressionService { inner }
    }
}

/// Tower service produced by [`DecompressionLayer`].
#[derive(Clone)]
pub struct DecompressionService<S> {
    inner: S,
}

impl<S> Service<Request> for DecompressionService<S>
where
    S: Service<Request, Response = Response, Error = Infallible> + Clone + Send + 'static,
    S::Future: Send + 'static,
{
    type Response = Response;
    type Error = Infallible;
    type Future = Pin<Box<dyn Future<Output = Result<Response, Infallible>> + Send + 'static>>;

    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.inner.poll_ready(cx)
    }

    fn call(&mut self, req: Request) -> Self::Future {
        let encoding = req
            .headers()
            .get(CONTENT_ENCODING)
            .and_then(|v| v.to_str().ok())
            .map(|s| s.to_lowercase());

        let cloned = self.inner.clone();
        let mut inner = std::mem::replace(&mut self.inner, cloned);

        Box::pin(async move {
            let encoding = match encoding {
                Some(e) => e,
                None => return inner.call(req).await,
            };

            let (mut parts, body) = req.into_request_parts();
            let body_bytes: Bytes = match body.to_bytes().await {
                Ok(b) => b,
                Err(_) => {
                    return Ok(http::Response::builder()
                        .status(StatusCode::BAD_REQUEST)
                        .body(Body::from("Failed to read compressed body"))
                        .unwrap());
                }
            };

            let cursor = std::io::Cursor::new(body_bytes.as_ref());

            // FIX: decompressed is always `Option<Vec<u8>>` — both arms produce
            // the same type so the `match` unifies without issue.
            let decompressed: Option<Vec<u8>> = match encoding.as_str() {
                "gzip" => {
                    let mut dec = GzipDecoder::new(cursor);
                    let mut out = Vec::new();
                    dec.read_to_end(&mut out).await.ok().map(|_| out)
                }
                "br" | "brotli" => {
                    let mut dec = BrotliDecoder::new(cursor);
                    let mut out = Vec::new();
                    dec.read_to_end(&mut out).await.ok().map(|_| out)
                }
                "zstd" => {
                    let mut dec = ZstdDecoder::new(cursor);
                    let mut out = Vec::new();
                    dec.read_to_end(&mut out).await.ok().map(|_| out)
                }
                "deflate" => {
                    let mut dec = DeflateDecoder::new(cursor);
                    let mut out = Vec::new();
                    dec.read_to_end(&mut out).await.ok().map(|_| out)
                }
                _ => None,
            };

            let new_body = match decompressed {
                Some(data) => {
                    // The body is now decoded: drop Content-Encoding and fix
                    // Content-Length, which still described the compressed
                    // wire size and would defeat downstream size checks.
                    parts.headers_mut().remove(CONTENT_ENCODING);
                    parts.headers_mut().insert(
                        CONTENT_LENGTH,
                        HeaderValue::from_str(&data.len().to_string()).expect("valid CL"),
                    );
                    Body::from_bytes(Bytes::from(data))
                }
                None => Body::from_bytes(body_bytes), // pass through as-is
            };

            let req = Request::from_request_parts(parts, new_body);
            inner.call(req).await
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Inner service that reports the Content-Length header it received and
    /// the actual body length, so the test can compare them.
    #[derive(Clone)]
    struct ReportService;

    impl Service<Request> for ReportService {
        type Response = Response;
        type Error = Infallible;
        type Future = Pin<Box<dyn Future<Output = Result<Response, Infallible>> + Send + 'static>>;

        fn poll_ready(&mut self, _: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
            Poll::Ready(Ok(()))
        }

        fn call(&mut self, req: Request) -> Self::Future {
            Box::pin(async move {
                let cl = req
                    .headers()
                    .get(CONTENT_LENGTH)
                    .and_then(|v| v.to_str().ok())
                    .unwrap_or("absent")
                    .to_string();
                let body = Body::to_string(req.into_body()).await.unwrap_or_default();
                let report = format!("cl={cl}|len={}", body.len());
                Ok(http::Response::builder()
                    .status(StatusCode::OK)
                    .body(Body::from(report))
                    .unwrap())
            })
        }
    }

    async fn gzip_compress(data: &[u8]) -> Bytes {
        let mut encoder = GzipEncoder::new(std::io::Cursor::new(data));
        let mut out = Vec::new();
        tokio::io::copy(&mut encoder, &mut out).await.unwrap();
        Bytes::from(out)
    }

    #[tokio::test]
    async fn decompression_fixes_content_length_to_decoded_size() {
        let original = b"hello world, this payload is much larger once decompressed!";
        let compressed = gzip_compress(original).await;
        assert!(compressed.len() != original.len());

        let mut svc = DecompressionLayer::new().layer(ReportService);
        let res = svc
            .call(Request::new(
                http::Request::builder()
                    .method(http::Method::POST)
                    .uri("/")
                    .header(CONTENT_ENCODING, "gzip")
                    .header(CONTENT_LENGTH, http::HeaderValue::from(compressed.len()))
                    .body(Body::from_bytes(compressed))
                    .unwrap(),
            ))
            .await
            .unwrap();

        let text = Body::to_string(res.into_body()).await.unwrap();
        // The stale compressed length must not survive; both views of size
        // must agree on the decoded payload.
        assert_eq!(
            text,
            format!("cl={}|len={}", original.len(), original.len())
        );
    }
}
