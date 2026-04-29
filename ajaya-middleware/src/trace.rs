//! Structured request/response tracing middleware.
//!
//! Creates a tracing span per request containing method, path, status,
//! and latency. Integrates with the `tracing` ecosystem for structured
//! logging and distributed tracing.
//!
//! # Example
//!
//! ```rust,ignore
//! use ajaya_middleware::trace::{TraceLayer, DefaultMakeSpan, LatencyUnit};
//! use tracing::Level;
//!
//! Router::new()
//!     .route("/", get(handler))
//!     .layer(TraceLayer::new_for_http());
//!
//! // Custom configuration
//! Router::new()
//!     .layer(
//!         TraceLayer::new_for_http()
//!             .make_span_with(DefaultMakeSpan::new().level(Level::DEBUG))
//!             .latency_unit(LatencyUnit::Micros),
//!     );
//! ```

use std::convert::Infallible;
use std::future::Future;
use std::pin::Pin;
use std::task::{Context, Poll};
use std::time::Instant;

use ajaya_core::{Request, Response};
use tower_layer::Layer;
use tower_service::Service;
use tracing::{Level, Span};

// ── LatencyUnit ───────────────────────────────────────────────────────────────

/// Unit for reporting request latency.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum LatencyUnit {
    /// Milliseconds (default).
    #[default]
    Millis,
    /// Microseconds.
    Micros,
    /// Seconds (floating point).
    Seconds,
}

impl LatencyUnit {
    fn format(self, elapsed: std::time::Duration) -> String {
        match self {
            LatencyUnit::Millis => format!("{}ms", elapsed.as_millis()),
            LatencyUnit::Micros => format!("{}µs", elapsed.as_micros()),
            LatencyUnit::Seconds => format!("{:.3}s", elapsed.as_secs_f64()),
        }
    }
}

// ── DefaultMakeSpan ───────────────────────────────────────────────────────────

/// The default span factory — creates one span per request.
#[derive(Debug, Clone)]
pub struct DefaultMakeSpan {
    level: Level,
    include_headers: bool,
}

impl Default for DefaultMakeSpan {
    fn default() -> Self {
        Self {
            level: Level::INFO,
            include_headers: false,
        }
    }
}

impl DefaultMakeSpan {
    /// Create a new `DefaultMakeSpan`.
    pub fn new() -> Self {
        Self::default()
    }

    /// Set the tracing level for the span.
    pub fn level(mut self, level: Level) -> Self {
        self.level = level;
        self
    }

    /// Include request headers as span fields.
    pub fn include_headers(mut self, include: bool) -> Self {
        self.include_headers = include;
        self
    }

    fn make_span(&self, req: &Request) -> Span {
        let method = req.method().as_str().to_owned();
        let path = req.uri().path().to_owned();
        let version = format!("{:?}", req.version());

        match self.level {
            Level::ERROR => tracing::error_span!(
                "request",
                http.method = %method,
                http.path = %path,
                http.version = %version,
                http.status_code = tracing::field::Empty,
                latency = tracing::field::Empty,
            ),
            Level::WARN => tracing::warn_span!(
                "request",
                http.method = %method,
                http.path = %path,
                http.version = %version,
                http.status_code = tracing::field::Empty,
                latency = tracing::field::Empty,
            ),
            Level::DEBUG => tracing::debug_span!(
                "request",
                http.method = %method,
                http.path = %path,
                http.version = %version,
                http.status_code = tracing::field::Empty,
                latency = tracing::field::Empty,
            ),
            Level::TRACE => tracing::trace_span!(
                "request",
                http.method = %method,
                http.path = %path,
                http.version = %version,
                http.status_code = tracing::field::Empty,
                latency = tracing::field::Empty,
            ),
            _ => tracing::info_span!(
                "request",
                http.method = %method,
                http.path = %path,
                http.version = %version,
                http.status_code = tracing::field::Empty,
                latency = tracing::field::Empty,
            ),
        }
    }
}

// ── TraceLayer ────────────────────────────────────────────────────────────────

/// Tower layer that creates a tracing span per request.
#[derive(Clone)]
pub struct TraceLayer {
    make_span: DefaultMakeSpan,
    latency_unit: LatencyUnit,
    log_on_failure: bool,
}

impl Default for TraceLayer {
    fn default() -> Self {
        Self {
            make_span: DefaultMakeSpan::default(),
            latency_unit: LatencyUnit::default(),
            log_on_failure: true,
        }
    }
}

impl TraceLayer {
    /// Create a `TraceLayer` suitable for HTTP servers.
    pub fn new_for_http() -> Self {
        Self::default()
    }

    /// Customize the span factory.
    pub fn make_span_with(mut self, make_span: DefaultMakeSpan) -> Self {
        self.make_span = make_span;
        self
    }

    /// Set the latency unit for reporting response times.
    pub fn latency_unit(mut self, unit: LatencyUnit) -> Self {
        self.latency_unit = unit;
        self
    }

    /// Whether to log at ERROR level on 5xx responses.
    pub fn log_failures(mut self, log: bool) -> Self {
        self.log_on_failure = log;
        self
    }
}

impl<S> Layer<S> for TraceLayer {
    type Service = TraceService<S>;

    fn layer(&self, inner: S) -> Self::Service {
        TraceService {
            inner,
            make_span: self.make_span.clone(),
            latency_unit: self.latency_unit,
            log_on_failure: self.log_on_failure,
        }
    }
}

/// Tower service produced by [`TraceLayer`].
#[derive(Clone)]
pub struct TraceService<S> {
    inner: S,
    make_span: DefaultMakeSpan,
    latency_unit: LatencyUnit,
    log_on_failure: bool,
}

impl<S> Service<Request> for TraceService<S>
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
        let span = self.make_span.make_span(&req);
        let cloned = self.inner.clone();
        let mut inner = std::mem::replace(&mut self.inner, cloned);
        let latency_unit = self.latency_unit;
        let log_on_failure = self.log_on_failure;

        Box::pin(async move {
            let start = Instant::now();
            let response = {
                let _guard = span.enter();
                inner.call(req).await?
            };

            let elapsed = start.elapsed();
            let status = response.status();
            let latency_str = latency_unit.format(elapsed);

            span.record("http.status_code", status.as_u16());
            span.record("latency", latency_str.as_str());

            if status.is_server_error() && log_on_failure {
                tracing::error!(
                    parent: &span,
                    status = status.as_u16(),
                    latency = %latency_str,
                    "server error"
                );
            } else if status.is_client_error() {
                tracing::warn!(
                    parent: &span,
                    status = status.as_u16(),
                    latency = %latency_str,
                    "client error"
                );
            } else {
                tracing::info!(
                    parent: &span,
                    status = status.as_u16(),
                    latency = %latency_str,
                    "response"
                );
            }

            Ok(response)
        })
    }
}
