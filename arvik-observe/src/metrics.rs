//! Prometheus metrics for Arvik services.

use std::convert::Infallible;
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::task::{Context, Poll};
use std::time::Instant;

use arvik_core::{Body, Request, Response, ResponseBuilder};
use arvik_router::MatchedPathExt;
use bytes::Bytes;
use http_body::{Body as _, Frame};
use prometheus::{
    Encoder, HistogramOpts, HistogramVec, IntCounterVec, IntGaugeVec, Opts, Registry, TextEncoder,
};
use tower_layer::Layer;
use tower_service::Service;

const LABELS: [&str; 6] = [
    "method",
    "route",
    "status",
    "service",
    "version",
    "environment",
];
const PENDING_ROUTE: &str = "__pending";
const UNKNOWN_ROUTE: &str = "__unknown";

/// Prometheus registry and metric families used by Arvik's metrics layer.
#[derive(Clone)]
pub struct MetricsRegistry {
    inner: Arc<MetricsInner>,
}

struct MetricsInner {
    registry: Registry,
    requests_total: IntCounterVec,
    request_duration: HistogramVec,
    requests_in_flight: IntGaugeVec,
    response_body_size: HistogramVec,
    request_body_size: HistogramVec,
}

impl MetricsRegistry {
    /// Create an isolated metrics registry.
    pub fn new() -> Self {
        Self::with_buckets(default_duration_buckets(), default_size_buckets())
    }

    /// Return the process-global metrics registry used by [`metrics_handler`].
    pub fn global() -> Self {
        GLOBAL_REGISTRY.clone()
    }

    /// Encode this registry using Prometheus' text exposition format.
    pub fn encode_text(&self) -> String {
        let encoder = TextEncoder::new();
        let mut buffer = Vec::new();
        encoder
            .encode(&self.inner.registry.gather(), &mut buffer)
            .expect("encoding prometheus metrics cannot fail");
        String::from_utf8(buffer).expect("prometheus text is valid UTF-8")
    }

    /// Return the Prometheus text content type.
    pub fn content_type() -> &'static str {
        "text/plain; version=0.0.4; charset=utf-8"
    }

    /// Gather raw Prometheus metric families.
    pub fn gather(&self) -> Vec<prometheus::proto::MetricFamily> {
        self.inner.registry.gather()
    }

    /// Create a registry with custom duration and size histogram buckets.
    pub fn with_buckets(duration_buckets: Vec<f64>, size_buckets: Vec<f64>) -> Self {
        let registry = Registry::new();

        let requests_total = IntCounterVec::new(
            Opts::new(
                "arvik_requests_total",
                "Total HTTP requests handled by Arvik.",
            ),
            &LABELS,
        )
        .expect("valid prometheus counter");
        let request_duration = HistogramVec::new(
            HistogramOpts::new(
                "arvik_request_duration_seconds",
                "HTTP request duration in seconds.",
            )
            .buckets(duration_buckets),
            &LABELS,
        )
        .expect("valid prometheus histogram");
        let requests_in_flight = IntGaugeVec::new(
            Opts::new(
                "arvik_requests_in_flight",
                "HTTP requests currently being processed by Arvik.",
            ),
            &LABELS,
        )
        .expect("valid prometheus gauge");
        let response_body_size = HistogramVec::new(
            HistogramOpts::new(
                "arvik_response_body_size_bytes",
                "HTTP response body bytes sent by Arvik.",
            )
            .buckets(size_buckets.clone()),
            &LABELS,
        )
        .expect("valid prometheus histogram");
        let request_body_size = HistogramVec::new(
            HistogramOpts::new(
                "arvik_request_body_size_bytes",
                "HTTP request body bytes consumed by Arvik.",
            )
            .buckets(size_buckets),
            &LABELS,
        )
        .expect("valid prometheus histogram");

        registry
            .register(Box::new(requests_total.clone()))
            .expect("register requests_total");
        registry
            .register(Box::new(request_duration.clone()))
            .expect("register request_duration");
        registry
            .register(Box::new(requests_in_flight.clone()))
            .expect("register requests_in_flight");
        registry
            .register(Box::new(response_body_size.clone()))
            .expect("register response_body_size");
        registry
            .register(Box::new(request_body_size.clone()))
            .expect("register request_body_size");

        Self {
            inner: Arc::new(MetricsInner {
                registry,
                requests_total,
                request_duration,
                requests_in_flight,
                response_body_size,
                request_body_size,
            }),
        }
    }
}

impl Default for MetricsRegistry {
    fn default() -> Self {
        Self::new()
    }
}

static GLOBAL_REGISTRY: once_cell::sync::Lazy<MetricsRegistry> =
    once_cell::sync::Lazy::new(MetricsRegistry::new);

/// Tower layer that records Prometheus HTTP metrics.
#[derive(Clone)]
pub struct PrometheusMetricsLayer {
    registry: MetricsRegistry,
    labels: StaticLabels,
    record_body_sizes: bool,
}

#[derive(Clone)]
struct StaticLabels {
    service: Arc<str>,
    version: Arc<str>,
    environment: Arc<str>,
}

impl StaticLabels {
    fn values<'a>(&'a self, method: &'a str, route: &'a str, status: &'a str) -> [&'a str; 6] {
        [
            method,
            route,
            status,
            self.service.as_ref(),
            self.version.as_ref(),
            self.environment.as_ref(),
        ]
    }
}

impl Default for StaticLabels {
    fn default() -> Self {
        Self {
            service: Arc::from("unknown"),
            version: Arc::from("unknown"),
            environment: Arc::from("unknown"),
        }
    }
}

impl PrometheusMetricsLayer {
    /// Create a metrics layer using the process-global registry.
    pub fn new() -> Self {
        Self::with_registry(MetricsRegistry::global())
    }

    /// Create a metrics layer using an explicit registry.
    pub fn with_registry(registry: MetricsRegistry) -> Self {
        Self {
            registry,
            labels: StaticLabels::default(),
            record_body_sizes: true,
        }
    }

    /// Set the `service` label.
    #[must_use]
    pub fn service_name(mut self, value: impl Into<Arc<str>>) -> Self {
        self.labels.service = value.into();
        self
    }

    /// Set the `version` label.
    #[must_use]
    pub fn version(mut self, value: impl Into<Arc<str>>) -> Self {
        self.labels.version = value.into();
        self
    }

    /// Set the `environment` label.
    #[must_use]
    pub fn environment(mut self, value: impl Into<Arc<str>>) -> Self {
        self.labels.environment = value.into();
        self
    }

    /// Enable or disable request/response body size histograms.
    #[must_use]
    pub fn record_body_sizes(mut self, enabled: bool) -> Self {
        self.record_body_sizes = enabled;
        self
    }
}

impl Default for PrometheusMetricsLayer {
    fn default() -> Self {
        Self::new()
    }
}

impl<S> Layer<S> for PrometheusMetricsLayer {
    type Service = PrometheusMetricsService<S>;

    fn layer(&self, inner: S) -> Self::Service {
        PrometheusMetricsService {
            inner,
            registry: self.registry.clone(),
            labels: self.labels.clone(),
            record_body_sizes: self.record_body_sizes,
        }
    }
}

/// Service produced by [`PrometheusMetricsLayer`].
#[derive(Clone)]
pub struct PrometheusMetricsService<S> {
    inner: S,
    registry: MetricsRegistry,
    labels: StaticLabels,
    record_body_sizes: bool,
}

impl<S> Service<Request> for PrometheusMetricsService<S>
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
        let registry = self.registry.clone();
        let labels = self.labels.clone();
        let record_body_sizes = self.record_body_sizes;
        let request_bytes = Arc::new(AtomicU64::new(0));
        let req = if record_body_sizes {
            req.map_body(|body| {
                Body::new(CountingBody::new(body, Arc::clone(&request_bytes), None))
            })
        } else {
            req
        };

        // The pending gauge lookup hashes straight off the request borrow —
        // no owned method copy for it (audit C11).
        let pending_values = labels.values(req.method().as_str(), PENDING_ROUTE, "pending");
        let in_flight_gauge = registry
            .inner
            .requests_in_flight
            .with_label_values(&pending_values);
        in_flight_gauge.inc();
        let in_flight = InFlightGuard::new(in_flight_gauge.clone());

        // Owned: `method` must outlive `req`, which moves into the future.
        let method = req.method().as_str().to_owned();

        let cloned = self.inner.clone();
        let mut inner = std::mem::replace(&mut self.inner, cloned);

        Box::pin(async move {
            let start = Instant::now();
            let mut response = inner.call(req).await?;
            let elapsed = start.elapsed().as_secs_f64();
            // `StatusCode::as_str` already returns the canonical zero-padded
            // form — borrowed from the response instead of a per-request
            // `format!` (audit C11). Only the streaming branch below needs
            // to clone it into its callback.
            let status_code = response.status();
            let status = status_code.as_str();
            // Refcount the interned route instead of re-formatting it (C11).
            let route = matched_route(response.extensions());
            let values = labels.values(&method, &route, status);

            registry
                .inner
                .requests_total
                .with_label_values(&values)
                .inc();
            registry
                .inner
                .request_duration
                .with_label_values(&values)
                .observe(elapsed);

            if record_body_sizes {
                let request_len = request_bytes.load(Ordering::Relaxed) as f64;
                registry
                    .inner
                    .request_body_size
                    .with_label_values(&values)
                    .observe(request_len);

                // A body with an exact size hint (every Content-Length
                // response) records straight from the hint: no counting
                // wrapper, atomic, or callback allocation on the hot path
                // (audit C11). Only genuinely streamed bodies get wrapped.
                match response.body().size_hint().exact() {
                    Some(len) => registry
                        .inner
                        .response_body_size
                        .with_label_values(&values)
                        .observe(len as f64),
                    None => {
                        let response_bytes = Arc::new(AtomicU64::new(0));
                        let observer = BodyObserver::new({
                            let registry = registry.clone();
                            let labels = labels.clone();
                            let method = method.clone();
                            let route = Arc::clone(&route);
                            let status = status.to_owned();
                            move |bytes| {
                                let values = labels.values(&method, &route, &status);
                                registry
                                    .inner
                                    .response_body_size
                                    .with_label_values(&values)
                                    .observe(bytes as f64);
                            }
                        });
                        response = response.map(|body| {
                            Body::new(CountingBody::new(body, response_bytes, Some(observer)))
                        });
                    }
                }
            }

            drop(in_flight);

            Ok(response)
        })
    }
}

/// Process-global Prometheus scrape handler.
pub async fn metrics_handler() -> Response {
    metrics_handler_with_registry(MetricsRegistry::global()).await
}

/// Prometheus scrape handler for a specific registry.
pub async fn metrics_handler_with_registry(registry: MetricsRegistry) -> Response {
    // gather() locks every collector and deep-clones metric families, then
    // the encoder walks them — CPU-bound work that must not run inline on
    // the tokio worker handling the scrape.
    let encoded = tokio::task::spawn_blocking(move || registry.encode_text())
        .await
        .unwrap_or_else(|err| {
            tracing::error!("metrics encode task failed: {}", err);
            String::new()
        });

    ResponseBuilder::new()
        .header(http::header::CONTENT_TYPE, MetricsRegistry::content_type())
        .body(encoded)
}

type BodyCallback = dyn Fn(u64) + Send + Sync + 'static;

#[derive(Clone)]
struct BodyObserver(Arc<BodyCallback>);

impl BodyObserver {
    fn new(callback: impl Fn(u64) + Send + Sync + 'static) -> Self {
        Self(Arc::new(callback))
    }

    fn observe(&self, bytes: u64) {
        (self.0)(bytes);
    }
}

struct CountingBody {
    inner: Body,
    bytes: Arc<AtomicU64>,
    observer: Option<BodyObserver>,
    finished: bool,
}

impl CountingBody {
    fn new(inner: Body, bytes: Arc<AtomicU64>, observer: Option<BodyObserver>) -> Self {
        Self {
            inner,
            bytes,
            observer,
            finished: false,
        }
    }

    fn finish(&mut self) {
        if self.finished {
            return;
        }
        self.finished = true;
        if let Some(observer) = &self.observer {
            observer.observe(self.bytes.load(Ordering::Relaxed));
        }
    }
}

impl http_body::Body for CountingBody {
    type Data = Bytes;
    type Error = arvik_core::body::BoxError;

    fn poll_frame(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<Option<Result<Frame<Self::Data>, Self::Error>>> {
        let this = self.as_mut().get_mut();
        match Pin::new(&mut this.inner).poll_frame(cx) {
            Poll::Ready(Some(Ok(frame))) => {
                if let Some(data) = frame.data_ref() {
                    this.bytes.fetch_add(data.len() as u64, Ordering::Relaxed);
                }
                Poll::Ready(Some(Ok(frame)))
            }
            Poll::Ready(Some(Err(err))) => {
                this.finish();
                Poll::Ready(Some(Err(err)))
            }
            Poll::Ready(None) => {
                this.finish();
                Poll::Ready(None)
            }
            Poll::Pending => Poll::Pending,
        }
    }

    fn is_end_stream(&self) -> bool {
        self.inner.is_end_stream()
    }

    fn size_hint(&self) -> http_body::SizeHint {
        self.inner.size_hint()
    }
}

impl Drop for CountingBody {
    fn drop(&mut self) {
        self.finish();
    }
}

struct InFlightGuard {
    gauge: prometheus::IntGauge,
    active: bool,
}

impl InFlightGuard {
    fn new(gauge: prometheus::IntGauge) -> Self {
        Self {
            gauge,
            active: true,
        }
    }
}

impl Drop for InFlightGuard {
    fn drop(&mut self) {
        if self.active {
            self.gauge.dec();
        }
    }
}

fn matched_route(extensions: &http::Extensions) -> Arc<str> {
    extensions
        .get::<MatchedPathExt>()
        .map(|matched| Arc::clone(&matched.0))
        .unwrap_or_else(|| Arc::from(UNKNOWN_ROUTE))
}

fn default_duration_buckets() -> Vec<f64> {
    vec![
        0.005, 0.01, 0.025, 0.05, 0.1, 0.25, 0.5, 1.0, 2.5, 5.0, 10.0,
    ]
}

fn default_size_buckets() -> Vec<f64> {
    vec![
        0.0,
        64.0,
        256.0,
        1024.0,
        4096.0,
        16_384.0,
        65_536.0,
        262_144.0,
        1_048_576.0,
        4_194_304.0,
    ]
}

#[cfg(test)]
mod tests {
    use super::*;
    use arvik_core::ResponseBuilder;
    use arvik_router::layer::oneshot;
    use arvik_router::{Router, get, post};
    use http::{Method, StatusCode};

    fn request(method: Method, uri: &str, body: impl Into<Body>) -> Request {
        Request::new(
            http::Request::builder()
                .method(method)
                .uri(uri)
                .body(body.into())
                .unwrap(),
        )
    }

    #[tokio::test]
    async fn records_request_status_and_matched_route() {
        async fn user() -> &'static str {
            "ok"
        }

        let registry = MetricsRegistry::new();
        let app = Router::new()
            .route("/users/{id}", get(user))
            .layer(PrometheusMetricsLayer::with_registry(registry.clone()))
            .into_service();

        let response = oneshot(app.clone(), request(Method::GET, "/users/42", ())).await;
        assert_eq!(response.status(), StatusCode::OK);

        let text = registry.encode_text();
        assert!(text.contains("arvik_requests_total"));
        assert!(text.contains(r#"route="/users/{id}""#));
        assert!(!text.contains("/users/42"));

        let _ = response.into_body().to_bytes().await.unwrap();
    }

    #[tokio::test]
    async fn records_body_sizes() {
        async fn echo(req: Request) -> Response {
            let body = req.into_body().to_bytes().await.unwrap();
            ResponseBuilder::new().body(body)
        }

        let registry = MetricsRegistry::new();
        let app = Router::new()
            .route("/echo", post(echo))
            .layer(PrometheusMetricsLayer::with_registry(registry.clone()))
            .into_service();

        let response = oneshot(app, request(Method::POST, "/echo", "hello")).await;
        assert_eq!(response.into_body().to_bytes().await.unwrap(), "hello");

        let text = registry.encode_text();
        assert!(text.contains("arvik_request_body_size_bytes_bucket"));
        assert!(text.contains("arvik_response_body_size_bytes_bucket"));
    }

    #[tokio::test]
    async fn metrics_handler_returns_text() {
        let registry = MetricsRegistry::new();
        let response = metrics_handler_with_registry(registry).await;

        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(
            response
                .headers()
                .get(http::header::CONTENT_TYPE)
                .unwrap()
                .to_str()
                .unwrap(),
            MetricsRegistry::content_type()
        );
    }
}
