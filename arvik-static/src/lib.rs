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
use std::path::{Path, PathBuf};
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
    /// Behind an Arc so the per-request service clone is a refcount bump —
    /// only an actual fallback invocation pays for the inner clone (C12).
    fallback: Option<Arc<BoxCloneService>>,
    precompressed_gzip: bool,
    precompressed_br: bool,
    call_fallback_on_method_not_allowed: bool,
    append_index_html_on_directories: bool,
    directory_listing: bool,
    chunk_size: usize,
    cache_control: Option<HeaderValue>,
    /// When false (default), requests resolving through a symlink to outside
    /// the (canonicalized) root are rejected.
    follow_symlinks: bool,
    /// Canonicalized serve root, resolved once on first use instead of per
    /// request (each `canonicalize` is a blocking-pool round-trip).
    canonical_root: tokio::sync::OnceCell<Option<std::sync::Arc<PathBuf>>>,
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
            follow_symlinks: false,
            canonical_root: tokio::sync::OnceCell::new(),
        }
    }

    /// Allow serving paths that resolve through symlinks.
    ///
    /// By default, a request whose resolved location escapes the serve root
    /// via a symlink is treated as not found. Enable this only when the tree
    /// is trusted (e.g. intentional mounts).
    pub fn follow_symlinks(mut self, enabled: bool) -> Self {
        self.follow_symlinks = enabled;
        self
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
        self.fallback = Some(Arc::new(BoxCloneService::new(service)));
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

        let candidate = self.root.join(&relative.asset_path);

        // Symlink containment: a planted link must not serve bytes from
        // outside the (canonicalized) root. Checked on the canonical path so
        // escapes through intermediate directories are caught too.
        if !self.follow_symlinks
            && !path_contained(&self.root, &self.canonical_root, &candidate).await
        {
            return self.call_fallback(req).await;
        }

        let metadata = match tokio::fs::metadata(&candidate).await {
            Ok(metadata) => metadata,
            Err(_) => return self.call_fallback(req).await,
        };

        if metadata.is_dir() {
            return self.handle_directory(req, relative, candidate).await;
        }

        let (accepts_br, accepts_gzip) =
            accepted_precompressed(&req, self.precompressed_br, self.precompressed_gzip);

        match self
            .select_file(
                candidate,
                metadata,
                &relative.asset_path,
                accepts_br,
                accepts_gzip,
            )
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
            let index_meta = tokio::fs::metadata(&index_path).await;
            if index_meta.as_ref().is_ok_and(|m| m.is_file()) {
                let index_meta = index_meta.unwrap();
                let index_asset_path = join_asset_path(&relative.asset_path, INDEX_FILE);
                let (accepts_br, accepts_gzip) =
                    accepted_precompressed(&req, self.precompressed_br, self.precompressed_gzip);

                return match self
                    .select_file(
                        index_path,
                        index_meta,
                        &index_asset_path,
                        accepts_br,
                        accepts_gzip,
                    )
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
        existing: std::fs::Metadata,
        asset_path: &str,
        accepts_br: bool,
        accepts_gzip: bool,
    ) -> Result<FsCandidate, ()> {
        let content_type = content_type(asset_path);

        // Probe order: `.br` → `.gz` → plain. Each probe is a single open
        // attempt (a miss costs one failed open instead of stat+open pairs —
        // audit C7b); `serve_fs_asset` re-validates via the open descriptor.
        let mut path = path;
        let mut content_encoding: Option<HeaderValue> = None;

        if accepts_br {
            let br_path = append_suffix(&path, ".br");
            if let Ok(meta) = tokio::fs::File::open(&br_path).await
                && meta.metadata().await.is_ok_and(|m| m.is_file())
            {
                path = br_path;
                content_encoding = Some(HeaderValue::from_static("br"));
                return Ok(FsCandidate {
                    path,
                    content_type,
                    content_encoding,
                    vary_accept_encoding: self.precompressed_gzip || self.precompressed_br,
                });
            }
        }

        if accepts_gzip {
            let gz_path = append_suffix(&path, ".gz");
            if let Ok(meta) = tokio::fs::File::open(&gz_path).await
                && meta.metadata().await.is_ok_and(|m| m.is_file())
            {
                path = gz_path;
                content_encoding = Some(HeaderValue::from_static("gzip"));
                return Ok(FsCandidate {
                    path,
                    content_type,
                    content_encoding,
                    vary_accept_encoding: self.precompressed_gzip || self.precompressed_br,
                });
            }
        }

        // `handle` already statted this exact path and ruled out directories —
        // reuse that metadata instead of a second syscall.
        if !existing.is_file() {
            return Err(());
        }
        Ok(FsCandidate {
            path,
            content_type,
            content_encoding,
            vary_accept_encoding: self.precompressed_gzip || self.precompressed_br,
        })
    }

    async fn call_fallback(&self, req: Request) -> Response {
        match &self.fallback {
            // The Arc deref clone is the one heap copy; it happens only when
            // the fallback actually runs.
            Some(service) => call_boxed((**service).clone(), req).await,
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
        let (accepts_br, accepts_gzip) =
            accepted_precompressed(&req, self.precompressed_br, self.precompressed_gzip);

        match self.select_file(accepts_br, accepts_gzip).await {
            Ok(asset) => {
                serve_fs_asset(asset, req, head, self.chunk_size, self.cache_control).await
            }
            Err(()) => not_found(),
        }
    }

    async fn select_file(&self, accepts_br: bool, accepts_gzip: bool) -> Result<FsCandidate, ()> {
        // Probe order: `.br` → `.gz` → plain; each probe is a single open
        // attempt (audit C7b).
        if accepts_br {
            let br_path = append_suffix(&self.path, ".br");
            if let Ok(f) = tokio::fs::File::open(&br_path).await
                && f.metadata().await.is_ok_and(|m| m.is_file())
            {
                return Ok(FsCandidate {
                    path: br_path,
                    content_type: self.mime.clone(),
                    content_encoding: Some(HeaderValue::from_static("br")),
                    vary_accept_encoding: self.precompressed_gzip || self.precompressed_br,
                });
            }
        }

        if accepts_gzip {
            let gz_path = append_suffix(&self.path, ".gz");
            if let Ok(f) = tokio::fs::File::open(&gz_path).await
                && f.metadata().await.is_ok_and(|m| m.is_file())
            {
                return Ok(FsCandidate {
                    path: gz_path,
                    content_type: self.mime.clone(),
                    content_encoding: Some(HeaderValue::from_static("gzip")),
                    vary_accept_encoding: self.precompressed_gzip || self.precompressed_br,
                });
            }
        }

        if !tokio::fs::metadata(&*self.path)
            .await
            .is_ok_and(|m| m.is_file())
        {
            return Err(());
        }
        Ok(FsCandidate {
            path: (*self.path).clone(),
            content_type: self.mime.clone(),
            content_encoding: None,
            vary_accept_encoding: self.precompressed_gzip || self.precompressed_br,
        })
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
    /// Behind an Arc so the per-request service clone is a refcount bump —
    /// only an actual fallback invocation pays for the inner clone (C12).
    fallback: Option<Arc<BoxCloneService>>,
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
        self.fallback = Some(Arc::new(BoxCloneService::new(service)));
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

        // File hit first (audit C8): `A::get` is O(1), while
        // `embedded_dir_exists` scans every embedded name. rust-embed output
        // cannot hold a name that is both a live file and a directory prefix,
        // so the reorder preserves behavior.
        if let Some(asset) = self.select_file(&relative.asset_path, &req) {
            return serve_embedded_asset(asset, req, head, self.cache_control);
        }

        if self.embedded_dir_exists(&relative.asset_path) {
            return self.handle_directory(req, relative).await;
        }

        self.call_fallback(req).await
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
        let (accepts_br, accepts_gzip) =
            accepted_precompressed(req, self.precompressed_br, self.precompressed_gzip);

        if accepts_br {
            let br_path = format!("{asset_path}.br");
            if let Some(file) = A::get(&br_path) {
                return Some(EmbeddedAsset::new(
                    &br_path,
                    file,
                    content_type.clone(),
                    Some(HeaderValue::from_static("br")),
                    self.precompressed_gzip || self.precompressed_br,
                ));
            }
        }

        if accepts_gzip {
            let gz_path = format!("{asset_path}.gz");
            if let Some(file) = A::get(&gz_path) {
                return Some(EmbeddedAsset::new(
                    &gz_path,
                    file,
                    content_type.clone(),
                    Some(HeaderValue::from_static("gzip")),
                    self.precompressed_gzip || self.precompressed_br,
                ));
            }
        }

        A::get(asset_path).map(|file| {
            EmbeddedAsset::new(
                asset_path,
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
            // The Arc deref clone is the one heap copy; it happens only when
            // the fallback actually runs.
            Some(service) => call_boxed((**service).clone(), req).await,
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

/// A filesystem candidate selected for serving — everything except metadata.
///
/// Metadata (length, mtime, ETag) is read from the *open* file at serving
/// time, closing the stat/open window where a deploy could swap or truncate
/// the file between selecting it and streaming it.
#[cfg(feature = "fs")]
struct FsCandidate {
    path: PathBuf,
    content_type: HeaderValue,
    content_encoding: Option<HeaderValue>,
    vary_accept_encoding: bool,
}

#[cfg(feature = "fs")]
async fn serve_fs_asset(
    candidate: FsCandidate,
    req: Request,
    head: bool,
    chunk_size: usize,
    cache_control: Option<HeaderValue>,
) -> Response {
    // Open first, then stat the open descriptor: the validators below are
    // guaranteed to describe exactly the bytes this call will stream.
    let file = match tokio::fs::File::open(&candidate.path).await {
        Ok(file) => file,
        Err(_) => return not_found(),
    };
    let metadata = match file.metadata().await {
        Ok(metadata) => metadata,
        Err(_) => return not_found(),
    };

    let len = metadata.len();
    let modified = metadata.modified().ok();
    let etag = fs_etag(len, modified);

    if is_not_modified(&req, &etag, modified) {
        return not_modified(
            &etag,
            modified,
            cache_control,
            candidate.vary_accept_encoding,
        );
    }

    // RFC 9110 §13.1.5: If-Range must gate the range — a stale validator
    // means the client's cached copy predates this file, so only a full 200
    // response can be safely stitched onto it.
    let range = if if_range_allows_partial(&req, &etag, modified) {
        match parse_range(req.headers().get(header::RANGE), len) {
            RangeDecision::Full => None,
            RangeDecision::Partial(range) => Some(range),
            RangeDecision::Unsatisfiable => return range_not_satisfiable(len),
        }
    } else {
        None
    };

    let body_len = range
        .as_ref()
        .map(|range| range.end - range.start + 1)
        .unwrap_or(len);

    let mut builder = ResponseBuilder::new()
        .status(if range.is_some() {
            StatusCode::PARTIAL_CONTENT
        } else {
            StatusCode::OK
        })
        .header(header::CONTENT_TYPE, candidate.content_type)
        .header(header::CONTENT_LENGTH, body_len.to_string())
        .header(header::ACCEPT_RANGES, "bytes")
        .header(header::ETAG, etag.clone());

    if let Some(modified) = modified {
        builder = builder.header(header::LAST_MODIFIED, httpdate::fmt_http_date(modified));
    }
    if let Some(cache_control) = cache_control {
        builder = builder.header(header::CACHE_CONTROL, cache_control);
    }
    if let Some(content_encoding) = candidate.content_encoding {
        builder = builder.header(header::CONTENT_ENCODING, content_encoding);
    }
    if candidate.vary_accept_encoding {
        builder = builder.header(header::VARY, "Accept-Encoding");
    }
    if let Some(range) = &range {
        builder = builder.header(
            header::CONTENT_RANGE,
            format!("bytes {}-{}/{}", range.start, range.end, len),
        );
    }

    if head {
        return builder.empty();
    }

    let body: Body = match range {
        Some(range) => ranged_file_body(file, range, chunk_size).await,
        None => file_body(file, chunk_size),
    };

    builder.body(body)
}

/// Evaluate `If-Range`: `true` means a partial response may be served.
///
/// ETag comparators use strong comparison; HTTP-date comparators must match
/// `Last-Modified` exactly. A missing header always allows ranges.
fn if_range_allows_partial(
    req: &Request,
    etag: &HeaderValue,
    modified: Option<SystemTime>,
) -> bool {
    let Some(value) = req.headers().get(header::IF_RANGE) else {
        return true;
    };
    let Ok(value) = value.to_str() else {
        return false;
    };

    if value.starts_with('"') || value.starts_with("W/") {
        // Weak validators never satisfy If-Range's strong comparison.
        return !value.starts_with("W/") && value.as_bytes() == etag.as_bytes();
    }

    match modified {
        Some(m) => httpdate::parse_http_date(value).is_ok_and(|date| date == m),
        None => false,
    }
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
/// Hash-derived per-asset facts. Immutable for the life of the binary, so a
/// process-wide cache builds each entry once instead of hex-formatting the
/// sha256 + validating an ETag header on every request (audit C14).
struct EmbeddedMeta {
    modified: Option<SystemTime>,
    etag: HeaderValue,
}

#[cfg(feature = "embed")]
fn embedded_meta(
    asset_path: &str,
    sha256: &[u8; 32],
    last_modified: Option<u64>,
) -> std::sync::Arc<EmbeddedMeta> {
    use std::collections::HashMap;
    use std::sync::{Arc, Mutex, OnceLock};

    static CACHE: OnceLock<Mutex<HashMap<Box<str>, Arc<EmbeddedMeta>>>> = OnceLock::new();
    let cache = CACHE.get_or_init(|| Mutex::new(HashMap::new()));

    // Hit path: no key allocation, no write lock.
    if let Ok(map) = cache.try_lock()
        && let Some(meta) = map.get(asset_path)
    {
        return Arc::clone(meta);
    }
    let mut map = cache.lock().expect("embedded metadata cache poisoned");
    if let Some(meta) = map.get(asset_path) {
        return Arc::clone(meta);
    }
    let meta = Arc::new(EmbeddedMeta {
        modified: last_modified.map(|seconds| UNIX_EPOCH + Duration::from_secs(seconds)),
        etag: embedded_etag(sha256),
    });
    map.insert(Box::from(asset_path), Arc::clone(&meta));
    meta
}

#[cfg(feature = "embed")]
impl EmbeddedAsset {
    fn new(
        asset_path: &str,
        file: rust_embed::EmbeddedFile,
        content_type: HeaderValue,
        content_encoding: Option<HeaderValue>,
        vary_accept_encoding: bool,
    ) -> Self {
        let len = file.data.len() as u64;
        let meta = embedded_meta(
            asset_path,
            &file.metadata.sha256_hash(),
            file.metadata.last_modified(),
        );
        let modified = meta.modified;
        let etag = meta.etag.clone();
        drop(meta);
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

    let range = if if_range_allows_partial(&req, &asset.etag, asset.modified) {
        match parse_range(req.headers().get(header::RANGE), asset.len) {
            RangeDecision::Full => None,
            RangeDecision::Partial(range) => Some(range),
            RangeDecision::Unsatisfiable => return range_not_satisfiable(asset.len),
        }
    } else {
        None
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

/// Verify that `path`, after resolving symlinks, still lives inside `root`.
///
/// Both sides are canonicalized so escapes through intermediate directory
/// links are caught as well as direct file links. Unresolvable paths are
/// treated as not contained.
#[cfg(feature = "fs")]
async fn path_contained(
    root: &Path,
    cached_root: &tokio::sync::OnceCell<Option<std::sync::Arc<PathBuf>>>,
    path: &Path,
) -> bool {
    // The serve root is canonicalized exactly once; only the candidate path
    // pays a realpath walk per request.
    let canonical_root = cached_root
        .get_or_init(|| async {
            tokio::fs::canonicalize(root)
                .await
                .ok()
                .map(std::sync::Arc::new)
        })
        .await;
    let Some(canonical_root) = canonical_root else {
        return false;
    };
    match tokio::fs::canonicalize(path).await {
        Ok(canonical_path) => canonical_path.starts_with(&**canonical_root),
        Err(_) => false,
    }
}

#[derive(Debug)]
struct RelativePath {
    /// Normalized, decoded request path. Doubles as the filesystem-relative
    /// path (`Path::join` views it) — one allocation serves both roles
    /// (audit C9).
    asset_path: String,
}

fn relative_path(path: &str) -> Result<RelativePath, ()> {
    if !valid_percent_encoding(path) {
        return Err(());
    }

    let decoded = percent_decode_str(path.trim_start_matches('/'))
        .decode_utf8()
        .map_err(|_| ())?;

    // Validate and normalize in one pass over the decoded bytes: keep Normal
    // components separated by '/', drop empty (`//`) and `.` segments, reject
    // traversal. This replaces `Path::components` machinery plus a Vec of
    // per-component Strings plus `join` with a single sized allocation
    // (audit C9). The leading-slash trim above rules out RootDir, and decoded
    // `%2F`s split exactly like real separators did under `components()`.
    // A decoded `%2F` at the front re-creates RootDir; keep rejecting it.
    if decoded.starts_with('/') || decoded.contains('\\') || decoded.contains('\0') {
        return Err(());
    }

    let mut asset_path = String::with_capacity(decoded.len());
    for part in decoded.split('/') {
        match part {
            "" | "." => {}          // empty (`//`) or CurDir
            ".." => return Err(()), // ParentDir
            _ => {
                if !asset_path.is_empty() {
                    asset_path.push('/');
                }
                asset_path.push_str(part);
            }
        }
    }

    Ok(RelativePath { asset_path })
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
    // MIME-for-extension is constant — resolve the hot web types to 'static
    // header values and skip mime_guess + per-request validation entirely
    // (audit C10). Values mirror mime_guess 2.0.5's table exactly; anything
    // uncommon falls through to it unchanged.
    let ext = std::path::Path::new(path)
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("");
    // mime_guess lowercases before lookup — match that without allocating in
    // the already-lowercase common case.
    let owned;
    let ext = if ext.bytes().any(|b| b.is_ascii_uppercase()) {
        owned = ext.to_ascii_lowercase();
        owned.as_str()
    } else {
        ext
    };
    let mime = match ext {
        "html" | "htm" => "text/html",
        "css" => "text/css",
        "js" => "text/javascript",
        "mjs" => "application/javascript",
        "json" => "application/json",
        "txt" => "text/plain",
        "xml" => "text/xml",
        "svg" => "image/svg+xml",
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "gif" => "image/gif",
        "webp" => "image/webp",
        "avif" => "image/avif",
        "ico" => "image/x-icon",
        "wasm" => "application/wasm",
        "woff" => "application/font-woff",
        "woff2" => "font/woff2",
        "ttf" => "font/ttf",
        "otf" => "application/font-sfnt",
        "mp4" => "video/mp4",
        "webm" => "video/webm",
        "pdf" => "application/pdf",
        _ => {
            return HeaderValue::from_str(
                mime_guess::from_path(path)
                    .first_or_octet_stream()
                    .essence_str(),
            )
            .expect("MIME values are valid headers");
        }
    };
    HeaderValue::from_static(mime)
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

/// RFC 9110 §5.2 allows multi-line lists — fold every header line into one
/// string without an intermediate Vec so both precompressed probes share a
/// single header read (audit C13).
fn combined_accept_encoding(req: &Request) -> String {
    let mut combined = String::new();
    for value in req.headers().get_all(header::ACCEPT_ENCODING) {
        if let Ok(v) = value.to_str() {
            if !combined.is_empty() {
                combined.push_str(", ");
            }
            combined.push_str(v);
        }
    }
    combined
}

/// Negotiate both encodings from one combined header string. Q-value
/// semantics apply — a bare `*;q=1` does not override an explicit
/// `br;q=0` refusal.
fn accepted_precompressed(req: &Request, want_br: bool, want_gzip: bool) -> (bool, bool) {
    if !(want_br || want_gzip) {
        return (false, false);
    }
    let combined = combined_accept_encoding(req);
    let ok = |enc: &str| arvik_core::accept::negotiate(&[enc], &combined).is_some();
    (want_br && ok("br"), want_gzip && ok("gzip"))
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

    // RFC 9110 §14.2: an unrecognized range unit MUST be ignored (serve 200
    // full), not answered with 416.
    let Some(spec) = value.strip_prefix("bytes=") else {
        return RangeDecision::Full;
    };
    // Multi-range requests would need multipart/byteranges bodies; serving a
    // single span would corrupt clients expecting all ranges. Full 200 is the
    // safe, spec-permitted answer.
    if spec.contains(',') || spec.is_empty() {
        return RangeDecision::Full;
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
    // Strong validator (nginx-style mtime+len). Strong comparison is required
    // for `If-Range` to allow partial responses at all, and lets range-aware
    // clients resume downloads against this file.
    HeaderValue::from_str(&format!(
        "\"{:x}-{:x}-{:x}\"",
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
    // ── round 3: relative_path normalization, content_type table parity ─────

    #[test]
    fn relative_path_normalizes_and_rejects_traversal() {
        // Normalization: duplicate slashes, dot segments, trailing slash.
        assert_eq!(
            relative_path("//assets//js/./app.js/").unwrap().asset_path,
            "assets/js/app.js"
        );
        // Root request collapses to empty.
        assert_eq!(relative_path("/").unwrap().asset_path, "");
        assert_eq!(relative_path("").unwrap().asset_path, "");

        // Rejections: parent traversal (raw and percent-encoded), decoded
        // RootDir (%2F at the front), backslashes, NUL.
        assert!(relative_path("/../etc/passwd").is_err());
        assert!(relative_path("/a/../../etc/passwd").is_err());
        assert!(relative_path("/%2e%2e/etc/passwd").is_err());
        assert!(relative_path("/a/%2e%2e/passwd").is_err());
        assert!(relative_path("/%2Fetc%2Fpasswd").is_err());
        assert!(relative_path("/a\\b").is_err());
        assert!(relative_path("/a\0b").is_err());

        // Consecutive dots inside a name are NOT traversal.
        assert_eq!(
            relative_path("/logo..final.png").unwrap().asset_path,
            "logo..final.png"
        );
        // Invalid percent escapes rejected before decode.
        assert!(relative_path("/a%zz").is_err());
    }

    #[test]
    fn content_type_matches_mime_guess_for_table_and_fallbacks() {
        let tabled = [
            "a.html", "a.htm", "a.css", "a.js", "a.mjs", "a.json", "a.txt", "a.xml", "a.svg",
            "a.png", "a.jpg", "a.jpeg", "a.gif", "a.webp", "a.avif", "a.ico", "a.wasm", "a.woff",
            "a.woff2", "a.ttf", "a.otf", "a.mp4", "a.webm", "a.pdf",
        ];
        for path in tabled {
            let expected = HeaderValue::from_str(
                mime_guess::from_path(path)
                    .first_or_octet_stream()
                    .essence_str(),
            )
            .unwrap();
            assert_eq!(content_type(path), expected, "table drift for {path}");
        }

        // Case-insensitive lookup, same as mime_guess.
        assert_eq!(content_type("A.PNG"), content_type("a.png"));

        // Fallbacks: unknown extension, no extension, dotfile.
        for path in ["a.xyz123", "noext", ".hidden", "dir.d/", "trailing."] {
            let expected = HeaderValue::from_str(
                mime_guess::from_path(path)
                    .first_or_octet_stream()
                    .essence_str(),
            )
            .unwrap();
            assert_eq!(content_type(path), expected, "fallback drift for {path}");
        }
    }

    // ── audit group 7: If-Range, symlink containment ─────────────────────────

    #[test]
    fn if_range_strong_etag_comparison() {
        let etag = HeaderValue::from_static("\"abc123\"");
        let modified = None;

        // No header → ranges allowed.
        let req = request("/f");
        assert!(if_range_allows_partial(&req, &etag, modified));

        // Exact strong match → allowed.
        let mut req = request("/f");
        req.headers_mut()
            .insert(header::IF_RANGE, HeaderValue::from_static("\"abc123\""));
        assert!(if_range_allows_partial(&req, &etag, modified));

        // Different validator → not allowed.
        let mut req = request("/f");
        req.headers_mut()
            .insert(header::IF_RANGE, HeaderValue::from_static("\"other\""));
        assert!(!if_range_allows_partial(&req, &etag, modified));

        // Weak validators never satisfy strong comparison.
        let mut req = request("/f");
        req.headers_mut()
            .insert(header::IF_RANGE, HeaderValue::from_static("W/\"abc123\""));
        assert!(!if_range_allows_partial(&req, &etag, modified));
    }

    #[tokio::test]
    async fn stale_if_range_downgrades_range_to_full_response() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("data.bin");
        write(&path, b"0123456789");

        let mut service = ServeFile::new(&path);

        // Discover the current ETag.
        let res = service.call(request("/ignored")).await.unwrap();
        let etag = res.headers()[header::ETAG].clone();

        // Matching If-Range → partial content.
        let mut req = request("/ignored");
        req.headers_mut()
            .insert(header::RANGE, HeaderValue::from_static("bytes=0-3"));
        req.headers_mut().insert(header::IF_RANGE, etag.clone());
        let res = service.call(req).await.unwrap();
        assert_eq!(res.status(), StatusCode::PARTIAL_CONTENT);
        assert_eq!(body_text(res).await, "0123");

        // Stale If-Range → full 200 (client must discard its cached version).
        let mut req = request("/ignored");
        req.headers_mut()
            .insert(header::RANGE, HeaderValue::from_static("bytes=0-3"));
        req.headers_mut()
            .insert(header::IF_RANGE, HeaderValue::from_static("\"stale\""));
        let res = service.call(req).await.unwrap();
        assert_eq!(res.status(), StatusCode::OK);
        assert_eq!(body_text(res).await, "0123456789");
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn symlink_escape_is_blocked_by_default() {
        let outside = tempfile::tempdir().unwrap();
        let secret = outside.path().join("secret.txt");
        write(&secret, b"top secret");

        let root = tempfile::tempdir().unwrap();
        #[cfg(unix)]
        std::os::unix::fs::symlink(&secret, root.path().join("leak.txt")).unwrap();

        let mut blocked = ServeDir::new(root.path());
        let res = blocked.call(request("/leak.txt")).await.unwrap();
        assert_ne!(
            body_text(res).await,
            "top secret",
            "symlink escape must not serve outside the root by default"
        );

        // Opt-in restores link traversal.
        let mut allowed = ServeDir::new(root.path()).follow_symlinks(true);
        let res = allowed.call(request("/leak.txt")).await.unwrap();
        assert_eq!(res.status(), StatusCode::OK);
        assert_eq!(body_text(res).await, "top secret");
    }
}
