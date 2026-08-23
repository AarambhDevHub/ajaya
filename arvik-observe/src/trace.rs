//! OpenTelemetry tracing integration for Arvik.

use std::convert::Infallible;
use std::future::Future;
use std::pin::Pin;
use std::str::FromStr;
use std::task::{Context, Poll};

use arvik_core::{Request, Response};
use arvik_router::MatchedPathExt;
use opentelemetry::trace::{
    SpanContext, SpanId, TraceContextExt, TraceFlags, TraceId, TraceState, TracerProvider,
};
use opentelemetry::{Context as OtelContext, KeyValue, global};
use opentelemetry_otlp::{Protocol, WithExportConfig};
use opentelemetry_sdk::Resource;
use opentelemetry_sdk::trace::SdkTracerProvider;
use tower_layer::Layer;
use tower_service::Service;
use tracing::{Instrument, Span};
use tracing_opentelemetry::OpenTelemetrySpanExt;
use tracing_subscriber::prelude::*;
use tracing_subscriber::util::SubscriberInitExt;

/// Supported incoming trace propagation formats.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Propagation {
    /// W3C `traceparent` and `tracestate`.
    TraceContext,
    /// Zipkin B3 single and multi-header propagation.
    B3,
    /// Jaeger `uber-trace-id` propagation.
    Jaeger,
}

impl Propagation {
    fn all() -> Vec<Self> {
        vec![Self::TraceContext, Self::B3, Self::Jaeger]
    }
}

/// Tower layer that creates an OpenTelemetry-compatible span per HTTP request.
#[derive(Debug, Clone)]
pub struct OtelLayer {
    service_name: String,
    propagators: Vec<Propagation>,
}

impl OtelLayer {
    /// Create an OpenTelemetry HTTP layer.
    pub fn new(service_name: impl Into<String>) -> Self {
        Self {
            service_name: service_name.into(),
            propagators: Propagation::all(),
        }
    }

    /// Replace the incoming propagation formats used by this layer.
    #[must_use]
    pub fn propagators(mut self, propagators: impl IntoIterator<Item = Propagation>) -> Self {
        self.propagators = propagators.into_iter().collect();
        self
    }
}

impl<S> Layer<S> for OtelLayer {
    type Service = OtelService<S>;

    fn layer(&self, inner: S) -> Self::Service {
        OtelService {
            inner,
            service_name: self.service_name.clone(),
            propagators: self.propagators.clone(),
        }
    }
}

/// Service produced by [`OtelLayer`].
#[derive(Clone)]
pub struct OtelService<S> {
    inner: S,
    service_name: String,
    propagators: Vec<Propagation>,
}

impl<S> Service<Request> for OtelService<S>
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
        // Skip all span construction (including the field strings and the
        // propagator extraction) when INFO spans are filtered out anyway.
        let span = if !tracing::enabled!(tracing::Level::INFO) {
            tracing::Span::none()
        } else {
            let method = req.method().as_str().to_owned();
            // Path only — query strings routinely carry credentials and must
            // not land in traces by default.
            let url = req.uri().path().to_string();
            let user_agent = req
                .headers()
                .get(http::header::USER_AGENT)
                .and_then(|value| value.to_str().ok())
                .unwrap_or("")
                .to_owned();
            let span = make_http_span(&self.service_name, &method, &url, &user_agent);

            if let Some(parent) = extract_parent(req.headers(), &self.propagators) {
                let _ = span.set_parent(parent);
            }
            span
        };

        let cloned = self.inner.clone();
        let mut inner = std::mem::replace(&mut self.inner, cloned);

        Box::pin(async move {
            let response = inner.call(req).instrument(span.clone()).await?;
            let status = response.status();
            let route = response
                .extensions()
                .get::<MatchedPathExt>()
                .map(|matched| matched.0.as_ref())
                .unwrap_or("__unknown");

            span.record("http.status_code", status.as_u16());
            span.record("http.route", route);
            span.set_attribute("http.status_code", status.as_u16() as i64);
            span.set_attribute("http.route", route.to_owned());
            if status.is_server_error() {
                span.set_status(opentelemetry::trace::Status::error(format!(
                    "HTTP {}",
                    status.as_u16()
                )));
            } else {
                span.set_status(opentelemetry::trace::Status::Ok);
            }

            Ok(response)
        })
    }
}

fn make_http_span(service_name: &str, method: &str, url: &str, user_agent: &str) -> Span {
    let span = tracing::info_span!(
        "http.request",
        otel.kind = "server",
        service.name = %service_name,
        http.method = %method,
        http.url = %url,
        http.user_agent = %user_agent,
        http.status_code = tracing::field::Empty,
        http.route = tracing::field::Empty,
    );
    span.set_attribute("service.name", service_name.to_owned());
    span.set_attribute("http.method", method.to_owned());
    span.set_attribute("http.url", url.to_owned());
    span.set_attribute("http.user_agent", user_agent.to_owned());
    span
}

/// OpenTelemetry exporter and subscriber configuration.
#[derive(Debug, Clone)]
pub struct OtelConfig {
    service_name: String,
    exporter: Exporter,
    resource_attrs: Vec<(String, String)>,
}

#[derive(Debug, Clone)]
enum Exporter {
    Stdout,
    OtlpGrpc(Option<String>),
    OtlpHttp(Option<String>),
}

impl OtelConfig {
    /// Create an OpenTelemetry configuration for a service.
    pub fn new(service_name: impl Into<String>) -> Self {
        Self {
            service_name: service_name.into(),
            exporter: Exporter::Stdout,
            resource_attrs: Vec::new(),
        }
    }

    /// Export spans to stdout. This is the default and is useful for local development.
    #[must_use]
    pub fn stdout(mut self) -> Self {
        self.exporter = Exporter::Stdout;
        self
    }

    /// Export spans to stdout using the stdout OpenTelemetry exporter.
    #[must_use]
    pub fn stdout_json(self) -> Self {
        self.stdout()
    }

    /// Export spans to an OTLP gRPC endpoint. Pass an empty string to use SDK defaults.
    #[must_use]
    pub fn otlp_grpc(mut self, endpoint: impl Into<String>) -> Self {
        let endpoint = endpoint.into();
        self.exporter = Exporter::OtlpGrpc((!endpoint.is_empty()).then_some(endpoint));
        self
    }

    /// Export spans to an OTLP HTTP/protobuf endpoint. Pass an empty string to use SDK defaults.
    #[must_use]
    pub fn otlp_http(mut self, endpoint: impl Into<String>) -> Self {
        let endpoint = endpoint.into();
        self.exporter = Exporter::OtlpHttp((!endpoint.is_empty()).then_some(endpoint));
        self
    }

    /// Add a resource attribute to all emitted spans.
    #[must_use]
    pub fn resource_attr(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.resource_attrs.push((key.into(), value.into()));
        self
    }

    /// Build a tracer provider without installing a global tracing subscriber.
    pub fn build_provider(&self) -> Result<SdkTracerProvider, OtelError> {
        let resource = Resource::builder()
            .with_service_name(self.service_name.clone())
            .with_attributes(
                self.resource_attrs
                    .iter()
                    .map(|(key, value)| KeyValue::new(key.clone(), value.clone())),
            )
            .build();
        let builder = SdkTracerProvider::builder().with_resource(resource);

        match &self.exporter {
            Exporter::Stdout => {
                // Batch, not simple: the simple processor serializes and
                // writes each span inline on whichever task ends it — a
                // synchronous stdout write on the request hot path.
                Ok(builder
                    .with_batch_exporter(opentelemetry_stdout::SpanExporter::default())
                    .build())
            }
            Exporter::OtlpGrpc(endpoint) => {
                let mut exporter = opentelemetry_otlp::SpanExporter::builder().with_tonic();
                if let Some(endpoint) = endpoint {
                    exporter = exporter.with_endpoint(endpoint);
                }
                let exporter = exporter
                    .build()
                    .map_err(|err| OtelError::Exporter(err.to_string()))?;
                Ok(builder.with_batch_exporter(exporter).build())
            }
            Exporter::OtlpHttp(endpoint) => {
                let mut exporter = opentelemetry_otlp::SpanExporter::builder()
                    .with_http()
                    .with_protocol(Protocol::HttpBinary);
                if let Some(endpoint) = endpoint {
                    exporter = exporter.with_endpoint(endpoint);
                }
                let exporter = exporter
                    .build()
                    .map_err(|err| OtelError::Exporter(err.to_string()))?;
                Ok(builder.with_batch_exporter(exporter).build())
            }
        }
    }

    /// Return the configured service name.
    pub fn service_name(&self) -> &str {
        &self.service_name
    }

    /// Install the tracer provider and a `tracing-opentelemetry` subscriber layer.
    ///
    /// The global subscriber is installed first; if that fails (typically
    /// because another subscriber — e.g. [`crate::logging::ArvikLogger`] — is
    /// already installed), the freshly built provider is shut down and the
    /// global reference released instead of leaking its exporter task.
    ///
    /// To combine structured logging with OpenTelemetry in one subscriber,
    /// use [`crate::logging::ArvikLogger::init_with_otel`].
    pub fn install(self) -> Result<OtelGuard, OtelError> {
        let provider = self.build_provider()?;
        let tracer = provider.tracer(self.service_name.clone());

        let install_result = tracing_subscriber::registry()
            .with(tracing_opentelemetry::layer().with_tracer(tracer))
            .try_init();

        if let Err(err) = install_result {
            // Stop exporter threads, then overwrite the global reference so
            // nothing keeps the provider alive for the rest of the process.
            let _ = provider.shutdown();
            global::set_tracer_provider(SdkTracerProvider::builder().build());
            return Err(OtelError::Subscriber(err.to_string()));
        }

        // Only publish globally once installation actually succeeded.
        global::set_tracer_provider(provider.clone());

        Ok(OtelGuard {
            provider: Some(provider),
        })
    }
}

/// Guard that shuts down the installed OpenTelemetry tracer provider.
pub struct OtelGuard {
    provider: Option<SdkTracerProvider>,
}

impl OtelGuard {
    /// Wrap an externally built provider so its lifetime is managed here.
    pub(crate) fn from_provider(provider: SdkTracerProvider) -> Self {
        Self {
            provider: Some(provider),
        }
    }
}

impl OtelGuard {
    /// Flush and shut down the tracer provider now.
    pub fn shutdown(mut self) -> Result<(), OtelError> {
        if let Some(provider) = self.provider.take() {
            provider
                .shutdown()
                .map_err(|err| OtelError::Shutdown(err.to_string()))?;
        }
        Ok(())
    }
}

impl Drop for OtelGuard {
    fn drop(&mut self) {
        if let Some(provider) = self.provider.take() {
            let _ = provider.shutdown();
        }
    }
}

/// Error returned by OpenTelemetry setup and shutdown.
#[derive(Debug, thiserror::Error)]
pub enum OtelError {
    /// Failed to build an exporter.
    #[error("failed to build OpenTelemetry exporter: {0}")]
    Exporter(String),
    /// Failed to install the tracing subscriber.
    #[error("failed to install OpenTelemetry tracing subscriber: {0}")]
    Subscriber(String),
    /// Failed to shut down the tracer provider.
    #[error("failed to shut down OpenTelemetry tracer provider: {0}")]
    Shutdown(String),
}

fn extract_parent(headers: &http::HeaderMap, propagators: &[Propagation]) -> Option<OtelContext> {
    for propagator in propagators {
        let context = match propagator {
            Propagation::TraceContext => extract_trace_context(headers),
            Propagation::B3 => extract_b3(headers),
            Propagation::Jaeger => extract_jaeger(headers),
        };
        if context.is_some() {
            return context;
        }
    }
    None
}

fn extract_trace_context(headers: &http::HeaderMap) -> Option<OtelContext> {
    let traceparent = header(headers, "traceparent")?;
    let mut parts = traceparent.split('-');
    let version = parts.next()?;
    let trace_id = parts.next()?;
    let span_id = parts.next()?;
    let flags = parts.next()?;

    if version.len() != 2 || trace_id.len() != 32 || span_id.len() != 16 || flags.len() != 2 {
        return None;
    }

    let trace_state = header(headers, "tracestate")
        .and_then(|value| TraceState::from_str(value).ok())
        .unwrap_or_default();
    let sampled = u8::from_str_radix(flags, 16)
        .ok()
        .is_some_and(|flags| flags & 0x01 == 0x01);

    make_remote_context(trace_id, span_id, sampled, trace_state)
}

fn extract_b3(headers: &http::HeaderMap) -> Option<OtelContext> {
    if let Some(single) = header(headers, "b3").filter(|value| value.contains('-')) {
        let mut parts = single.split('-');
        let trace_id = parts.next()?;
        let span_id = parts.next()?;
        let sampled = parts
            .next()
            .map(|flag| flag == "1" || flag.eq_ignore_ascii_case("d"))
            .unwrap_or(false);
        return make_remote_context(trace_id, span_id, sampled, TraceState::default());
    }

    let trace_id = header(headers, "x-b3-traceid")?;
    let span_id = header(headers, "x-b3-spanid")?;
    let sampled = header(headers, "x-b3-flags").is_some_and(|value| value == "1")
        || header(headers, "x-b3-sampled")
            .is_some_and(|value| value == "1" || value.eq_ignore_ascii_case("true"));

    make_remote_context(trace_id, span_id, sampled, TraceState::default())
}

fn extract_jaeger(headers: &http::HeaderMap) -> Option<OtelContext> {
    let value = header(headers, "uber-trace-id")?;
    let mut parts = value.split(':');
    let trace_id = parts.next()?;
    let span_id = parts.next()?;
    let _parent_span_id = parts.next()?;
    let flags = parts.next().unwrap_or("0");
    let sampled = u8::from_str_radix(flags, 16)
        .ok()
        .is_some_and(|flags| flags & 0x01 == 0x01);

    make_remote_context(trace_id, span_id, sampled, TraceState::default())
}

fn make_remote_context(
    trace_id: &str,
    span_id: &str,
    sampled: bool,
    trace_state: TraceState,
) -> Option<OtelContext> {
    let trace_id = TraceId::from_hex(trace_id).ok()?;
    let span_id = SpanId::from_hex(span_id).ok()?;
    let flags = if sampled {
        TraceFlags::SAMPLED
    } else {
        TraceFlags::NOT_SAMPLED
    };
    let context = SpanContext::new(trace_id, span_id, flags, true, trace_state);
    context
        .is_valid()
        .then(|| OtelContext::current().with_remote_span_context(context))
}

fn header<'a>(headers: &'a http::HeaderMap, name: &str) -> Option<&'a str> {
    headers.get(name)?.to_str().ok()
}

#[cfg(test)]
mod tests {
    use super::*;
    use arvik_router::layer::oneshot;
    use arvik_router::{Router, get};
    use http::{HeaderValue, Method, StatusCode};

    fn request(uri: &str) -> Request {
        Request::new(
            http::Request::builder()
                .method(Method::GET)
                .uri(uri)
                .body(arvik_core::Body::empty())
                .unwrap(),
        )
    }

    #[test]
    fn extracts_trace_context_parent() {
        let mut headers = http::HeaderMap::new();
        headers.insert(
            "traceparent",
            HeaderValue::from_static("00-4bf92f3577b34da6a3ce929d0e0e4736-00f067aa0ba902b7-01"),
        );
        headers.insert("tracestate", HeaderValue::from_static("vendor=value"));

        assert!(extract_trace_context(&headers).is_some());
    }

    #[test]
    fn extracts_b3_parent() {
        let mut headers = http::HeaderMap::new();
        headers.insert(
            "b3",
            HeaderValue::from_static("4bf92f3577b34da6a3ce929d0e0e4736-00f067aa0ba902b7-1"),
        );

        assert!(extract_b3(&headers).is_some());
    }

    #[test]
    fn extracts_jaeger_parent() {
        let mut headers = http::HeaderMap::new();
        headers.insert(
            "uber-trace-id",
            HeaderValue::from_static("4bf92f3577b34da6a3ce929d0e0e4736:00f067aa0ba902b7:0:1"),
        );

        assert!(extract_jaeger(&headers).is_some());
    }

    #[tokio::test]
    async fn layer_dispatches_and_preserves_response() {
        async fn handler() -> &'static str {
            "ok"
        }

        let app = Router::new()
            .route("/users/{id}", get(handler))
            .layer(OtelLayer::new("test"))
            .into_service();
        let response = oneshot(app, request("/users/42")).await;

        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(
            response
                .extensions()
                .get::<MatchedPathExt>()
                .map(|matched| matched.0.as_ref()),
            Some("/users/{id}")
        );
    }

    #[test]
    fn exporter_builders_construct() {
        let stdout = OtelConfig::new("test").stdout().build_provider();
        assert!(stdout.is_ok());

        let http = OtelConfig::new("test")
            .otlp_http("http://localhost:4318")
            .build_provider();
        assert!(http.is_ok());
    }
}
