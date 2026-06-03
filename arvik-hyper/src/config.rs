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
    http1_keep_alive: bool,
    http1_half_close: bool,
    http1_title_case_headers: bool,
    http1_preserve_header_case: bool,
    http1_pipeline_flush: bool,
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
            http1_keep_alive: true,
            http1_half_close: false,
            http1_title_case_headers: false,
            http1_preserve_header_case: false,
            http1_pipeline_flush: false,
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

    pub(crate) fn auto_builder(&self) -> Builder<TokioExecutor> {
        let mut builder = Builder::new(TokioExecutor::new());

        {
            let mut http1 = builder.http1();
            http1.keep_alive(self.http1_keep_alive);
            http1.half_close(self.http1_half_close);
            http1.title_case_headers(self.http1_title_case_headers);
            http1.preserve_header_case(self.http1_preserve_header_case);
            http1.pipeline_flush(self.http1_pipeline_flush);
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
            .http2_only(true)
            .http2_max_concurrent_streams(500)
            .http2_keep_alive_interval(Duration::from_secs(5));

        assert!(config.is_http2_only());
        assert_eq!(config.http2_max_concurrent_streams, Some(500));
        assert_eq!(
            config.http2_keep_alive_interval,
            Some(Duration::from_secs(5))
        );
    }
}
