//! Low-level server and protocol tuning.
//!
//! `ServerConfig` intentionally covers only runtime/server knobs. File/env
//! configuration belongs to the later Arvik configuration system.

use std::time::Duration;

use hyper_util::rt::{TokioExecutor, TokioTimer};
use hyper_util::server::conn::auto::Builder;

/// Low-level HTTP server tuning.
#[derive(Debug, Clone)]
pub struct ServerConfig {
    max_connections: Option<usize>,
    tcp_nodelay: Option<bool>,
    tcp_keepalive: Option<Duration>,
    tcp_keepalive_interval: Option<Duration>,
    tcp_keepalive_retries: Option<u32>,
    reuse_port: bool,
    reuse_address: bool,
    backlog: Option<i32>,
    socket_recv_buffer_size: Option<usize>,
    socket_send_buffer_size: Option<usize>,
    accept_workers: usize,
    http1_keep_alive: bool,
    http1_half_close: bool,
    http1_title_case_headers: bool,
    http1_preserve_header_case: bool,
    http1_pipeline_flush: bool,
    /// Upper bound on reading request headers (slowloris hardening).
    http1_header_read_timeout: Option<Duration>,
    http1_max_buf_size: Option<usize>,
    /// Upper bound on the TLS handshake phase (`None` disables the guard).
    handshake_timeout: Option<Duration>,
    http2_only: bool,
    http2_adaptive_window: bool,
    http2_initial_stream_window_size: Option<u32>,
    http2_initial_connection_window_size: Option<u32>,
    http2_max_concurrent_streams: Option<u32>,
    http2_keep_alive_interval: Option<Duration>,
    http2_keep_alive_timeout: Duration,
    http2_max_frame_size: Option<u32>,
    http2_max_send_buf_size: Option<usize>,
    http2_max_header_list_size: Option<u32>,
}

impl Default for ServerConfig {
    fn default() -> Self {
        Self {
            max_connections: None,
            tcp_nodelay: None,
            tcp_keepalive: None,
            tcp_keepalive_interval: None,
            tcp_keepalive_retries: None,
            reuse_port: false,
            reuse_address: false,
            backlog: None,
            socket_recv_buffer_size: None,
            socket_send_buffer_size: None,
            accept_workers: 1,
            http1_keep_alive: true,
            http1_half_close: false,
            http1_title_case_headers: false,
            http1_preserve_header_case: false,
            http1_pipeline_flush: false,
            http1_header_read_timeout: None,
            http1_max_buf_size: None,
            handshake_timeout: Some(Duration::from_secs(10)),
            http2_only: false,
            http2_adaptive_window: true,
            http2_initial_stream_window_size: None,
            http2_initial_connection_window_size: None,
            http2_max_concurrent_streams: Some(1_000),
            http2_keep_alive_interval: Some(Duration::from_secs(20)),
            http2_keep_alive_timeout: Duration::from_secs(10),
            http2_max_frame_size: None,
            http2_max_send_buf_size: None,
            http2_max_header_list_size: Some(64 * 1024),
        }
    }
}

impl ServerConfig {
    /// Create a default server config.
    pub fn new() -> Self {
        Self::default()
    }

    /// Create a config tuned for low-latency HTTP/2 responses.
    pub fn http2_low_latency() -> Self {
        Self::default()
            .http2_adaptive_window(true)
            .http2_max_concurrent_streams(256)
            .http2_max_send_buf_size(256 * 1024)
            .http2_keep_alive_interval(Duration::from_secs(15))
            .http2_keep_alive_timeout(Duration::from_secs(5))
            .tcp_nodelay(true)
    }

    /// Create a config tuned for high-throughput HTTP/2 workloads.
    pub fn http2_high_throughput() -> Self {
        Self::default()
            .http2_adaptive_window(true)
            .http2_max_concurrent_streams(2_000)
            .http2_initial_stream_window_size(1_048_576)
            .http2_initial_connection_window_size(8_388_608)
            .http2_max_send_buf_size(1_048_576)
            .http2_keep_alive_interval(Duration::from_secs(20))
            .http2_keep_alive_timeout(Duration::from_secs(10))
            .tcp_nodelay(true)
    }

    /// Set the maximum number of concurrently accepted connections.
    ///
    /// Connections accepted while this limit is reached are closed immediately.
    #[must_use]
    pub fn max_connections(mut self, max: usize) -> Self {
        self.max_connections = Some(max);
        self
    }

    /// Disable the connection admission limit.
    #[must_use]
    pub fn unlimited_connections(mut self) -> Self {
        self.max_connections = None;
        self
    }

    /// Return the configured concurrent connection limit.
    pub fn max_connections_limit(&self) -> Option<usize> {
        self.max_connections
    }

    /// Return the configured TCP_NODELAY setting, if explicitly set.
    pub fn tcp_nodelay_setting(&self) -> Option<bool> {
        self.tcp_nodelay
    }

    /// Return the configured TCP keepalive idle duration.
    pub fn tcp_keepalive_duration(&self) -> Option<Duration> {
        self.tcp_keepalive
    }

    /// Return the configured TCP keepalive probe interval.
    pub fn tcp_keepalive_interval_duration(&self) -> Option<Duration> {
        self.tcp_keepalive_interval
    }

    /// Return the configured TCP keepalive retry count.
    pub fn tcp_keepalive_retries_count(&self) -> Option<u32> {
        self.tcp_keepalive_retries
    }

    /// Return whether SO_REUSEPORT is requested.
    pub fn reuse_port_enabled(&self) -> bool {
        self.reuse_port
    }

    /// Return whether SO_REUSEADDR is requested.
    pub fn reuse_address_enabled(&self) -> bool {
        self.reuse_address
    }

    /// Return the configured listen backlog.
    pub fn backlog_size(&self) -> Option<i32> {
        self.backlog
    }

    /// Return the configured receive buffer size.
    pub fn socket_recv_buffer_size_value(&self) -> Option<usize> {
        self.socket_recv_buffer_size
    }

    /// Return the configured send buffer size.
    pub fn socket_send_buffer_size_value(&self) -> Option<usize> {
        self.socket_send_buffer_size
    }

    /// Return the number of listener accept workers.
    pub fn accept_workers_count(&self) -> usize {
        self.accept_workers
    }

    /// Return whether HTTP/1 keep-alive is enabled.
    pub fn http1_keep_alive_enabled(&self) -> bool {
        self.http1_keep_alive
    }

    /// Return whether HTTP/2-only mode is enabled.
    pub fn http2_only_enabled(&self) -> bool {
        self.http2_only
    }

    /// Return the HTTP/2 max concurrent streams setting.
    pub fn http2_max_concurrent_streams_limit(&self) -> Option<u32> {
        self.http2_max_concurrent_streams
    }

    /// Enable or disable TCP_NODELAY on accepted TCP sockets.
    #[must_use]
    pub fn tcp_nodelay(mut self, enabled: bool) -> Self {
        self.tcp_nodelay = Some(enabled);
        self
    }

    /// Set TCP keepalive idle duration on accepted TCP sockets.
    #[must_use]
    pub fn tcp_keepalive(mut self, duration: Duration) -> Self {
        self.tcp_keepalive = Some(duration);
        self
    }

    /// Set TCP keepalive probe interval on accepted TCP sockets.
    #[must_use]
    pub fn tcp_keepalive_interval(mut self, duration: Duration) -> Self {
        self.tcp_keepalive_interval = Some(duration);
        self
    }

    /// Set TCP keepalive retry count where the operating system supports it.
    #[must_use]
    pub fn tcp_keepalive_retries(mut self, retries: u32) -> Self {
        self.tcp_keepalive_retries = Some(retries);
        self
    }

    /// Enable or disable SO_REUSEPORT on listener sockets.
    #[must_use]
    pub fn reuse_port(mut self, enabled: bool) -> Self {
        self.reuse_port = enabled;
        self
    }

    /// Enable or disable SO_REUSEADDR on listener sockets.
    #[must_use]
    pub fn reuse_address(mut self, enabled: bool) -> Self {
        self.reuse_address = enabled;
        self
    }

    /// Set the listen backlog used by tuned listener sockets.
    #[must_use]
    pub fn backlog(mut self, backlog: i32) -> Self {
        self.backlog = Some(backlog);
        self
    }

    /// Set SO_RCVBUF on listener sockets.
    #[must_use]
    pub fn socket_recv_buffer_size(mut self, bytes: usize) -> Self {
        self.socket_recv_buffer_size = Some(bytes);
        self
    }

    /// Set SO_SNDBUF on listener sockets.
    #[must_use]
    pub fn socket_send_buffer_size(mut self, bytes: usize) -> Self {
        self.socket_send_buffer_size = Some(bytes);
        self
    }

    /// Set the number of listener accept workers.
    ///
    /// Values greater than one require [`ServerConfig::reuse_port`] to be
    /// enabled so each worker owns its own listener socket.
    #[must_use]
    pub fn accept_workers(mut self, workers: usize) -> Self {
        self.accept_workers = workers.max(1);
        self
    }

    /// Set one accept worker per available CPU.
    #[must_use]
    pub fn accept_workers_per_cpu(self) -> Self {
        let workers = std::thread::available_parallelism()
            .map(usize::from)
            .unwrap_or(1);
        self.accept_workers(workers)
    }

    /// Enable or disable HTTP/1 keep-alive.
    #[must_use]
    pub fn http1_keepalive(self, enabled: bool) -> Self {
        self.http1_keep_alive(enabled)
    }

    /// Enable or disable HTTP/1 keep-alive.
    #[must_use]
    pub fn http1_keep_alive(mut self, enabled: bool) -> Self {
        self.http1_keep_alive = enabled;
        self
    }

    /// Enable or disable HTTP/1 half-close support.
    #[must_use]
    pub fn http1_half_close(mut self, enabled: bool) -> Self {
        self.http1_half_close = enabled;
        self
    }

    /// Write HTTP/1 response header names in title case.
    #[must_use]
    pub fn http1_title_case_headers(mut self, enabled: bool) -> Self {
        self.http1_title_case_headers = enabled;
        self
    }

    /// Preserve original HTTP/1 header casing where Hyper supports it.
    #[must_use]
    pub fn http1_preserve_header_case(mut self, enabled: bool) -> Self {
        self.http1_preserve_header_case = enabled;
        self
    }

    /// Aggregate HTTP/1 flushes for pipelined responses.
    #[must_use]
    pub fn http1_pipeline_flush(mut self, enabled: bool) -> Self {
        self.http1_pipeline_flush = enabled;
        self
    }

    /// Upper bound on reading request headers.
    ///
    /// Hardens against slow-loris clients that dribble headers forever
    /// (hyper's own default is 30 s; `None` keeps that default).
    #[must_use]
    pub fn http1_header_read_timeout(mut self, timeout: impl Into<Option<Duration>>) -> Self {
        self.http1_header_read_timeout = timeout.into();
        self
    }

    /// Cap the per-connection HTTP/1 read buffer size in bytes.
    #[must_use]
    pub fn http1_max_buf_size(mut self, size: usize) -> Self {
        self.http1_max_buf_size = Some(size);
        self
    }

    /// Upper bound on the TLS handshake phase (default 10 s).
    ///
    /// Clients that open a connection but never complete the handshake are
    /// disconnected after this duration instead of holding a task and socket
    /// indefinitely. Pass `None` to disable the guard entirely.
    #[must_use]
    pub fn handshake_timeout(mut self, timeout: impl Into<Option<Duration>>) -> Self {
        self.handshake_timeout = timeout.into();
        self
    }

    /// Accept HTTP/2 prior-knowledge connections only.
    #[must_use]
    pub fn http2_only(mut self, enabled: bool) -> Self {
        self.http2_only = enabled;
        self
    }

    /// Enable or disable HTTP/2 adaptive flow control.
    #[must_use]
    pub fn http2_adaptive_window(mut self, enabled: bool) -> Self {
        self.http2_adaptive_window = enabled;
        self
    }

    /// Set HTTP/2 stream-level initial window size.
    #[must_use]
    pub fn http2_initial_stream_window_size(mut self, size: u32) -> Self {
        self.http2_initial_stream_window_size = Some(size);
        self
    }

    /// Set HTTP/2 connection-level initial window size.
    #[must_use]
    pub fn http2_initial_connection_window_size(mut self, size: u32) -> Self {
        self.http2_initial_connection_window_size = Some(size);
        self
    }

    /// Set HTTP/2 max concurrent streams.
    #[must_use]
    pub fn http2_max_concurrent_streams(mut self, max: u32) -> Self {
        self.http2_max_concurrent_streams = Some(max);
        self
    }

    /// Disable Hyper's HTTP/2 max concurrent stream limit.
    #[must_use]
    pub fn http2_unlimited_concurrent_streams(mut self) -> Self {
        self.http2_max_concurrent_streams = None;
        self
    }

    /// Set HTTP/2 keep-alive interval.
    #[must_use]
    pub fn http2_keep_alive_interval(mut self, interval: Duration) -> Self {
        self.http2_keep_alive_interval = Some(interval);
        self
    }

    /// Disable HTTP/2 keep-alive pings.
    #[must_use]
    pub fn http2_disable_keep_alive(mut self) -> Self {
        self.http2_keep_alive_interval = None;
        self
    }

    /// Set HTTP/2 keep-alive acknowledgement timeout.
    #[must_use]
    pub fn http2_keep_alive_timeout(mut self, timeout: Duration) -> Self {
        self.http2_keep_alive_timeout = timeout;
        self
    }

    /// Set HTTP/2 max frame size.
    #[must_use]
    pub fn http2_max_frame_size(mut self, size: u32) -> Self {
        self.http2_max_frame_size = Some(size);
        self
    }

    /// Set HTTP/2 max send buffer size per stream.
    #[must_use]
    pub fn http2_max_send_buf_size(mut self, size: usize) -> Self {
        self.http2_max_send_buf_size = Some(size);
        self
    }

    /// Set HTTP/2 max header list size.
    #[must_use]
    pub fn http2_max_header_list_size(mut self, size: u32) -> Self {
        self.http2_max_header_list_size = Some(size);
        self
    }

    pub(crate) fn is_http2_only(&self) -> bool {
        self.http2_only
    }

    pub(crate) fn http1_header_read_timeout_value(&self) -> Option<Duration> {
        self.http1_header_read_timeout
    }

    pub(crate) fn http1_max_buf_size_value(&self) -> Option<usize> {
        self.http1_max_buf_size
    }

    #[cfg_attr(not(any(feature = "tls", feature = "native-tls")), allow(dead_code))]
    pub(crate) fn handshake_timeout_value(&self) -> Option<Duration> {
        self.handshake_timeout
    }

    pub(crate) fn needs_tuned_listener(&self) -> bool {
        self.reuse_port
            || self.reuse_address
            || self.backlog.is_some()
            || self.socket_recv_buffer_size.is_some()
            || self.socket_send_buffer_size.is_some()
            || self.accept_workers > 1
    }

    pub(crate) fn auto_builder(&self) -> Builder<TokioExecutor> {
        let mut builder = Builder::new(TokioExecutor::new());

        {
            let mut http1 = builder.http1();
            http1.keep_alive(self.http1_keep_alive);
            http1.half_close(self.http1_half_close);
            http1.title_case_headers(self.http1_title_case_headers);
            http1.preserve_header_case(self.http1_preserve_header_case);
            http1.pipeline_flush(self.http1_pipeline_flush);
            if let Some(timeout) = self.http1_header_read_timeout_value() {
                http1.header_read_timeout(timeout);
            }
            if let Some(size) = self.http1_max_buf_size_value() {
                http1.max_buf_size(size);
            }
            http1.timer(TokioTimer::new());
        }

        {
            let mut http2 = builder.http2();
            http2.timer(TokioTimer::new());
            http2.adaptive_window(self.http2_adaptive_window);
            http2.initial_stream_window_size(self.http2_initial_stream_window_size);
            http2.initial_connection_window_size(self.http2_initial_connection_window_size);
            http2.max_concurrent_streams(self.http2_max_concurrent_streams);
            http2.keep_alive_interval(self.http2_keep_alive_interval);
            http2.keep_alive_timeout(self.http2_keep_alive_timeout);
            http2.max_frame_size(self.http2_max_frame_size);
            if let Some(size) = self.http2_max_send_buf_size {
                http2.max_send_buf_size(size);
            }
            if let Some(size) = self.http2_max_header_list_size {
                http2.max_header_list_size(size);
            }
        }

        if self.http2_only {
            builder = builder.http2_only();
        }

        builder
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builder_methods_store_http2_only() {
        let config = ServerConfig::new()
            .max_connections(64)
            .http2_only(true)
            .http2_max_concurrent_streams(500)
            .http2_keep_alive_interval(Duration::from_secs(5));

        assert!(config.is_http2_only());
        assert_eq!(config.max_connections_limit(), Some(64));
        assert_eq!(config.http2_max_concurrent_streams, Some(500));
        assert_eq!(
            config.http2_keep_alive_interval,
            Some(Duration::from_secs(5))
        );
    }

    #[test]
    fn builder_methods_store_socket_options() {
        let config = ServerConfig::new()
            .tcp_nodelay(true)
            .tcp_keepalive(Duration::from_secs(60))
            .tcp_keepalive_interval(Duration::from_secs(5))
            .tcp_keepalive_retries(3)
            .reuse_port(true)
            .reuse_address(true)
            .backlog(4096)
            .socket_recv_buffer_size(256 * 1024)
            .socket_send_buffer_size(512 * 1024)
            .accept_workers(4);

        assert_eq!(config.tcp_nodelay_setting(), Some(true));
        assert_eq!(
            config.tcp_keepalive_duration(),
            Some(Duration::from_secs(60))
        );
        assert_eq!(
            config.tcp_keepalive_interval_duration(),
            Some(Duration::from_secs(5))
        );
        assert_eq!(config.tcp_keepalive_retries_count(), Some(3));
        assert!(config.reuse_port_enabled());
        assert!(config.reuse_address_enabled());
        assert_eq!(config.backlog_size(), Some(4096));
        assert_eq!(config.socket_recv_buffer_size_value(), Some(256 * 1024));
        assert_eq!(config.socket_send_buffer_size_value(), Some(512 * 1024));
        assert_eq!(config.accept_workers_count(), 4);
        assert!(config.needs_tuned_listener());
    }

    #[test]
    fn http2_presets_store_expected_values() {
        let low = ServerConfig::http2_low_latency();
        assert_eq!(low.tcp_nodelay_setting(), Some(true));
        assert_eq!(low.http2_max_concurrent_streams, Some(256));

        let high = ServerConfig::http2_high_throughput();
        assert_eq!(high.tcp_nodelay_setting(), Some(true));
        assert_eq!(high.http2_max_concurrent_streams, Some(2_000));
        assert_eq!(high.http2_initial_connection_window_size, Some(8_388_608));
    }

    #[test]
    fn handshake_and_http1_timeout_knobs() {
        // Defaults: 10 s handshake guard, hyper defaults for HTTP/1 knobs.
        let config = ServerConfig::new();
        assert_eq!(
            config.handshake_timeout_value(),
            Some(Duration::from_secs(10))
        );
        assert_eq!(config.http1_header_read_timeout_value(), None);
        assert_eq!(config.http1_max_buf_size_value(), None);

        let tuned = ServerConfig::new()
            .handshake_timeout(Duration::from_secs(3))
            .http1_header_read_timeout(Duration::from_secs(15))
            .http1_max_buf_size(64 * 1024);
        assert_eq!(
            tuned.handshake_timeout_value(),
            Some(Duration::from_secs(3))
        );
        assert_eq!(
            tuned.http1_header_read_timeout_value(),
            Some(Duration::from_secs(15))
        );
        assert_eq!(tuned.http1_max_buf_size_value(), Some(64 * 1024));

        let disabled = ServerConfig::new().handshake_timeout(None);
        assert_eq!(disabled.handshake_timeout_value(), None);
    }
}
