//! Structured logging setup and request logging middleware.

use std::collections::BTreeMap;
use std::convert::Infallible;
use std::error::Error;
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;
use std::task::{Context, Poll};
use std::time::Instant;

use arvik_core::{Request, Response};
use arvik_middleware::request_id::{RequestId, X_REQUEST_ID};
use arvik_router::MatchedPathExt;
use http::header::{AUTHORIZATION, COOKIE, HeaderName, HeaderValue, SET_COOKIE};
use tower_layer::Layer;
use tower_service::Service;
use tracing::{Instrument, Span};
use tracing_subscriber::EnvFilter;
use uuid::Uuid;

/// Error type returned by logger initialization.
pub type LoggerError = Box<dyn Error + Send + Sync + 'static>;

/// Process-wide logger setup entrypoint.
pub struct ArvikLogger;

impl ArvikLogger {
    /// Initialize the global tracing subscriber with sensible defaults.
    pub fn init() -> Result<(), LoggerError> {
        Self::builder().init()
    }

    /// Create a configurable logger builder.
    pub fn builder() -> ArvikLoggerBuilder {
        ArvikLoggerBuilder::default()
    }
}

/// Builder for [`ArvikLogger`].
#[derive(Debug, Clone)]
pub struct ArvikLoggerBuilder {
    format: LogFormat,
    env_filter: Option<String>,
    with_target: bool,
    with_thread_ids: bool,
}

impl Default for ArvikLoggerBuilder {
    fn default() -> Self {
        Self {
            format: LogFormat::Auto,
            env_filter: None,
            with_target: true,
            with_thread_ids: false,
        }
    }
}

impl ArvikLoggerBuilder {
    /// Set the output format.
    #[must_use]
    pub fn format(mut self, format: LogFormat) -> Self {
        self.format = format;
        self
    }

    /// Set an explicit env-filter directive. Defaults to `RUST_LOG`, then `info`.
    #[must_use]
    pub fn env_filter(mut self, directive: impl Into<String>) -> Self {
        self.env_filter = Some(directive.into());
        self
    }

    /// Include or omit tracing targets.
    #[must_use]
    pub fn with_target(mut self, enabled: bool) -> Self {
        self.with_target = enabled;
        self
    }

    /// Include or omit thread IDs.
    #[must_use]
    pub fn with_thread_ids(mut self, enabled: bool) -> Self {
        self.with_thread_ids = enabled;
        self
    }

    /// Install the configured tracing subscriber.
    pub fn init(self) -> Result<(), LoggerError> {
        let format = self.format.resolve();
        let directive = self.env_filter_directive();
        let env_filter = EnvFilter::try_new(directive)?;

        match format {
            LogFormat::Json => tracing_subscriber::fmt()
                .with_env_filter(env_filter)
                .with_target(self.with_target)
                .with_thread_ids(self.with_thread_ids)
                .json()
                .try_init(),
            LogFormat::Pretty => tracing_subscriber::fmt()
                .with_env_filter(env_filter)
                .with_target(self.with_target)
                .with_thread_ids(self.with_thread_ids)
                .pretty()
                .try_init(),
            LogFormat::Auto => unreachable!("LogFormat::resolve never returns Auto"),
        }
        .map_err(Into::into)
    }

    /// Install logging and OpenTelemetry as **one** subscriber.
    ///
    /// [`ArvikLogger::init`] and [`OtelConfig::install`](crate::trace::OtelConfig::install)
    /// each try to claim the global subscriber, so calling both always fails
    /// for whichever runs second. This method composes the log layer and the
    /// OTel span layer into a single subscriber instead. The returned guard
    /// shuts the tracer provider down on drop; on failure nothing is left
    /// installed.
    #[cfg(all(feature = "logging", feature = "opentelemetry"))]
    pub fn init_with_otel(
        self,
        otel: crate::trace::OtelConfig,
    ) -> Result<crate::trace::OtelGuard, LoggerError> {
        use opentelemetry::global;
        use opentelemetry::trace::TracerProvider as _;
        use tracing_subscriber::layer::SubscriberExt;
        use tracing_subscriber::util::SubscriberInitExt;

        let provider = otel
            .build_provider()
            .map_err(|err| -> LoggerError { Box::new(err) })?;

        let format = self.format.resolve();
        let env_filter = EnvFilter::try_new(self.env_filter_directive())?;

        let fmt_layer = tracing_subscriber::fmt::layer()
            .with_target(self.with_target)
            .with_thread_ids(self.with_thread_ids);

        // The Json/Pretty layers have distinct types, so each arm composes and
        // installs its own subscriber.
        let install_result = match format {
            LogFormat::Json => {
                let otel_layer = tracing_opentelemetry::layer()
                    .with_tracer(provider.tracer(otel.service_name().to_string()));
                tracing_subscriber::registry()
                    .with(env_filter)
                    .with(fmt_layer.json())
                    .with(otel_layer)
                    .try_init()
            }
            LogFormat::Pretty => {
                let otel_layer = tracing_opentelemetry::layer()
                    .with_tracer(provider.tracer(otel.service_name().to_string()));
                tracing_subscriber::registry()
                    .with(env_filter)
                    .with(fmt_layer.pretty())
                    .with(otel_layer)
                    .try_init()
            }
            LogFormat::Auto => unreachable!("LogFormat::resolve never returns Auto"),
        };

        if let Err(err) = install_result {
            // Mirror OtelConfig::install's failure hygiene: release exporter
            // threads and drop the global reference before surfacing the error.
            let _ = provider.shutdown();
            global::set_tracer_provider(
                opentelemetry_sdk::trace::SdkTracerProvider::builder().build(),
            );
            return Err(Box::new(crate::trace::OtelError::Subscriber(
                err.to_string(),
            )));
        }

        global::set_tracer_provider(provider.clone());
        Ok(crate::trace::OtelGuard::from_provider(provider))
    }

    fn env_filter_directive(&self) -> String {
        self.env_filter_directive_with(|key| std::env::var(key).ok())
    }

    fn env_filter_directive_with(&self, env: impl Fn(&str) -> Option<String>) -> String {
        self.env_filter
            .clone()
            .or_else(|| env("RUST_LOG"))
            .unwrap_or_else(|| "info".to_string())
    }
}

/// Structured logger output format.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum LogFormat {
    /// JSON in production-like envs, pretty logs otherwise.
    #[default]
    Auto,
    /// JSON logs suitable for `docker logs app | jq`.
    Json,
    /// Human-readable pretty logs for local development.
    Pretty,
}

impl LogFormat {
    /// Resolve `Auto` using process environment variables.
    pub fn resolve(self) -> Self {
        self.resolve_with(|key| std::env::var(key).ok())
    }

    fn resolve_with(self, env: impl Fn(&str) -> Option<String>) -> Self {
        if self != Self::Auto {
            return self;
        }

        for key in ["ARVIK_ENV", "APP_ENV", "RUST_ENV", "ENV"] {
            if env(key).is_some_and(|value| {
                let value = value.to_ascii_lowercase();
                value == "production" || value == "prod"
            }) {
                return Self::Json;
            }
        }

        Self::Pretty
    }
}

/// Tower layer that emits structured request completion logs.
#[derive(Clone)]
pub struct StructuredLoggingLayer {
    include_headers: bool,
    sensitive_headers: Arc<[HeaderName]>,
    request_id_header: HeaderName,
}

impl StructuredLoggingLayer {
    /// Create a structured request logging layer.
    pub fn new() -> Self {
        Self {
            include_headers: false,
            sensitive_headers: default_sensitive_headers().into(),
            request_id_header: HeaderName::from_static(X_REQUEST_ID),
        }
    }

    /// Include request headers in request spans. Sensitive headers are masked.
    #[must_use]
    pub fn include_headers(mut self, enabled: bool) -> Self {
        self.include_headers = enabled;
        self
    }

    /// Replace the sensitive header list used when header logging is enabled.
    #[must_use]
    pub fn sensitive_headers(mut self, headers: impl IntoIterator<Item = HeaderName>) -> Self {
        self.sensitive_headers = headers.into_iter().collect::<Vec<_>>().into();
        self
    }

    /// Replace the request ID header name. Defaults to `x-request-id`.
    #[must_use]
    pub fn request_id_header(mut self, header: HeaderName) -> Self {
        self.request_id_header = header;
        self
    }
}

impl Default for StructuredLoggingLayer {
    fn default() -> Self {
        Self::new()
    }
}

impl<S> Layer<S> for StructuredLoggingLayer {
    type Service = StructuredLoggingService<S>;

    fn layer(&self, inner: S) -> Self::Service {
        StructuredLoggingService {
            inner,
            include_headers: self.include_headers,
            sensitive_headers: Arc::clone(&self.sensitive_headers),
            request_id_header: self.request_id_header.clone(),
        }
    }
}

/// Service produced by [`StructuredLoggingLayer`].
#[derive(Clone)]
pub struct StructuredLoggingService<S> {
    inner: S,
    include_headers: bool,
    sensitive_headers: Arc<[HeaderName]>,
    request_id_header: HeaderName,
}

impl<S> Service<Request> for StructuredLoggingService<S>
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

    fn call(&mut self, mut req: Request) -> Self::Future {
        // Request-ID plumbing runs unconditionally (it is functional), but the
        // span field strings are only built when INFO spans are actually
        // enabled — otherwise every filtered-out request still paid several
        // heap allocations.
        let span_enabled = tracing::enabled!(tracing::Level::INFO);

        let (method, uri) = if span_enabled {
            (req.method().as_str().to_string(), req.uri().to_string())
        } else {
            (String::new(), String::new())
        };
        let request_id = request_id(&req, &self.request_id_header);

        req.extensions_mut()
            .insert(RequestId::from_string(request_id.clone()));
        if let Ok(value) = HeaderValue::from_str(&request_id) {
            req.headers_mut()
                .insert(self.request_id_header.clone(), value);
        }

        let span = if span_enabled {
            request_span(
                &method,
                &uri,
                &request_id,
                self.include_headers
                    .then(|| masked_headers(req.headers(), &self.sensitive_headers)),
            )
        } else {
            tracing::Span::none()
        };

        let request_id_header = self.request_id_header.clone();
        let request_id_for_response = request_id.clone();
        let cloned = self.inner.clone();
        let mut inner = std::mem::replace(&mut self.inner, cloned);

        Box::pin(async move {
            let started = Instant::now();
            let mut response = inner.call(req).instrument(span.clone()).await?;
            let status = response.status();
            let route = response
                .extensions()
                .get::<MatchedPathExt>()
                .map(|matched| matched.0.to_string())
                .unwrap_or_else(|| "__unknown".to_string());
            let latency_ms = started.elapsed().as_millis();

            if let Ok(value) = HeaderValue::from_str(&request_id_for_response) {
                response.headers_mut().insert(request_id_header, value);
            }

            span.record("http.status_code", status.as_u16());
            span.record("http.route", route.as_str());
            span.record("latency_ms", latency_ms);

            if status.is_server_error() {
                tracing::error!(
                    parent: &span,
                    status = status.as_u16(),
                    route = route.as_str(),
                    latency_ms = latency_ms,
                    "request completed"
                );
            } else if status.is_client_error() {
                tracing::warn!(
                    parent: &span,
                    status = status.as_u16(),
                    route = route.as_str(),
                    latency_ms = latency_ms,
                    "request completed"
                );
            } else {
                tracing::info!(
                    parent: &span,
                    status = status.as_u16(),
                    route = route.as_str(),
                    latency_ms = latency_ms,
                    "request completed"
                );
            }

            Ok(response)
        })
    }
}

fn request_id(req: &Request, header: &HeaderName) -> String {
    req.headers()
        .get(header)
        .and_then(|value| value.to_str().ok())
        .map(str::to_owned)
        .or_else(|| {
            req.extensions()
                .get::<RequestId>()
                .map(|id| id.as_str().to_string())
        })
        .unwrap_or_else(|| Uuid::new_v4().to_string())
}

fn request_span(
    method: &str,
    uri: &str,
    request_id: &str,
    headers: Option<BTreeMap<String, String>>,
) -> Span {
    match headers {
        Some(headers) => tracing::info_span!(
            "request",
            http.method = %method,
            http.uri = %uri,
            http.route = tracing::field::Empty,
            http.status_code = tracing::field::Empty,
            latency_ms = tracing::field::Empty,
            request_id = %request_id,
            request.headers = ?headers,
        ),
        None => tracing::info_span!(
            "request",
            http.method = %method,
            http.uri = %uri,
            http.route = tracing::field::Empty,
            http.status_code = tracing::field::Empty,
            latency_ms = tracing::field::Empty,
            request_id = %request_id,
        ),
    }
}

fn masked_headers(
    headers: &http::HeaderMap,
    sensitive_headers: &[HeaderName],
) -> BTreeMap<String, String> {
    let mut values = BTreeMap::new();
    for (name, value) in headers {
        let value = if sensitive_headers.iter().any(|sensitive| sensitive == name) {
            "[redacted]".to_string()
        } else {
            value
                .to_str()
                .map(str::to_string)
                .unwrap_or_else(|_| "[non-utf8]".to_string())
        };
        values.insert(name.as_str().to_string(), value);
    }
    values
}

fn default_sensitive_headers() -> Vec<HeaderName> {
    vec![
        AUTHORIZATION,
        COOKIE,
        SET_COOKIE,
        HeaderName::from_static("x-api-key"),
        HeaderName::from_static("proxy-authorization"),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;
    use arvik_core::Body;
    use arvik_router::layer::oneshot;
    use arvik_router::{Router, get};
    use http::StatusCode;

    fn request(uri: &str) -> Request {
        Request::new(
            http::Request::builder()
                .uri(uri)
                .body(Body::empty())
                .unwrap(),
        )
    }

    #[test]
    fn auto_format_uses_json_for_production() {
        assert_eq!(
            LogFormat::Auto.resolve_with(|key| (key == "ARVIK_ENV").then(|| "production".into())),
            LogFormat::Json
        );
        assert_eq!(LogFormat::Auto.resolve_with(|_| None), LogFormat::Pretty);
    }

    #[test]
    fn rust_log_env_is_honored() {
        let builder = ArvikLogger::builder();
        assert_eq!(
            builder.env_filter_directive_with(|key| (key == "RUST_LOG").then(|| "debug".into())),
            "debug"
        );

        let builder = ArvikLogger::builder().env_filter("warn");
        assert_eq!(builder.env_filter_directive_with(|_| None), "warn");
    }

    #[tokio::test]
    async fn reuses_incoming_request_id() {
        async fn handler() -> &'static str {
            "ok"
        }

        let app = Router::new()
            .route("/", get(handler))
            .layer(StructuredLoggingLayer::new())
            .into_service();
        let req = Request::new(
            http::Request::builder()
                .uri("/")
                .header(X_REQUEST_ID, "incoming-id")
                .body(Body::empty())
                .unwrap(),
        );
        let response = oneshot(app, req).await;

        assert_eq!(
            response
                .headers()
                .get(X_REQUEST_ID)
                .and_then(|value| value.to_str().ok()),
            Some("incoming-id")
        );
    }

    #[tokio::test]
    async fn generates_missing_request_id_and_preserves_matched_route() {
        async fn handler() -> &'static str {
            "ok"
        }

        let app = Router::new()
            .route("/users/{id}", get(handler))
            .layer(StructuredLoggingLayer::new())
            .into_service();
        let response = oneshot(app, request("/users/42")).await;

        assert_eq!(response.status(), StatusCode::OK);
        assert!(response.headers().get(X_REQUEST_ID).is_some());
        assert_eq!(
            response
                .extensions()
                .get::<MatchedPathExt>()
                .map(|matched| matched.0.as_ref()),
            Some("/users/{id}")
        );
    }

    #[test]
    fn masks_sensitive_headers() {
        let mut headers = http::HeaderMap::new();
        headers.insert(AUTHORIZATION, HeaderValue::from_static("Bearer secret"));
        headers.insert("x-public", HeaderValue::from_static("visible"));

        let masked = masked_headers(&headers, &default_sensitive_headers());

        assert_eq!(masked.get("authorization").unwrap(), "[redacted]");
        assert_eq!(masked.get("x-public").unwrap(), "visible");
    }
}
