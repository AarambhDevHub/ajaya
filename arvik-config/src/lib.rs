//! File and environment configuration for Arvik.
//!
//! This crate maps higher-level application configuration onto Arvik's
//! low-level server/runtime types. It intentionally keeps file/env parsing out
//! of `arvik_hyper::ServerConfig`.

use std::path::{Path, PathBuf};
use std::time::Duration;

use arvik_hyper::{ServerConfig, ShutdownConfig};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use thiserror::Error;

/// Result type used by the configuration system.
pub type Result<T> = std::result::Result<T, ConfigError>;

/// Configuration load, parse, and validation errors.
#[derive(Debug, Error)]
pub enum ConfigError {
    /// A config file could not be read.
    #[error("failed to read config file `{path}`: {source}")]
    Io {
        /// File path.
        path: PathBuf,
        /// Source error.
        #[source]
        source: std::io::Error,
    },
    /// A TOML config file could not be parsed.
    #[error("failed to parse TOML config file `{path}`: {source}")]
    Toml {
        /// File path.
        path: PathBuf,
        /// Source error.
        #[source]
        source: toml::de::Error,
    },
    /// A JSON config file could not be parsed.
    #[error("failed to parse JSON config file `{path}`: {source}")]
    Json {
        /// File path.
        path: PathBuf,
        /// Source error.
        #[source]
        source: serde_json::Error,
    },
    /// A config file extension is not supported.
    #[error("unsupported config file extension for `{path}`; expected `.toml` or `.json`")]
    UnsupportedExtension {
        /// File path.
        path: PathBuf,
    },
    /// An environment override was invalid.
    #[error("invalid environment variable `{var}`: {message}")]
    Env {
        /// Variable name.
        var: String,
        /// Human-readable failure.
        message: String,
    },
    /// The merged config could not be decoded into the schema.
    #[error("failed to decode merged config: {0}")]
    Decode(#[from] serde_json::Error),
    /// The config schema failed validation.
    #[error("invalid config: {0}")]
    Validation(String),
    /// The configured bind address is invalid.
    #[error("invalid bind address `{addr}`: {source}")]
    Address {
        /// Address string.
        addr: String,
        /// Source error.
        #[source]
        source: std::net::AddrParseError,
    },
    /// Hot reload watcher failure.
    #[cfg(feature = "hot-reload")]
    #[error("config watcher error: {0}")]
    Watch(String),
}

/// Complete Arvik configuration loaded from defaults, files, and environment.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct ArvikConfig {
    /// Listener, admission, and app-level server settings.
    pub server: ServerSection,
    /// HTTP/1 protocol settings.
    pub http1: Http1Section,
    /// HTTP/2 protocol settings.
    pub http2: Http2Section,
    /// Optional TLS file settings.
    pub tls: Option<TlsSection>,
    /// Graceful shutdown settings.
    pub shutdown: ShutdownSection,
}

impl ArvikConfig {
    /// Create a config builder.
    pub fn builder() -> ArvikConfigBuilder {
        ArvikConfigBuilder::default()
    }

    /// Return the bind address as a string.
    pub fn bind_addr_string(&self) -> String {
        if self.server.host.contains(':')
            && !self.server.host.starts_with('[')
            && !self.server.host.ends_with(']')
        {
            format!("[{}]:{}", self.server.host, self.server.port)
        } else {
            format!("{}:{}", self.server.host, self.server.port)
        }
    }

    /// Parse and return the bind address.
    pub fn bind_addr(&self) -> Result<std::net::SocketAddr> {
        let addr = self.bind_addr_string();
        addr.parse()
            .map_err(|source| ConfigError::Address { addr, source })
    }

    /// Convert this config into low-level Arvik server tuning.
    pub fn server_config(&self) -> ServerConfig {
        let mut config = ServerConfig::default()
            .http1_keep_alive(self.http1.keep_alive)
            .http1_half_close(self.http1.half_close)
            .http1_title_case_headers(self.http1.title_case_headers)
            .http1_preserve_header_case(self.http1.preserve_header_case)
            .http1_pipeline_flush(self.http1.pipeline_flush)
            .http2_only(self.http2.only)
            .http2_adaptive_window(self.http2.adaptive_window)
            .http2_keep_alive_timeout(Duration::from_secs(self.http2.keep_alive_timeout_secs));

        if let Some(max) = self.server.max_connections {
            config = config.max_connections(max);
        }
        if let Some(enabled) = self.server.tcp_nodelay {
            config = config.tcp_nodelay(enabled);
        }
        if let Some(secs) = self.server.tcp_keepalive_secs {
            config = config.tcp_keepalive(Duration::from_secs(secs));
        }
        if let Some(secs) = self.server.tcp_keepalive_interval_secs {
            config = config.tcp_keepalive_interval(Duration::from_secs(secs));
        }
        if let Some(retries) = self.server.tcp_keepalive_retries {
            config = config.tcp_keepalive_retries(retries);
        }
        if self.server.reuse_port {
            config = config.reuse_port(true);
        }
        if self.server.reuse_address {
            config = config.reuse_address(true);
        }
        if let Some(backlog) = self.server.backlog {
            config = config.backlog(backlog as i32);
        }
        if let Some(size) = self.server.socket_recv_buffer_size {
            config = config.socket_recv_buffer_size(size);
        }
        if let Some(size) = self.server.socket_send_buffer_size {
            config = config.socket_send_buffer_size(size);
        }
        if let Some(workers) = self.server.accept_workers {
            config = config.accept_workers(workers);
        }
        if let Some(size) = self.http2.initial_stream_window_size {
            config = config.http2_initial_stream_window_size(size);
        }
        if let Some(size) = self.http2.initial_connection_window_size {
            config = config.http2_initial_connection_window_size(size);
        }
        match self.http2.max_concurrent_streams {
            Some(max) => config = config.http2_max_concurrent_streams(max),
            None => config = config.http2_unlimited_concurrent_streams(),
        }
        match self.http2.keep_alive_interval_secs {
            Some(secs) => config = config.http2_keep_alive_interval(Duration::from_secs(secs)),
            None => config = config.http2_disable_keep_alive(),
        }
        if let Some(size) = self.http2.max_frame_size {
            config = config.http2_max_frame_size(size);
        }
        if let Some(size) = self.http2.max_send_buf_size {
            config = config.http2_max_send_buf_size(size);
        }
        if let Some(size) = self.http2.max_header_list_size {
            config = config.http2_max_header_list_size(size);
        }

        config
    }

    /// Convert this config into graceful shutdown tuning.
    pub fn shutdown_config(&self) -> ShutdownConfig {
        ShutdownConfig::default()
            .drain_timeout(Duration::from_secs(self.shutdown.drain_timeout_secs))
    }

    /// Validate this config.
    pub fn validate(&self) -> Result<()> {
        if self.server.port == 0 {
            return Err(ConfigError::Validation(
                "server.port must be between 1 and 65535".into(),
            ));
        }
        validate_nonzero(self.server.workers, "server.workers")?;
        validate_nonzero(self.server.backlog, "server.backlog")?;
        validate_nonzero(self.server.max_connections, "server.max_connections")?;
        validate_nonzero(self.server.body_limit, "server.body_limit")?;
        validate_nonzero(self.server.tcp_keepalive_secs, "server.tcp_keepalive_secs")?;
        validate_nonzero(
            self.server.tcp_keepalive_interval_secs,
            "server.tcp_keepalive_interval_secs",
        )?;
        validate_nonzero(
            self.server.tcp_keepalive_retries,
            "server.tcp_keepalive_retries",
        )?;
        validate_nonzero(
            self.server.socket_recv_buffer_size,
            "server.socket_recv_buffer_size",
        )?;
        validate_nonzero(
            self.server.socket_send_buffer_size,
            "server.socket_send_buffer_size",
        )?;
        validate_nonzero(self.server.accept_workers, "server.accept_workers")?;
        if self.server.accept_workers.unwrap_or(1) > 1 && !self.server.reuse_port {
            return Err(ConfigError::Validation(
                "server.accept_workers greater than 1 requires server.reuse_port = true".into(),
            ));
        }
        if let Some(backlog) = self.server.backlog {
            if backlog > i32::MAX as u32 {
                return Err(ConfigError::Validation(
                    "server.backlog must fit into i32".into(),
                ));
            }
        }
        validate_nonzero(
            self.http2.initial_stream_window_size,
            "http2.initial_stream_window_size",
        )?;
        validate_nonzero(
            self.http2.initial_connection_window_size,
            "http2.initial_connection_window_size",
        )?;
        validate_nonzero(
            self.http2.max_concurrent_streams,
            "http2.max_concurrent_streams",
        )?;
        validate_nonzero(self.http2.max_send_buf_size, "http2.max_send_buf_size")?;
        validate_nonzero(
            self.http2.max_header_list_size,
            "http2.max_header_list_size",
        )?;

        if self.http2.keep_alive_interval_secs == Some(0) {
            return Err(ConfigError::Validation(
                "http2.keep_alive_interval_secs must be greater than 0 or omitted".into(),
            ));
        }
        if self.http2.keep_alive_timeout_secs == 0 {
            return Err(ConfigError::Validation(
                "http2.keep_alive_timeout_secs must be greater than 0".into(),
            ));
        }
        if let Some(size) = self.http2.max_frame_size {
            if !(16_384..=16_777_215).contains(&size) {
                return Err(ConfigError::Validation(
                    "http2.max_frame_size must be between 16384 and 16777215".into(),
                ));
            }
        }
        if self.shutdown.drain_timeout_secs == 0 {
            return Err(ConfigError::Validation(
                "shutdown.drain_timeout_secs must be greater than 0".into(),
            ));
        }
        if let Some(tls) = &self.tls {
            tls.validate()?;
        }

        Ok(())
    }
}

/// Listener, admission, and app-level server settings.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct ServerSection {
    /// Listener host.
    pub host: String,
    /// Listener port.
    pub port: u16,
    /// Runtime worker hint for applications that build their own Tokio runtime.
    pub workers: Option<usize>,
    /// Socket backlog hint for future listener builders.
    pub backlog: Option<u32>,
    /// Maximum concurrently admitted connections.
    pub max_connections: Option<usize>,
    /// Application request body limit. Apply with `RequestBodyLimitLayer`.
    pub body_limit: Option<usize>,
    /// Explicit TCP_NODELAY setting for accepted sockets.
    pub tcp_nodelay: Option<bool>,
    /// TCP keepalive idle time in seconds.
    pub tcp_keepalive_secs: Option<u64>,
    /// TCP keepalive probe interval in seconds.
    pub tcp_keepalive_interval_secs: Option<u64>,
    /// TCP keepalive retry count where supported.
    pub tcp_keepalive_retries: Option<u32>,
    /// Enable SO_REUSEPORT for tuned listeners.
    pub reuse_port: bool,
    /// Enable SO_REUSEADDR for tuned listeners.
    pub reuse_address: bool,
    /// Listener receive buffer size.
    pub socket_recv_buffer_size: Option<usize>,
    /// Listener send buffer size.
    pub socket_send_buffer_size: Option<usize>,
    /// Number of accept worker listener sockets.
    pub accept_workers: Option<usize>,
}

impl Default for ServerSection {
    fn default() -> Self {
        Self {
            host: "127.0.0.1".into(),
            port: 8080,
            workers: None,
            backlog: None,
            max_connections: None,
            body_limit: None,
            tcp_nodelay: None,
            tcp_keepalive_secs: None,
            tcp_keepalive_interval_secs: None,
            tcp_keepalive_retries: None,
            reuse_port: false,
            reuse_address: false,
            socket_recv_buffer_size: None,
            socket_send_buffer_size: None,
            accept_workers: None,
        }
    }
}

/// HTTP/1 protocol settings.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Http1Section {
    /// Enable HTTP/1 keep-alive.
    pub keep_alive: bool,
    /// Enable HTTP/1 half-close support.
    pub half_close: bool,
    /// Write response headers in title case.
    pub title_case_headers: bool,
    /// Preserve incoming header case where Hyper supports it.
    pub preserve_header_case: bool,
    /// Aggregate flushes for pipelined responses.
    pub pipeline_flush: bool,
}

impl Default for Http1Section {
    fn default() -> Self {
        Self {
            keep_alive: true,
            half_close: false,
            title_case_headers: false,
            preserve_header_case: false,
            pipeline_flush: false,
        }
    }
}

/// HTTP/2 protocol settings.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Http2Section {
    /// Accept HTTP/2 prior-knowledge connections only.
    pub only: bool,
    /// Enable adaptive flow control.
    pub adaptive_window: bool,
    /// Initial stream window size.
    pub initial_stream_window_size: Option<u32>,
    /// Initial connection window size.
    pub initial_connection_window_size: Option<u32>,
    /// Maximum concurrent streams. `None` disables the Hyper limit.
    pub max_concurrent_streams: Option<u32>,
    /// Keep-alive ping interval in seconds. `None` disables keep-alive pings.
    pub keep_alive_interval_secs: Option<u64>,
    /// Keep-alive acknowledgement timeout in seconds.
    pub keep_alive_timeout_secs: u64,
    /// Maximum HTTP/2 frame size.
    pub max_frame_size: Option<u32>,
    /// Maximum send buffer size per stream.
    pub max_send_buf_size: Option<usize>,
    /// Maximum header list size.
    pub max_header_list_size: Option<u32>,
}

impl Default for Http2Section {
    fn default() -> Self {
        Self {
            only: false,
            adaptive_window: true,
            initial_stream_window_size: None,
            initial_connection_window_size: None,
            max_concurrent_streams: Some(1_000),
            keep_alive_interval_secs: Some(20),
            keep_alive_timeout_secs: 10,
            max_frame_size: None,
            max_send_buf_size: None,
            max_header_list_size: Some(64 * 1024),
        }
    }
}

/// Optional TLS file settings.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct TlsSection {
    /// TLS backend.
    pub backend: TlsBackend,
    /// PEM certificate path for rustls.
    pub cert_path: Option<PathBuf>,
    /// PEM private key path for rustls.
    pub key_path: Option<PathBuf>,
    /// PKCS#12 identity path for native-tls.
    pub pkcs12_path: Option<PathBuf>,
    /// PKCS#12 identity password for native-tls.
    pub pkcs12_password: Option<String>,
}

impl Default for TlsSection {
    fn default() -> Self {
        Self {
            backend: TlsBackend::Rustls,
            cert_path: None,
            key_path: None,
            pkcs12_path: None,
            pkcs12_password: None,
        }
    }
}

impl TlsSection {
    fn validate(&self) -> Result<()> {
        match self.backend {
            TlsBackend::Rustls => {
                if self.cert_path.is_none() || self.key_path.is_none() {
                    return Err(ConfigError::Validation(
                        "tls.cert_path and tls.key_path are required for rustls".into(),
                    ));
                }
            }
            TlsBackend::NativeTls => {
                if self.pkcs12_path.is_none() {
                    return Err(ConfigError::Validation(
                        "tls.pkcs12_path is required for native-tls".into(),
                    ));
                }
            }
        }

        Ok(())
    }
}

/// Supported TLS backends in config files.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum TlsBackend {
    /// rustls backend.
    Rustls,
    /// native-tls backend.
    NativeTls,
}

/// Graceful shutdown settings.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct ShutdownSection {
    /// Drain timeout in seconds.
    pub drain_timeout_secs: u64,
}

impl Default for ShutdownSection {
    fn default() -> Self {
        Self {
            drain_timeout_secs: 30,
        }
    }
}

/// Builder for [`ArvikConfig`].
#[derive(Debug, Clone)]
pub struct ArvikConfigBuilder {
    files: Vec<PathBuf>,
    env_prefix: String,
    defaults: ArvikConfig,
}

impl Default for ArvikConfigBuilder {
    fn default() -> Self {
        Self {
            files: Vec::new(),
            env_prefix: "ARVIK".into(),
            defaults: ArvikConfig::default(),
        }
    }
}

impl ArvikConfigBuilder {
    /// Add a TOML or JSON config file.
    #[must_use]
    pub fn file(mut self, path: impl Into<PathBuf>) -> Self {
        self.files.push(path.into());
        self
    }

    /// Set the environment variable prefix. Defaults to `ARVIK`.
    #[must_use]
    pub fn env_prefix(mut self, prefix: impl Into<String>) -> Self {
        self.env_prefix = prefix.into();
        self
    }

    /// Set explicit defaults before files and env vars are applied.
    #[must_use]
    pub fn defaults(mut self, config: ArvikConfig) -> Self {
        self.defaults = config;
        self
    }

    /// Load, merge, and validate the config.
    pub fn build(self) -> Result<ArvikConfig> {
        self.build_inner()
    }

    fn build_inner(&self) -> Result<ArvikConfig> {
        let mut merged = serde_json::to_value(&self.defaults)?;

        for file in &self.files {
            let value = load_file(file)?;
            merge_values(&mut merged, value);
        }

        apply_env(&mut merged, &self.env_prefix)?;

        let config: ArvikConfig = serde_json::from_value(merged)?;
        config.validate()?;
        Ok(config)
    }

    /// Watch configured files and publish valid config updates.
    #[cfg(feature = "hot-reload")]
    pub fn watch(self) -> Result<ConfigWatcher> {
        ConfigWatcher::new(self)
    }
}

fn load_file(path: &Path) -> Result<Value> {
    let contents = std::fs::read_to_string(path).map_err(|source| ConfigError::Io {
        path: path.to_path_buf(),
        source,
    })?;

    match path.extension().and_then(|ext| ext.to_str()) {
        Some("toml") => {
            let value: toml::Value =
                toml::from_str(&contents).map_err(|source| ConfigError::Toml {
                    path: path.to_path_buf(),
                    source,
                })?;
            serde_json::to_value(value).map_err(ConfigError::from)
        }
        Some("json") => serde_json::from_str(&contents).map_err(|source| ConfigError::Json {
            path: path.to_path_buf(),
            source,
        }),
        _ => Err(ConfigError::UnsupportedExtension {
            path: path.to_path_buf(),
        }),
    }
}

fn merge_values(base: &mut Value, overlay: Value) {
    match (base, overlay) {
        (Value::Object(base), Value::Object(overlay)) => {
            for (key, value) in overlay {
                merge_values(base.entry(key).or_insert(Value::Null), value);
            }
        }
        (base, overlay) => *base = overlay,
    }
}

fn apply_env(root: &mut Value, prefix: &str) -> Result<()> {
    let prefix = format!("{}_", prefix.trim_end_matches('_'));
    for (var, value) in std::env::vars() {
        if !var.starts_with(&prefix) {
            continue;
        }

        let suffix = &var[prefix.len()..];
        let Some((path, value)) = parse_env_override(suffix, &var, &value)? else {
            continue;
        };
        set_path(root, path, value);
    }

    Ok(())
}

fn parse_env_override(
    suffix: &str,
    var: &str,
    value: &str,
) -> Result<Option<(&'static [&'static str], Value)>> {
    macro_rules! bool_value {
        ($path:expr) => {{
            let path: &'static [&'static str] = $path;
            Some((path, Value::Bool(parse_bool(var, value)?)))
        }};
    }
    macro_rules! string_value {
        ($path:expr) => {{
            let path: &'static [&'static str] = $path;
            Some((path, Value::String(value.to_owned())))
        }};
    }
    macro_rules! number_value {
        ($path:expr, $ty:ty) => {{
            let path: &'static [&'static str] = $path;
            let parsed: $ty = value.parse().map_err(|_| ConfigError::Env {
                var: var.into(),
                message: format!("expected {}", stringify!($ty)),
            })?;
            Some((path, Value::from(parsed)))
        }};
    }

    let parsed = match suffix {
        "SERVER_HOST" => string_value!(&["server", "host"]),
        "SERVER_PORT" => number_value!(&["server", "port"], u16),
        "SERVER_WORKERS" => number_value!(&["server", "workers"], usize),
        "SERVER_BACKLOG" => number_value!(&["server", "backlog"], u32),
        "SERVER_MAX_CONNECTIONS" => number_value!(&["server", "max_connections"], usize),
        "SERVER_BODY_LIMIT" => number_value!(&["server", "body_limit"], usize),
        "SERVER_TCP_NODELAY" => bool_value!(&["server", "tcp_nodelay"]),
        "SERVER_TCP_KEEPALIVE_SECS" => {
            number_value!(&["server", "tcp_keepalive_secs"], u64)
        }
        "SERVER_TCP_KEEPALIVE_INTERVAL_SECS" => {
            number_value!(&["server", "tcp_keepalive_interval_secs"], u64)
        }
        "SERVER_TCP_KEEPALIVE_RETRIES" => {
            number_value!(&["server", "tcp_keepalive_retries"], u32)
        }
        "SERVER_REUSE_PORT" => bool_value!(&["server", "reuse_port"]),
        "SERVER_REUSE_ADDRESS" => bool_value!(&["server", "reuse_address"]),
        "SERVER_SOCKET_RECV_BUFFER_SIZE" => {
            number_value!(&["server", "socket_recv_buffer_size"], usize)
        }
        "SERVER_SOCKET_SEND_BUFFER_SIZE" => {
            number_value!(&["server", "socket_send_buffer_size"], usize)
        }
        "SERVER_ACCEPT_WORKERS" => number_value!(&["server", "accept_workers"], usize),
        "HTTP1_KEEP_ALIVE" => bool_value!(&["http1", "keep_alive"]),
        "HTTP1_HALF_CLOSE" => bool_value!(&["http1", "half_close"]),
        "HTTP1_TITLE_CASE_HEADERS" => bool_value!(&["http1", "title_case_headers"]),
        "HTTP1_PRESERVE_HEADER_CASE" => bool_value!(&["http1", "preserve_header_case"]),
        "HTTP1_PIPELINE_FLUSH" => bool_value!(&["http1", "pipeline_flush"]),
        "HTTP2_ONLY" => bool_value!(&["http2", "only"]),
        "HTTP2_ADAPTIVE_WINDOW" => bool_value!(&["http2", "adaptive_window"]),
        "HTTP2_INITIAL_STREAM_WINDOW_SIZE" => {
            number_value!(&["http2", "initial_stream_window_size"], u32)
        }
        "HTTP2_INITIAL_CONNECTION_WINDOW_SIZE" => {
            number_value!(&["http2", "initial_connection_window_size"], u32)
        }
        "HTTP2_MAX_CONCURRENT_STREAMS" => {
            number_value!(&["http2", "max_concurrent_streams"], u32)
        }
        "HTTP2_KEEP_ALIVE_INTERVAL_SECS" => {
            number_value!(&["http2", "keep_alive_interval_secs"], u64)
        }
        "HTTP2_KEEP_ALIVE_TIMEOUT_SECS" => {
            number_value!(&["http2", "keep_alive_timeout_secs"], u64)
        }
        "HTTP2_MAX_FRAME_SIZE" => number_value!(&["http2", "max_frame_size"], u32),
        "HTTP2_MAX_SEND_BUF_SIZE" => number_value!(&["http2", "max_send_buf_size"], usize),
        "HTTP2_MAX_HEADER_LIST_SIZE" => {
            number_value!(&["http2", "max_header_list_size"], u32)
        }
        "TLS_BACKEND" => string_value!(&["tls", "backend"]),
        "TLS_CERT_PATH" => string_value!(&["tls", "cert_path"]),
        "TLS_KEY_PATH" => string_value!(&["tls", "key_path"]),
        "TLS_PKCS12_PATH" => string_value!(&["tls", "pkcs12_path"]),
        "TLS_PKCS12_PASSWORD" => string_value!(&["tls", "pkcs12_password"]),
        "SHUTDOWN_DRAIN_TIMEOUT_SECS" => {
            number_value!(&["shutdown", "drain_timeout_secs"], u64)
        }
        _ => None,
    };

    Ok(parsed)
}

fn parse_bool(var: &str, value: &str) -> Result<bool> {
    match value.to_ascii_lowercase().as_str() {
        "true" | "1" | "yes" | "on" => Ok(true),
        "false" | "0" | "no" | "off" => Ok(false),
        _ => Err(ConfigError::Env {
            var: var.into(),
            message: "expected boolean value".into(),
        }),
    }
}

fn set_path(root: &mut Value, path: &[&str], value: Value) {
    let mut current = root;
    for segment in &path[..path.len() - 1] {
        if !current.is_object() {
            *current = Value::Object(Map::new());
        }
        current = current
            .as_object_mut()
            .expect("object just inserted")
            .entry((*segment).to_owned())
            .or_insert_with(|| Value::Object(Map::new()));
    }

    if !current.is_object() {
        *current = Value::Object(Map::new());
    }
    current
        .as_object_mut()
        .expect("object just inserted")
        .insert(path[path.len() - 1].to_owned(), value);
}

fn validate_nonzero<T>(value: Option<T>, name: &str) -> Result<()>
where
    T: PartialEq + From<u8>,
{
    if value == Some(T::from(0)) {
        return Err(ConfigError::Validation(format!(
            "{name} must be greater than 0"
        )));
    }

    Ok(())
}

/// Debounced config hot reload watcher.
#[cfg(feature = "hot-reload")]
pub struct ConfigWatcher {
    current: std::sync::Arc<std::sync::RwLock<ArvikConfig>>,
    updates: tokio::sync::mpsc::UnboundedReceiver<Result<ArvikConfig>>,
    _watcher: notify::RecommendedWatcher,
}

#[cfg(feature = "hot-reload")]
impl ConfigWatcher {
    fn new(builder: ArvikConfigBuilder) -> Result<Self> {
        use notify::Watcher;

        let initial = builder.build_inner()?;
        let current = std::sync::Arc::new(std::sync::RwLock::new(initial));
        let (event_tx, mut event_rx) =
            tokio::sync::mpsc::unbounded_channel::<notify::Result<notify::Event>>();
        let (update_tx, updates) = tokio::sync::mpsc::unbounded_channel();

        let mut watcher = notify::recommended_watcher(move |event| {
            let _ = event_tx.send(event);
        })
        .map_err(|err| ConfigError::Watch(err.to_string()))?;

        for file in &builder.files {
            watcher
                .watch(file, notify::RecursiveMode::NonRecursive)
                .map_err(|err| ConfigError::Watch(err.to_string()))?;
        }

        let current_task = std::sync::Arc::clone(&current);
        tokio::spawn(async move {
            while let Some(event) = event_rx.recv().await {
                if let Err(err) = event {
                    let _ = update_tx.send(Err(ConfigError::Watch(err.to_string())));
                    continue;
                }

                tokio::time::sleep(Duration::from_millis(200)).await;
                while event_rx.try_recv().is_ok() {}

                match builder.build_inner() {
                    Ok(config) => {
                        if let Ok(mut current) = current_task.write() {
                            *current = config.clone();
                        }
                        let _ = update_tx.send(Ok(config));
                    }
                    Err(err) => {
                        let _ = update_tx.send(Err(err));
                    }
                }
            }
        });

        Ok(Self {
            current,
            updates,
            _watcher: watcher,
        })
    }

    /// Return the last successfully loaded config.
    pub fn current(&self) -> ArvikConfig {
        self.current
            .read()
            .expect("config watcher lock poisoned")
            .clone()
    }

    /// Wait for the next reload result.
    ///
    /// Failed reloads are returned as errors and do not replace
    /// [`current`](Self::current).
    pub async fn next(&mut self) -> Result<ArvikConfig> {
        self.updates
            .recv()
            .await
            .unwrap_or_else(|| Err(ConfigError::Watch("config watcher stopped".into())))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};

    static NEXT_ENV_ID: AtomicUsize = AtomicUsize::new(0);

    fn unique_prefix() -> String {
        format!(
            "ARVIK_TEST_{}_{}",
            std::process::id(),
            NEXT_ENV_ID.fetch_add(1, Ordering::SeqCst)
        )
    }

    #[test]
    fn defaults_are_valid() {
        let config = ArvikConfig::builder().build().unwrap();
        assert_eq!(config.server.host, "127.0.0.1");
        assert_eq!(config.server.port, 8080);
        assert_eq!(config.bind_addr_string(), "127.0.0.1:8080");
        config.bind_addr().unwrap();
    }

    #[test]
    fn toml_file_overlays_defaults() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("arvik.toml");
        std::fs::write(
            &path,
            r#"
                [server]
                host = "0.0.0.0"
                port = 3000
                max_connections = 128

                [http2]
                only = true
                max_concurrent_streams = 64

                [shutdown]
                drain_timeout_secs = 5
            "#,
        )
        .unwrap();

        let config = ArvikConfig::builder().file(&path).build().unwrap();
        assert_eq!(config.server.host, "0.0.0.0");
        assert_eq!(config.server.port, 3000);
        assert_eq!(config.server.max_connections, Some(128));
        assert!(config.server_config().http2_only_enabled());
        assert_eq!(
            config.server_config().http2_max_concurrent_streams_limit(),
            Some(64)
        );
        assert_eq!(
            config.shutdown_config().drain_timeout_value(),
            Duration::from_secs(5)
        );
    }

    #[test]
    fn json_file_loads() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("arvik.json");
        std::fs::write(
            &path,
            r#"{"server":{"host":"127.0.0.2","port":4040},"http1":{"keep_alive":false}}"#,
        )
        .unwrap();

        let config = ArvikConfig::builder().file(&path).build().unwrap();
        assert_eq!(config.server.host, "127.0.0.2");
        assert_eq!(config.server.port, 4040);
        assert!(!config.server_config().http1_keep_alive_enabled());
    }

    #[test]
    fn env_overrides_files() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("arvik.toml");
        std::fs::write(&path, "[server]\nport = 3000\n").unwrap();
        let prefix = unique_prefix();
        let key = format!("{prefix}_SERVER_PORT");

        unsafe {
            std::env::set_var(&key, "9090");
        }
        let config = ArvikConfig::builder()
            .env_prefix(&prefix)
            .file(&path)
            .build()
            .unwrap();
        unsafe {
            std::env::remove_var(&key);
        }

        assert_eq!(config.server.port, 9090);
    }

    #[test]
    fn validation_reports_bad_limits() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("arvik.toml");
        std::fs::write(&path, "[server]\nmax_connections = 0\n").unwrap();

        let err = ArvikConfig::builder().file(&path).build().unwrap_err();
        assert!(err.to_string().contains("server.max_connections"));
    }

    #[test]
    fn server_socket_fields_map_to_server_config() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("arvik.toml");
        std::fs::write(
            &path,
            r#"
                [server]
                tcp_nodelay = true
                tcp_keepalive_secs = 60
                tcp_keepalive_interval_secs = 5
                tcp_keepalive_retries = 3
                reuse_port = true
                reuse_address = true
                backlog = 4096
                socket_recv_buffer_size = 262144
                socket_send_buffer_size = 524288
                accept_workers = 2
            "#,
        )
        .unwrap();

        let config = ArvikConfig::builder().file(&path).build().unwrap();
        let server = config.server_config();
        assert_eq!(server.tcp_nodelay_setting(), Some(true));
        assert_eq!(
            server.tcp_keepalive_duration(),
            Some(Duration::from_secs(60))
        );
        assert_eq!(
            server.tcp_keepalive_interval_duration(),
            Some(Duration::from_secs(5))
        );
        assert_eq!(server.tcp_keepalive_retries_count(), Some(3));
        assert!(server.reuse_port_enabled());
        assert!(server.reuse_address_enabled());
        assert_eq!(server.backlog_size(), Some(4096));
        assert_eq!(server.socket_recv_buffer_size_value(), Some(262144));
        assert_eq!(server.socket_send_buffer_size_value(), Some(524288));
        assert_eq!(server.accept_workers_count(), 2);
    }

    #[test]
    fn accept_workers_requires_reuse_port_in_config() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("arvik.toml");
        std::fs::write(&path, "[server]\naccept_workers = 2\n").unwrap();

        let err = ArvikConfig::builder().file(&path).build().unwrap_err();
        assert!(err.to_string().contains("requires server.reuse_port"));
    }

    #[test]
    fn tls_requires_backend_files() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("arvik.toml");
        std::fs::write(&path, "[tls]\nbackend = \"rustls\"\n").unwrap();

        let err = ArvikConfig::builder().file(&path).build().unwrap_err();
        assert!(err.to_string().contains("tls.cert_path"));
    }

    #[cfg(feature = "hot-reload")]
    #[tokio::test]
    async fn watcher_preserves_current_config_after_failed_reload() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("arvik.toml");
        std::fs::write(&path, "[server]\nport = 3001\n").unwrap();

        let mut watcher = ArvikConfig::builder().file(&path).watch().unwrap();
        assert_eq!(watcher.current().server.port, 3001);

        std::fs::write(&path, "[server]\nport = 0\n").unwrap();
        let err = watcher.next().await.unwrap_err();

        assert!(err.to_string().contains("server.port"));
        assert_eq!(watcher.current().server.port, 3001);
    }
}
