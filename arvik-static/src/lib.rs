//! Static file and embedded asset serving for Arvik.
//!
//! `ServeDir` and `ServeFile` serve runtime filesystem assets. When the
//! `embed` feature is enabled, `EmbeddedFileService` serves `rust-embed`
//! assets from the binary with the same cache, range, and content negotiation
//! behavior.

use std::convert::Infallible;
use std::future::Future;
#[cfg(feature = "embed")]
use std::marker::PhantomData;
use std::path::{Component, Path, PathBuf};
use std::pin::Pin;
use std::sync::Arc;
use std::task::{Context, Poll};
#[cfg(feature = "embed")]
use std::time::Duration;
use std::time::{SystemTime, UNIX_EPOCH};

use arvik_core::{Body, OriginalUri, Request, Response, ResponseBuilder};
#[cfg(feature = "embed")]
use bytes::Bytes;
use http::{HeaderValue, Method, StatusCode, Uri, header};
use percent_encoding::percent_decode_str;
use tower_service::Service;

const DEFAULT_CHUNK_SIZE: usize = 64 * 1024;
const INDEX_FILE: &str = "index.html";

type BoxFutureResponse = Pin<Box<dyn Future<Output = Result<Response, Infallible>> + Send>>;

/// Serve a directory tree from the filesystem.
#[cfg(feature = "fs")]
#[derive(Clone)]
pub struct ServeDir {
    root: Arc<PathBuf>,
    fallback: Option<BoxCloneService>,
    precompressed_gzip: bool,
    precompressed_br: bool,
    call_fallback_on_method_not_allowed: bool,
    append_index_html_on_directories: bool,
    directory_listing: bool,
    chunk_size: usize,
    cache_control: Option<HeaderValue>,
}

#[cfg(feature = "fs")]
impl ServeDir {
    /// Create a directory-serving service rooted at `path`.
    pub fn new(path: impl Into<PathBuf>) -> Self {
        Self {
            root: Arc::new(path.into()),
            fallback: None,
            precompressed_gzip: false,
            precompressed_br: false,
            call_fallback_on_method_not_allowed: false,
            append_index_html_on_directories: true,
            directory_listing: false,
            chunk_size: DEFAULT_CHUNK_SIZE,
            cache_control: None,
        }
    }

    /// Set a service called when a file is not found.
    pub fn not_found_service<S>(mut self, service: S) -> Self
    where
        S: Service<Request, Response = Response, Error = Infallible>
            + Clone
            + Send
            + Sync
            + 'static,
        S::Future: Send + 'static,
    {
        self.fallback = Some(BoxCloneService::new(service));
        self
    }

    /// Enable serving `.gz` variants when the client accepts gzip.
    pub fn precompressed_gzip(mut self) -> Self {
        self.precompressed_gzip = true;
        self
    }

    /// Enable serving `.br` variants when the client accepts Brotli.
    pub fn precompressed_br(mut self) -> Self {
        self.precompressed_br = true;
        self
    }

    /// Route method-not-allowed requests to the fallback service.
    pub fn call_fallback_on_method_not_allowed(mut self, call_fallback: bool) -> Self {
        self.call_fallback_on_method_not_allowed = call_fallback;
        self
    }

    /// Control whether directories try `index.html`.
    pub fn append_index_html_on_directories(mut self, append: bool) -> Self {
        self.append_index_html_on_directories = append;
        self
    }

    /// Enable simple HTML directory listings.
    pub fn directory_listing(mut self, enabled: bool) -> Self {
        self.directory_listing = enabled;
        self
    }

    /// Set the filesystem read chunk size. Zero keeps the default.
    pub fn with_buf_chunk_size(mut self, chunk_size: usize) -> Self {
        if chunk_size > 0 {
            self.chunk_size = chunk_size;
        }
        self
    }

    /// Set a `Cache-Control` header for successful and conditional responses.
    pub fn cache_control<V>(mut self, value: V) -> Self
    where
        V: TryInto<HeaderValue>,
        V::Error: std::fmt::Debug,
    {
        self.cache_control = Some(value.try_into().expect("valid Cache-Control header value"));
        self
    }

    async fn handle(self, req: Request) -> Response {
        if !is_get_or_head(req.method()) {
            return if self.call_fallback_on_method_not_allowed {
                self.call_fallback(req).await
            } else {
                method_not_allowed()
            };
        }

        let head = req.method() == Method::HEAD;
        let relative = match relative_path(req.uri().path()) {
            Ok(path) => path,
            Err(()) => return self.call_fallback(req).await,
        };

        let candidate = self.root.join(&relative.fs_path);
        let metadata = match tokio::fs::metadata(&candidate).await {
            Ok(metadata) => metadata,
            Err(_) => return self.call_fallback(req).await,
        };

        if metadata.is_dir() {
            return self.handle_directory(req, relative, candidate).await;
        }

        let accepts_br = self.precompressed_br && accepts_encoding(&req, "br");
        let accepts_gzip = self.precompressed_gzip && accepts_encoding(&req, "gzip");

        match self
            .select_file(candidate, &relative.asset_path, accepts_br, accepts_gzip)
            .await
        {
            Ok(asset) => {
                serve_fs_asset(asset, req, head, self.chunk_size, self.cache_control).await
            }
            Err(()) => self.call_fallback(req).await,
        }
    }

    async fn handle_directory(
        self,
        req: Request,
        relative: RelativePath,
        dir: PathBuf,
    ) -> Response {
        if !req.uri().path().ends_with('/')
            && (self.append_index_html_on_directories || self.directory_listing)
        {
            return redirect_to_slash(&req);
        }

        if self.append_index_html_on_directories {
            let index_path = dir.join(INDEX_FILE);
            if tokio::fs::metadata(&index_path)
                .await
                .map(|m| m.is_file())
                .unwrap_or(false)
            {
                let index_asset_path = join_asset_path(&relative.asset_path, INDEX_FILE);
                let accepts_br = self.precompressed_br && accepts_encoding(&req, "br");
                let accepts_gzip = self.precompressed_gzip && accepts_encoding(&req, "gzip");

                return match self
                    .select_file(index_path, &index_asset_path, accepts_br, accepts_gzip)
                    .await
                {
                    Ok(asset) => {
                        serve_fs_asset(
                            asset,
                            req,
                            false,
                            self.chunk_size,
                            self.cache_control.clone(),
                        )
                        .await
                    }
                    Err(()) => self.call_fallback(req).await,
                };
            }
        }

        if self.directory_listing {
            return directory_listing_response(&dir, req.uri().path()).await;
        }

        self.call_fallback(req).await
    }

    async fn select_file(
        &self,
        path: PathBuf,
        asset_path: &str,
        accepts_br: bool,
        accepts_gzip: bool,
    ) -> Result<FsAsset, ()> {
        let content_type = content_type(asset_path);

        if accepts_br {
            let br_path = append_suffix(&path, ".br");
            if let Ok(metadata) = tokio::fs::metadata(&br_path).await
                && metadata.is_file()
            {
                return Ok(FsAsset::new(
                    br_path,
                    metadata,
                    content_type,
                    Some(HeaderValue::from_static("br")),
                    self.precompressed_gzip || self.precompressed_br,
                ));
            }
        }

        if accepts_gzip {
            let gz_path = append_suffix(&path, ".gz");
            if let Ok(metadata) = tokio::fs::metadata(&gz_path).await
                && metadata.is_file()
            {
                return Ok(FsAsset::new(
                    gz_path,
                    metadata,
                    content_type,
                    Some(HeaderValue::from_static("gzip")),
                    self.precompressed_gzip || self.precompressed_br,
                ));
            }
        }

        let metadata = tokio::fs::metadata(&path).await.map_err(|_| ())?;
        if !metadata.is_file() {
            return Err(());
        }
        Ok(FsAsset::new(
            path,
            metadata,
            content_type,
            None,
            self.precompressed_gzip || self.precompressed_br,
        ))
    }

    async fn call_fallback(&self, req: Request) -> Response {
        match &self.fallback {
            Some(service) => call_boxed(service.clone(), req).await,
            None => not_found(),
        }
    }
}

#[cfg(feature = "fs")]
impl Service<Request> for ServeDir {
    type Response = Response;
    type Error = Infallible;
    type Future = BoxFutureResponse;

    fn poll_ready(&mut self, _cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        Poll::Ready(Ok(()))
    }

    fn call(&mut self, req: Request) -> Self::Future {
        let service = self.clone();
        Box::pin(async move { Ok(service.handle(req).await) })
    }
}

/// Serve a single file from the filesystem.
#[cfg(feature = "fs")]
#[derive(Clone)]
pub struct ServeFile {
    path: Arc<PathBuf>,
    mime: HeaderValue,
    precompressed_gzip: bool,
    precompressed_br: bool,
    chunk_size: usize,
    cache_control: Option<HeaderValue>,
}

#[cfg(feature = "fs")]
impl ServeFile {
    /// Create a file-serving service with MIME type detected from extension.
    pub fn new(path: impl Into<PathBuf>) -> Self {
        let path = path.into();
        let mime = content_type(path.to_string_lossy().as_ref());
        Self {
            path: Arc::new(path),
            mime,
            precompressed_gzip: false,
            precompressed_br: false,
            chunk_size: DEFAULT_CHUNK_SIZE,
            cache_control: None,
        }
    }

    /// Create a file-serving service with a specific MIME type.
    pub fn new_with_mime(path: impl Into<PathBuf>, mime: &mime::Mime) -> Self {
        Self {
            path: Arc::new(path.into()),
            mime: HeaderValue::from_str(mime.as_ref()).expect("valid MIME header value"),
            precompressed_gzip: false,
            precompressed_br: false,
            chunk_size: DEFAULT_CHUNK_SIZE,
            cache_control: None,
        }
    }

    /// Enable serving a `.gz` sibling when the client accepts gzip.
    pub fn precompressed_gzip(mut self) -> Self {
        self.precompressed_gzip = true;
        self
    }

    /// Enable serving a `.br` sibling when the client accepts Brotli.
    pub fn precompressed_br(mut self) -> Self {
        self.precompressed_br = true;
        self
    }

    /// Set the filesystem read chunk size. Zero keeps the default.
    pub fn with_buf_chunk_size(mut self, chunk_size: usize) -> Self {
        if chunk_size > 0 {
            self.chunk_size = chunk_size;
        }
        self
    }

    /// Set a `Cache-Control` header for successful and conditional responses.
    pub fn cache_control<V>(mut self, value: V) -> Self
    where
        V: TryInto<HeaderValue>,
        V::Error: std::fmt::Debug,
    {
        self.cache_control = Some(value.try_into().expect("valid Cache-Control header value"));
        self
    }

    async fn handle(self, req: Request) -> Response {
        if !is_get_or_head(req.method()) {
            return method_not_allowed();
        }

        let head = req.method() == Method::HEAD;
        let accepts_br = self.precompressed_br && accepts_encoding(&req, "br");
        let accepts_gzip = self.precompressed_gzip && accepts_encoding(&req, "gzip");

        match self.select_file(accepts_br, accepts_gzip).await {
            Ok(asset) => {
                serve_fs_asset(asset, req, head, self.chunk_size, self.cache_control).await
            }
            Err(()) => not_found(),
        }
    }

    async fn select_file(&self, accepts_br: bool, accepts_gzip: bool) -> Result<FsAsset, ()> {
        if accepts_br {
            let br_path = append_suffix(&self.path, ".br");
            if let Ok(metadata) = tokio::fs::metadata(&br_path).await
                && metadata.is_file()
            {
                return Ok(FsAsset::new(
                    br_path,
                    metadata,
                    self.mime.clone(),
                    Some(HeaderValue::from_static("br")),
                    self.precompressed_gzip || self.precompressed_br,
                ));
            }
        }

        if accepts_gzip {
            let gz_path = append_suffix(&self.path, ".gz");
            if let Ok(metadata) = tokio::fs::metadata(&gz_path).await
                && metadata.is_file()
            {
                return Ok(FsAsset::new(
                    gz_path,
                    metadata,
                    self.mime.clone(),
                    Some(HeaderValue::from_static("gzip")),
                    self.precompressed_gzip || self.precompressed_br,
                ));
            }
        }

        let metadata = tokio::fs::metadata(&*self.path).await.map_err(|_| ())?;
        if !metadata.is_file() {
            return Err(());
        }
        Ok(FsAsset::new(
            (*self.path).clone(),
            metadata,
            self.mime.clone(),
            None,
            self.precompressed_gzip || self.precompressed_br,
        ))
    }
}

#[cfg(feature = "fs")]
impl Service<Request> for ServeFile {
    type Response = Response;
    type Error = Infallible;
    type Future = BoxFutureResponse;

    fn poll_ready(&mut self, _cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        Poll::Ready(Ok(()))
    }

    fn call(&mut self, req: Request) -> Self::Future {
        let service = self.clone();
        Box::pin(async move { Ok(service.handle(req).await) })
    }
}

/// Serve assets embedded with `rust-embed`.
#[cfg(feature = "embed")]
pub struct EmbeddedFileService<A> {
    fallback: Option<BoxCloneService>,
    precompressed_gzip: bool,
    precompressed_br: bool,
    call_fallback_on_method_not_allowed: bool,
    append_index_html_on_directories: bool,
    directory_listing: bool,
    cache_control: Option<HeaderValue>,
    _marker: PhantomData<fn() -> A>,
}

#[cfg(feature = "embed")]
impl<A> Clone for EmbeddedFileService<A> {
    fn clone(&self) -> Self {
        Self {
            fallback: self.fallback.clone(),
            precompressed_gzip: self.precompressed_gzip,
            precompressed_br: self.precompressed_br,
            call_fallback_on_method_not_allowed: self.call_fallback_on_method_not_allowed,
            append_index_html_on_directories: self.append_index_html_on_directories,
            directory_listing: self.directory_listing,
            cache_control: self.cache_control.clone(),
            _marker: PhantomData,
        }
    }
}

#[cfg(feature = "embed")]
impl<A> EmbeddedFileService<A>
where
    A: rust_embed::RustEmbed + Send + Sync + 'static,
{
    /// Create an embedded asset service.
    pub fn new() -> Self {
        Self {
            fallback: None,
            precompressed_gzip: false,
            precompressed_br: false,
            call_fallback_on_method_not_allowed: false,
            append_index_html_on_directories: true,
            directory_listing: false,
            cache_control: None,
            _marker: PhantomData,
        }
    }

    /// Set a service called when an embedded asset is not found.
    pub fn not_found_service<S>(mut self, service: S) -> Self
    where
        S: Service<Request, Response = Response, Error = Infallible>
            + Clone
            + Send
            + Sync
            + 'static,
        S::Future: Send + 'static,
    {
        self.fallback = Some(BoxCloneService::new(service));
        self
    }

    /// Enable serving embedded `.gz` variants when the client accepts gzip.
    pub fn precompressed_gzip(mut self) -> Self {
        self.precompressed_gzip = true;
        self
    }

    /// Enable serving embedded `.br` variants when the client accepts Brotli.
    pub fn precompressed_br(mut self) -> Self {
        self.precompressed_br = true;
        self
    }

    /// Route method-not-allowed requests to the fallback service.
    pub fn call_fallback_on_method_not_allowed(mut self, call_fallback: bool) -> Self {
        self.call_fallback_on_method_not_allowed = call_fallback;
        self
    }

    /// Control whether directories try `index.html`.
    pub fn append_index_html_on_directories(mut self, append: bool) -> Self {
        self.append_index_html_on_directories = append;
        self
    }

    /// Enable simple HTML directory listings.
    pub fn directory_listing(mut self, enabled: bool) -> Self {
        self.directory_listing = enabled;
        self
    }

    /// Set a `Cache-Control` header for successful and conditional responses.
    pub fn cache_control<V>(mut self, value: V) -> Self
    where
        V: TryInto<HeaderValue>,
        V::Error: std::fmt::Debug,
    {
        self.cache_control = Some(value.try_into().expect("valid Cache-Control header value"));
        self
    }

    async fn handle(self, req: Request) -> Response {
        if !is_get_or_head(req.method()) {
            return if self.call_fallback_on_method_not_allowed {
                self.call_fallback(req).await
            } else {
                method_not_allowed()
            };
        }

        let head = req.method() == Method::HEAD;
        let relative = match relative_path(req.uri().path()) {
            Ok(path) => path,
            Err(()) => return self.call_fallback(req).await,
        };

        if self.embedded_dir_exists(&relative.asset_path) {
            return self.handle_directory(req, relative).await;
        }

        match self.select_file(&relative.asset_path, &req) {
            Some(asset) => serve_embedded_asset(asset, req, head, self.cache_control),
            None => self.call_fallback(req).await,
        }
    }

    async fn handle_directory(self, req: Request, relative: RelativePath) -> Response {
        if !req.uri().path().ends_with('/')
            && (self.append_index_html_on_directories || self.directory_listing)
        {
            return redirect_to_slash(&req);
        }

        if self.append_index_html_on_directories {
            let index_path = join_asset_path(&relative.asset_path, INDEX_FILE);
            if let Some(asset) = self.select_file(&index_path, &req) {
                return serve_embedded_asset(asset, req, false, self.cache_control.clone());
            }
        }

        if self.directory_listing {
            return embedded_directory_listing::<A>(&relative.asset_path, req.uri().path());
        }

        self.call_fallback(req).await
    }

    fn select_file(&self, asset_path: &str, req: &Request) -> Option<EmbeddedAsset> {
        let content_type = content_type(asset_path);

        if self.precompressed_br && accepts_encoding(req, "br") {
            let br_path = format!("{asset_path}.br");
            if let Some(file) = A::get(&br_path) {
                return Some(EmbeddedAsset::new(
                    file,
                    content_type.clone(),
                    Some(HeaderValue::from_static("br")),
                    self.precompressed_gzip || self.precompressed_br,
                ));
            }
        }

        if self.precompressed_gzip && accepts_encoding(req, "gzip") {
            let gz_path = format!("{asset_path}.gz");
            if let Some(file) = A::get(&gz_path) {
                return Some(EmbeddedAsset::new(
                    file,
                    content_type.clone(),
                    Some(HeaderValue::from_static("gzip")),
                    self.precompressed_gzip || self.precompressed_br,
                ));
            }
        }

        A::get(asset_path).map(|file| {
            EmbeddedAsset::new(
                file,
                content_type,
                None,
                self.precompressed_gzip || self.precompressed_br,
            )
        })
    }

    fn embedded_dir_exists(&self, asset_path: &str) -> bool {
        let prefix = if asset_path.is_empty() {
            String::new()
        } else {
            format!("{}/", asset_path.trim_end_matches('/'))
        };

        A::iter().any(|path| path.starts_with(&prefix) && path.len() > prefix.len())
    }

    async fn call_fallback(&self, req: Request) -> Response {
        match &self.fallback {
            Some(service) => call_boxed(service.clone(), req).await,
            None => not_found(),
        }
    }
}

#[cfg(feature = "embed")]
impl<A> Default for EmbeddedFileService<A>
where
    A: rust_embed::RustEmbed + Send + Sync + 'static,
{
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(feature = "embed")]
impl<A> Service<Request> for EmbeddedFileService<A>
where
    A: rust_embed::RustEmbed + Send + Sync + 'static,
{
    type Response = Response;
    type Error = Infallible;
    type Future = BoxFutureResponse;

    fn poll_ready(&mut self, _cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        Poll::Ready(Ok(()))
    }

    fn call(&mut self, req: Request) -> Self::Future {
        let service = self.clone();
        Box::pin(async move { Ok(service.handle(req).await) })
    }
}

#[cfg(feature = "embed")]
pub use rust_embed::{Embed, EmbeddedFile, Filenames, Metadata, RustEmbed, utils};

struct BoxCloneService(Box<dyn ErasedService>);

trait ErasedService: Send + Sync {
    fn call_erased(&mut self, req: Request) -> BoxFutureResponse;
    fn clone_box(&self) -> Box<dyn ErasedService>;
}

impl<S> ErasedService for S
where
    S: Service<Request, Response = Response, Error = Infallible> + Clone + Send + Sync + 'static,
    S::Future: Send + 'static,
{
    fn call_erased(&mut self, req: Request) -> BoxFutureResponse {
        Box::pin(Service::call(self, req))
    }

    fn clone_box(&self) -> Box<dyn ErasedService> {
        Box::new(self.clone())
    }
}

impl BoxCloneService {
    fn new<S>(service: S) -> Self
    where
        S: Service<Request, Response = Response, Error = Infallible>
            + Clone
            + Send
            + Sync
            + 'static,
        S::Future: Send + 'static,
    {
        Self(Box::new(service))
    }
}

impl Clone for BoxCloneService {
    fn clone(&self) -> Self {
        Self(self.0.clone_box())
    }
}

async fn call_boxed(mut service: BoxCloneService, req: Request) -> Response {
    service
        .0
        .call_erased(req)
        .await
        .unwrap_or_else(|infallible| match infallible {})
}

#[cfg(feature = "fs")]
struct FsAsset {
    path: PathBuf,
    len: u64,
    modified: Option<SystemTime>,
    etag: HeaderValue,
    content_type: HeaderValue,
    content_encoding: Option<HeaderValue>,
    vary_accept_encoding: bool,
}

#[cfg(feature = "fs")]
impl FsAsset {
    fn new(
        path: PathBuf,
        metadata: std::fs::Metadata,
        content_type: HeaderValue,
        content_encoding: Option<HeaderValue>,
        vary_accept_encoding: bool,
    ) -> Self {
        let modified = metadata.modified().ok();
        let etag = fs_etag(metadata.len(), modified);
        Self {
            path,
            len: metadata.len(),
            modified,
            etag,
            content_type,
            content_encoding,
            vary_accept_encoding,
        }
    }
}

#[cfg(feature = "fs")]
async fn serve_fs_asset(
    asset: FsAsset,
    req: Request,
    head: bool,
    chunk_size: usize,
    cache_control: Option<HeaderValue>,
) -> Response {
    if is_not_modified(&req, &asset.etag, asset.modified) {
        return not_modified(
            &asset.etag,
            asset.modified,
            cache_control,
            asset.vary_accept_encoding,
        );
    }

    let range = match parse_range(req.headers().get(header::RANGE), asset.len) {
        RangeDecision::Full => None,
        RangeDecision::Partial(range) => Some(range),
        RangeDecision::Unsatisfiable => return range_not_satisfiable(asset.len),
    };

    let body_len = range
        .as_ref()
        .map(|range| range.end - range.start + 1)
        .unwrap_or(asset.len);

    let mut builder = ResponseBuilder::new()
        .status(if range.is_some() {
            StatusCode::PARTIAL_CONTENT
        } else {
            StatusCode::OK
        })
        .header(header::CONTENT_TYPE, asset.content_type)
        .header(header::CONTENT_LENGTH, body_len.to_string())
        .header(header::ACCEPT_RANGES, "bytes")
        .header(header::ETAG, asset.etag.clone());

    if let Some(modified) = asset.modified {
        builder = builder.header(header::LAST_MODIFIED, httpdate::fmt_http_date(modified));
    }
    if let Some(cache_control) = cache_control {
        builder = builder.header(header::CACHE_CONTROL, cache_control);
    }
    if let Some(content_encoding) = asset.content_encoding {
        builder = builder.header(header::CONTENT_ENCODING, content_encoding);
    }
    if asset.vary_accept_encoding {
        builder = builder.header(header::VARY, "Accept-Encoding");
    }
    if let Some(range) = &range {
        builder = builder.header(
            header::CONTENT_RANGE,
            format!("bytes {}-{}/{}", range.start, range.end, asset.len),
        );
    }

    if head {
        return builder.empty();
    }

    let file = match tokio::fs::File::open(&asset.path).await {
        Ok(file) => file,
        Err(_) => return not_found(),
    };

    let body: Body = match range {
        Some(range) => ranged_file_body(file, range, chunk_size).await,
        None => file_body(file, chunk_size),
    };

    builder.body(body)
}

#[cfg(feature = "fs")]
fn file_body(file: tokio::fs::File, chunk_size: usize) -> Body {
    let stream = tokio_util::io::ReaderStream::with_capacity(file, chunk_size);
    Body::from_stream(stream)
}

#[cfg(feature = "fs")]
async fn ranged_file_body(mut file: tokio::fs::File, range: ByteRange, chunk_size: usize) -> Body {
    use tokio::io::{AsyncReadExt as _, AsyncSeekExt as _};

    if file
        .seek(std::io::SeekFrom::Start(range.start))
        .await
        .is_err()
    {
        return Body::empty();
    }
    let stream = tokio_util::io::ReaderStream::with_capacity(
        file.take(range.end - range.start + 1),
        chunk_size,
    );
    Body::from_stream(stream)
}

#[cfg(feature = "embed")]
struct EmbeddedAsset {
    bytes: Bytes,
    len: u64,
    modified: Option<SystemTime>,
    etag: HeaderValue,
    content_type: HeaderValue,
    content_encoding: Option<HeaderValue>,
    vary_accept_encoding: bool,
}

#[cfg(feature = "embed")]
impl EmbeddedAsset {
    fn new(
        file: rust_embed::EmbeddedFile,
        content_type: HeaderValue,
        content_encoding: Option<HeaderValue>,
        vary_accept_encoding: bool,
    ) -> Self {
        let len = file.data.len() as u64;
        let etag = embedded_etag(&file.metadata.sha256_hash());
        let modified = file
            .metadata
            .last_modified()
            .map(|seconds| UNIX_EPOCH + Duration::from_secs(seconds));
        let bytes = match file.data {
            std::borrow::Cow::Borrowed(bytes) => Bytes::from_static(bytes),
            std::borrow::Cow::Owned(bytes) => Bytes::from(bytes),
        };

        Self {
            bytes,
            len,
            modified,
            etag,
            content_type,
            content_encoding,
            vary_accept_encoding,
        }
    }
}

#[cfg(feature = "embed")]
fn serve_embedded_asset(
    asset: EmbeddedAsset,
    req: Request,
    head: bool,
    cache_control: Option<HeaderValue>,
) -> Response {
    if is_not_modified(&req, &asset.etag, asset.modified) {
        return not_modified(
            &asset.etag,
            asset.modified,
            cache_control,
            asset.vary_accept_encoding,
        );
    }

    let range = match parse_range(req.headers().get(header::RANGE), asset.len) {
        RangeDecision::Full => None,
        RangeDecision::Partial(range) => Some(range),
        RangeDecision::Unsatisfiable => return range_not_satisfiable(asset.len),
    };

    let body_len = range
        .as_ref()
        .map(|range| range.end - range.start + 1)
        .unwrap_or(asset.len);

    let mut builder = ResponseBuilder::new()
        .status(if range.is_some() {
            StatusCode::PARTIAL_CONTENT
        } else {
            StatusCode::OK
        })
        .header(header::CONTENT_TYPE, asset.content_type)
        .header(header::CONTENT_LENGTH, body_len.to_string())
        .header(header::ACCEPT_RANGES, "bytes")
        .header(header::ETAG, asset.etag.clone());

    if let Some(modified) = asset.modified {
        builder = builder.header(header::LAST_MODIFIED, httpdate::fmt_http_date(modified));
    }
    if let Some(cache_control) = cache_control {
        builder = builder.header(header::CACHE_CONTROL, cache_control);
    }
    if let Some(content_encoding) = asset.content_encoding {
        builder = builder.header(header::CONTENT_ENCODING, content_encoding);
    }
    if asset.vary_accept_encoding {
        builder = builder.header(header::VARY, "Accept-Encoding");
    }
    if let Some(range) = &range {
        builder = builder.header(
            header::CONTENT_RANGE,
            format!("bytes {}-{}/{}", range.start, range.end, asset.len),
        );
    }

    if head {
        return builder.empty();
    }

    let body: Body = match range {
        Some(range) => asset
            .bytes
            .slice(range.start as usize..=range.end as usize)
            .into(),
        None => asset.bytes.into(),
    };
    builder.body(body)
}

#[derive(Debug)]
struct RelativePath {
    fs_path: PathBuf,
    asset_path: String,
}

fn relative_path(path: &str) -> Result<RelativePath, ()> {
    if !valid_percent_encoding(path) {
        return Err(());
    }

    let decoded = percent_decode_str(path.trim_start_matches('/'))
        .decode_utf8()
        .map_err(|_| ())?;

    if decoded.contains('\\') || decoded.contains('\0') {
        return Err(());
    }

    let mut fs_path = PathBuf::new();
    let mut asset_parts = Vec::new();

    for component in Path::new(decoded.as_ref()).components() {
        match component {
            Component::Normal(part) => {
                fs_path.push(part);
                asset_parts.push(part.to_string_lossy().into_owned());
            }
            Component::CurDir => {}
            Component::ParentDir | Component::RootDir | Component::Prefix(_) => return Err(()),
        }
    }

    Ok(RelativePath {
        fs_path,
        asset_path: asset_parts.join("/"),
    })
}

fn valid_percent_encoding(path: &str) -> bool {
    let bytes = path.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' {
            if i + 2 >= bytes.len()
                || !bytes[i + 1].is_ascii_hexdigit()
                || !bytes[i + 2].is_ascii_hexdigit()
            {
                return false;
            }
            i += 3;
        } else {
            i += 1;
        }
    }
    true
}

fn is_get_or_head(method: &Method) -> bool {
    *method == Method::GET || *method == Method::HEAD
}

fn content_type(path: &str) -> HeaderValue {
    HeaderValue::from_str(
        mime_guess::from_path(path)
            .first_or_octet_stream()
            .essence_str(),
    )
    .expect("MIME values are valid headers")
}

fn append_suffix(path: &Path, suffix: &str) -> PathBuf {
    let mut file_name = path
        .file_name()
        .map(|name| name.to_os_string())
        .unwrap_or_default();
    file_name.push(suffix);

    let mut path = path.to_path_buf();
    path.set_file_name(file_name);
    path
}

fn join_asset_path(base: &str, file: &str) -> String {
    if base.is_empty() {
        file.to_string()
    } else {
        format!("{}/{}", base.trim_end_matches('/'), file)
    }
}

fn accepts_encoding(req: &Request, encoding: &str) -> bool {
    req.headers()
        .get(header::ACCEPT_ENCODING)
        .and_then(|value| value.to_str().ok())
        .map(|header| {
            header.split(',').any(|part| {
                let mut pieces = part.trim().split(';');
                let token = pieces.next().unwrap_or("").trim();
                let mut q = 1.0_f32;
                for param in pieces {
                    let param = param.trim();
                    if let Some(value) = param.strip_prefix("q=") {
                        q = value.parse::<f32>().unwrap_or(0.0);
                    }
                }
                q > 0.0 && (token.eq_ignore_ascii_case(encoding) || token == "*")
            })
        })
        .unwrap_or(false)
}

#[derive(Clone, Copy)]
struct ByteRange {
    start: u64,
    end: u64,
}

enum RangeDecision {
    Full,
    Partial(ByteRange),
    Unsatisfiable,
}

fn parse_range(value: Option<&HeaderValue>, len: u64) -> RangeDecision {
    let Some(value) = value.and_then(|value| value.to_str().ok()) else {
        return RangeDecision::Full;
    };

    let Some(spec) = value.strip_prefix("bytes=") else {
        return RangeDecision::Unsatisfiable;
    };
    if spec.contains(',') || spec.is_empty() {
        return RangeDecision::Unsatisfiable;
    }

    let Some((start, end)) = spec.split_once('-') else {
        return RangeDecision::Unsatisfiable;
    };

    if len == 0 {
        return RangeDecision::Unsatisfiable;
    }

    if start.is_empty() {
        let Ok(suffix) = end.parse::<u64>() else {
            return RangeDecision::Unsatisfiable;
        };
        if suffix == 0 {
            return RangeDecision::Unsatisfiable;
        }
        let take = suffix.min(len);
        return RangeDecision::Partial(ByteRange {
            start: len - take,
            end: len - 1,
        });
    }

    let Ok(start) = start.parse::<u64>() else {
        return RangeDecision::Unsatisfiable;
    };
    if start >= len {
        return RangeDecision::Unsatisfiable;
    }

    let end = if end.is_empty() {
        len - 1
    } else {
        let Ok(end) = end.parse::<u64>() else {
            return RangeDecision::Unsatisfiable;
        };
        if end < start {
            return RangeDecision::Unsatisfiable;
        }
        end.min(len - 1)
    };

    RangeDecision::Partial(ByteRange { start, end })
}

fn is_not_modified(req: &Request, etag: &HeaderValue, modified: Option<SystemTime>) -> bool {
    if let Some(value) = req.headers().get(header::IF_NONE_MATCH)
        && let (Ok(header), Ok(etag)) = (value.to_str(), etag.to_str())
    {
        return header
            .split(',')
            .map(str::trim)
            .any(|candidate| candidate == "*" || etag_matches(candidate, etag));
    }

    if let (Some(modified), Some(value)) = (modified, req.headers().get(header::IF_MODIFIED_SINCE))
        && let Ok(value) = value.to_str()
        && let Ok(since) = httpdate::parse_http_date(value)
    {
        return unix_seconds(modified) <= unix_seconds(since);
    }

    false
}

fn etag_matches(candidate: &str, etag: &str) -> bool {
    candidate == etag
        || candidate.strip_prefix("W/") == Some(etag)
        || etag.strip_prefix("W/") == Some(candidate)
}

fn unix_seconds(time: SystemTime) -> u64 {
    time.duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

fn fs_etag(len: u64, modified: Option<SystemTime>) -> HeaderValue {
    let modified = modified
        .and_then(|time| time.duration_since(UNIX_EPOCH).ok())
        .unwrap_or_default();
    HeaderValue::from_str(&format!(
        "W/\"{:x}-{:x}-{:x}\"",
        len,
        modified.as_secs(),
        modified.subsec_nanos()
    ))
    .expect("generated ETag is valid")
}

#[cfg(feature = "embed")]
fn embedded_etag(hash: &[u8; 32]) -> HeaderValue {
    let mut value = String::with_capacity(66);
    value.push('"');
    for byte in hash {
        use std::fmt::Write as _;
        let _ = write!(&mut value, "{byte:02x}");
    }
    value.push('"');
    HeaderValue::from_str(&value).expect("generated ETag is valid")
}

fn not_modified(
    etag: &HeaderValue,
    modified: Option<SystemTime>,
    cache_control: Option<HeaderValue>,
    vary_accept_encoding: bool,
) -> Response {
    let mut builder = ResponseBuilder::new()
        .status(StatusCode::NOT_MODIFIED)
        .header(header::ETAG, etag.clone());
    if let Some(modified) = modified {
        builder = builder.header(header::LAST_MODIFIED, httpdate::fmt_http_date(modified));
    }
    if let Some(cache_control) = cache_control {
        builder = builder.header(header::CACHE_CONTROL, cache_control);
    }
    if vary_accept_encoding {
        builder = builder.header(header::VARY, "Accept-Encoding");
    }
    builder.empty()
}

fn range_not_satisfiable(len: u64) -> Response {
    ResponseBuilder::new()
        .status(StatusCode::RANGE_NOT_SATISFIABLE)
        .header(header::CONTENT_RANGE, format!("bytes */{len}"))
        .empty()
}

fn method_not_allowed() -> Response {
    ResponseBuilder::new()
        .status(StatusCode::METHOD_NOT_ALLOWED)
        .header(header::ALLOW, "GET, HEAD")
        .text("Method Not Allowed")
}

fn not_found() -> Response {
    ResponseBuilder::new()
        .status(StatusCode::NOT_FOUND)
        .header(header::CONTENT_TYPE, "text/plain; charset=utf-8")
        .text("Not Found")
}

fn redirect_to_slash(req: &Request) -> Response {
    let uri = req
        .extensions()
        .get::<OriginalUri>()
        .map(|original| &original.0)
        .unwrap_or_else(|| req.uri());
    let location = slash_location(uri);

    ResponseBuilder::new()
        .status(StatusCode::TEMPORARY_REDIRECT)
        .header(header::LOCATION, location)
        .empty()
}

fn slash_location(uri: &Uri) -> String {
    let path = uri.path();
    let query = uri.query();
    let mut location = if path.ends_with('/') {
        path.to_string()
    } else {
        format!("{path}/")
    };
    if let Some(query) = query {
        location.push('?');
        location.push_str(query);
    }
    location
}

#[cfg(feature = "fs")]
async fn directory_listing_response(dir: &Path, request_path: &str) -> Response {
    let mut entries = Vec::new();
    let mut read_dir = match tokio::fs::read_dir(dir).await {
        Ok(read_dir) => read_dir,
        Err(_) => return not_found(),
    };

    while let Ok(Some(entry)) = read_dir.next_entry().await {
        let name = entry.file_name().to_string_lossy().into_owned();
        let suffix = match entry.file_type().await {
            Ok(file_type) if file_type.is_dir() => "/",
            _ => "",
        };
        entries.push(format!("{name}{suffix}"));
    }

    entries.sort();
    listing_response(request_path, entries)
}

#[cfg(feature = "embed")]
fn embedded_directory_listing<A: rust_embed::RustEmbed>(
    asset_path: &str,
    request_path: &str,
) -> Response {
    use std::collections::BTreeSet;

    let prefix = if asset_path.is_empty() {
        String::new()
    } else {
        format!("{}/", asset_path.trim_end_matches('/'))
    };

    let mut entries = BTreeSet::new();
    for path in A::iter() {
        if !path.starts_with(&prefix) || path.len() == prefix.len() {
            continue;
        }
        let rest = &path[prefix.len()..];
        let Some(first) = rest.split('/').next() else {
            continue;
        };
        let suffix = if rest.contains('/') { "/" } else { "" };
        entries.insert(format!("{first}{suffix}"));
    }

    listing_response(request_path, entries.into_iter().collect())
}

fn listing_response(request_path: &str, entries: Vec<String>) -> Response {
    let mut html = String::new();
    html.push_str("<!doctype html><meta charset=\"utf-8\"><title>Index of ");
    html.push_str(&escape_html(request_path));
    html.push_str("</title><h1>Index of ");
    html.push_str(&escape_html(request_path));
    html.push_str("</h1><ul>");

    for entry in entries {
        html.push_str("<li><a href=\"");
        html.push_str(&escape_html(&entry));
        html.push_str("\">");
        html.push_str(&escape_html(&entry));
        html.push_str("</a></li>");
    }

    html.push_str("</ul>");
    ResponseBuilder::new()
        .header(header::CONTENT_TYPE, "text/html; charset=utf-8")
        .body(html)
}

fn escape_html(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    for ch in value.chars() {
        match ch {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&#39;"),
            _ => out.push(ch),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use http::header;

    fn request(path: &str) -> Request {
        Request::new(
            http::Request::builder()
                .uri(path)
                .body(Body::empty())
                .unwrap(),
        )
    }

    #[cfg(feature = "fs")]
    fn write(path: &Path, body: &[u8]) {
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, body).unwrap();
    }

    #[cfg(feature = "fs")]
    async fn body_text(res: Response) -> String {
        String::from_utf8(res.into_body().to_bytes().await.unwrap().to_vec()).unwrap()
    }

    #[cfg(feature = "fs")]
    #[tokio::test]
    async fn serve_file_body_mime_and_head() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("app.css");
        write(&path, b"body{color:red}");

        let mut service = ServeFile::new(&path);
        let res = service.call(request("/ignored")).await.unwrap();
        assert_eq!(res.status(), StatusCode::OK);
        assert_eq!(res.headers()[header::CONTENT_TYPE], "text/css");
        assert_eq!(body_text(res).await, "body{color:red}");

        let mut req = request("/ignored");
        *req.method_mut() = Method::HEAD;
        let res = service.call(req).await.unwrap();
        assert_eq!(res.status(), StatusCode::OK);
        assert_eq!(res.headers()[header::CONTENT_LENGTH], "15");
        assert!(res.into_body().to_bytes().await.unwrap().is_empty());
    }

    #[cfg(feature = "fs")]
    #[tokio::test]
    async fn serve_dir_index_redirect_listing_and_fallback() {
        let dir = tempfile::tempdir().unwrap();
        write(&dir.path().join("docs/index.html"), b"<h1>docs</h1>");
        write(&dir.path().join("404.html"), b"custom 404");
        write(&dir.path().join("list/a.txt"), b"a");

        let mut service = ServeDir::new(dir.path())
            .not_found_service(ServeFile::new(dir.path().join("404.html")))
            .directory_listing(true);

        let res = service.call(request("/docs")).await.unwrap();
        assert_eq!(res.status(), StatusCode::TEMPORARY_REDIRECT);
        assert_eq!(res.headers()[header::LOCATION], "/docs/");

        let res = service.call(request("/docs/")).await.unwrap();
        assert_eq!(body_text(res).await, "<h1>docs</h1>");

        let res = service.call(request("/list/")).await.unwrap();
        assert!(body_text(res).await.contains("a.txt"));

        let res = service.call(request("/missing")).await.unwrap();
        assert_eq!(res.status(), StatusCode::OK);
        assert_eq!(body_text(res).await, "custom 404");
    }

    #[cfg(feature = "fs")]
    #[tokio::test]
    async fn validators_and_ranges() {
        let dir = tempfile::tempdir().unwrap();
        write(&dir.path().join("file.txt"), b"abcdef");

        let mut service = ServeDir::new(dir.path());
        let res = service.call(request("/file.txt")).await.unwrap();
        let etag = res.headers()[header::ETAG].clone();
        let last_modified = res.headers()[header::LAST_MODIFIED].clone();

        let req = Request::new(
            http::Request::builder()
                .uri("/file.txt")
                .header(header::IF_NONE_MATCH, etag.clone())
                .body(Body::empty())
                .unwrap(),
        );
        assert_eq!(
            service.call(req).await.unwrap().status(),
            StatusCode::NOT_MODIFIED
        );

        let req = Request::new(
            http::Request::builder()
                .uri("/file.txt")
                .header(header::IF_MODIFIED_SINCE, last_modified)
                .body(Body::empty())
                .unwrap(),
        );
        assert_eq!(
            service.call(req).await.unwrap().status(),
            StatusCode::NOT_MODIFIED
        );

        let req = Request::new(
            http::Request::builder()
                .uri("/file.txt")
                .header(header::RANGE, "bytes=1-3")
                .body(Body::empty())
                .unwrap(),
        );
        let res = service.call(req).await.unwrap();
        assert_eq!(res.status(), StatusCode::PARTIAL_CONTENT);
        assert_eq!(res.headers()[header::CONTENT_RANGE], "bytes 1-3/6");
        assert_eq!(body_text(res).await, "bcd");

        let req = Request::new(
            http::Request::builder()
                .uri("/file.txt")
                .header(header::RANGE, "bytes=99-100")
                .body(Body::empty())
                .unwrap(),
        );
        assert_eq!(
            service.call(req).await.unwrap().status(),
            StatusCode::RANGE_NOT_SATISFIABLE
        );
    }

    #[cfg(feature = "fs")]
    #[tokio::test]
    async fn rejects_traversal_and_serves_precompressed() {
        let dir = tempfile::tempdir().unwrap();
        write(&dir.path().join("app.js"), b"plain");
        write(&dir.path().join("app.js.gz"), b"gzip");
        write(&dir.path().join("app.js.br"), b"brotli");

        let mut service = ServeDir::new(dir.path())
            .precompressed_gzip()
            .precompressed_br();

        assert_eq!(
            service
                .call(request("/../Cargo.toml"))
                .await
                .unwrap()
                .status(),
            StatusCode::NOT_FOUND
        );
        assert_eq!(
            service.call(request("/%zz")).await.unwrap().status(),
            StatusCode::NOT_FOUND
        );

        let req = Request::new(
            http::Request::builder()
                .uri("/app.js")
                .header(header::ACCEPT_ENCODING, "gzip, br")
                .body(Body::empty())
                .unwrap(),
        );
        let res = service.call(req).await.unwrap();
        assert_eq!(res.headers()[header::CONTENT_ENCODING], "br");
        assert_eq!(body_text(res).await, "brotli");
    }

    #[cfg(feature = "embed")]
    #[derive(rust_embed::RustEmbed)]
    #[folder = "test-assets/embed"]
    struct Assets;

    #[cfg(feature = "embed")]
    async fn embedded_text(res: Response) -> String {
        String::from_utf8(res.into_body().to_bytes().await.unwrap().to_vec()).unwrap()
    }

    #[cfg(feature = "embed")]
    #[tokio::test]
    async fn embedded_body_etag_range_and_listing() {
        let mut service = EmbeddedFileService::<Assets>::new().directory_listing(true);

        let res = service.call(request("/app.css")).await.unwrap();
        assert_eq!(res.status(), StatusCode::OK);
        assert_eq!(res.headers()[header::CONTENT_TYPE], "text/css");
        let etag = res.headers()[header::ETAG].clone();
        assert!(etag.to_str().unwrap().starts_with('"'));

        let req = Request::new(
            http::Request::builder()
                .uri("/app.css")
                .header(header::IF_NONE_MATCH, etag)
                .body(Body::empty())
                .unwrap(),
        );
        assert_eq!(
            service.call(req).await.unwrap().status(),
            StatusCode::NOT_MODIFIED
        );

        let req = Request::new(
            http::Request::builder()
                .uri("/app.css")
                .header(header::RANGE, "bytes=0-3")
                .body(Body::empty())
                .unwrap(),
        );
        assert_eq!(
            service.call(req).await.unwrap().status(),
            StatusCode::PARTIAL_CONTENT
        );

        let res = service.call(request("/dir/")).await.unwrap();
        assert!(embedded_text(res).await.contains("file.txt"));
    }
}
