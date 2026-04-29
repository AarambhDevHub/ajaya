//! Token-bucket rate limiting middleware.
//!
//! Limits request rates per client IP (or custom key). Returns
//! `429 Too Many Requests` with a `Retry-After` header when the
//! limit is exceeded.
//!
//! # Example
//!
//! ```rust,ignore
//! use ajaya_middleware::rate_limit::RateLimitLayer;
//! use std::time::Duration;
//!
//! // 100 requests per second per IP
//! Router::new()
//!     .route("/api", get(handler))
//!     .layer(RateLimitLayer::new(100, Duration::from_secs(1)));
//! ```

use std::collections::HashMap;
use std::convert::Infallible;
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;
use std::task::{Context, Poll};
use std::time::{Duration, Instant};

use ajaya_core::{Body, Request, Response};
use http::StatusCode;
use parking_lot::Mutex;
use tower_layer::Layer;
use tower_service::Service;

// ── Token bucket ─────────────────────────────────────────────────────────────

struct Bucket {
    tokens: f64,
    capacity: f64,
    refill_rate: f64, // tokens per second
    last_refill: Instant,
}

impl Bucket {
    fn new(capacity: u64) -> Self {
        Self {
            tokens: capacity as f64,
            capacity: capacity as f64,
            refill_rate: capacity as f64,
            last_refill: Instant::now(),
        }
    }

    /// Try to consume one token.
    ///
    /// Returns `None` if the request is allowed, or `Some(retry_after_secs)`
    /// if the bucket is empty.
    fn try_consume(&mut self) -> Option<f64> {
        let now = Instant::now();
        let elapsed = now.duration_since(self.last_refill).as_secs_f64();
        self.tokens = (self.tokens + elapsed * self.refill_rate).min(self.capacity);
        self.last_refill = now;

        if self.tokens >= 1.0 {
            self.tokens -= 1.0;
            None // allowed
        } else {
            // Time until next token is available.
            let retry_after = (1.0 - self.tokens) / self.refill_rate;
            Some(retry_after)
        }
    }
}

// ── Key extraction ────────────────────────────────────────────────────────────

/// Strategy for extracting a rate-limit key from a request.
#[derive(Debug, Clone)]
pub enum KeyExtractor {
    /// Rate limit per client IP address (default).
    /// Reads `X-Forwarded-For` first, falls back to socket addr extension.
    IpAddress,
    /// Custom header value as the key.
    Header(String),
    /// Rate limit globally (all requests share one bucket).
    Global,
}

impl KeyExtractor {
    fn extract_key(&self, req: &Request) -> String {
        match self {
            KeyExtractor::IpAddress => extract_ip(req),
            KeyExtractor::Header(name) => req
                .headers()
                .get(name.as_str())
                .and_then(|v| v.to_str().ok())
                .unwrap_or("unknown")
                .to_string(),
            KeyExtractor::Global => "__global__".to_string(),
        }
    }
}

fn extract_ip(req: &Request) -> String {
    // Try X-Forwarded-For first (behind proxy).
    if let Some(forwarded) = req
        .headers()
        .get("x-forwarded-for")
        .and_then(|v| v.to_str().ok())
    {
        if let Some(ip) = forwarded.split(',').next() {
            return ip.trim().to_string();
        }
    }

    // Try X-Real-IP.
    if let Some(real_ip) = req.headers().get("x-real-ip").and_then(|v| v.to_str().ok()) {
        return real_ip.trim().to_string();
    }

    // Fall back to ConnectInfo extension.
    req.extensions()
        .get::<std::net::SocketAddr>()
        .map(|addr| addr.ip().to_string())
        .unwrap_or_else(|| "unknown".to_string())
}

// ── RateLimitLayer ────────────────────────────────────────────────────────────

/// Tower layer that enforces a token-bucket rate limit.
#[derive(Clone)]
pub struct RateLimitLayer {
    capacity: u64,
    extractor: KeyExtractor,
    /// Shared bucket state — all clones of this layer share the same map.
    state: Arc<Mutex<HashMap<String, Bucket>>>,
}

impl RateLimitLayer {
    /// Create a new rate limiter allowing `capacity` requests per `window`.
    ///
    /// Keys by client IP address by default. The `window` parameter is
    /// kept for API symmetry but the token bucket naturally distributes
    /// `capacity` tokens over a one-second replenishment cycle; adjust
    /// `capacity` to match your `window` (e.g. 10 req / 10s → capacity=1).
    pub fn new(capacity: u64, _window: Duration) -> Self {
        Self {
            capacity,
            extractor: KeyExtractor::IpAddress,
            state: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    /// Rate limit by a custom request header value.
    pub fn by_header(mut self, header_name: impl Into<String>) -> Self {
        self.extractor = KeyExtractor::Header(header_name.into());
        self
    }

    /// Apply a single global rate limit (not per-key).
    pub fn global(mut self) -> Self {
        self.extractor = KeyExtractor::Global;
        self
    }

    /// Use a custom key extractor.
    pub fn with_extractor(mut self, extractor: KeyExtractor) -> Self {
        self.extractor = extractor;
        self
    }
}

impl<S> Layer<S> for RateLimitLayer {
    type Service = RateLimitService<S>;

    fn layer(&self, inner: S) -> Self::Service {
        RateLimitService {
            inner,
            capacity: self.capacity,
            extractor: self.extractor.clone(),
            state: Arc::clone(&self.state),
        }
    }
}

/// Tower service produced by [`RateLimitLayer`].
#[derive(Clone)]
pub struct RateLimitService<S> {
    inner: S,
    capacity: u64,
    extractor: KeyExtractor,
    state: Arc<Mutex<HashMap<String, Bucket>>>,
}

impl<S> Service<Request> for RateLimitService<S>
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
        let key = self.extractor.extract_key(&req);
        let capacity = self.capacity;
        let state = Arc::clone(&self.state);

        let cloned = self.inner.clone();
        let mut inner = std::mem::replace(&mut self.inner, cloned);
        Box::pin(async move {
            // Check the bucket inside a short synchronous critical section.
            let retry_after: Option<f64> = {
                let mut buckets = state.lock();
                let bucket = buckets
                    .entry(key.clone())
                    .or_insert_with(|| Bucket::new(capacity));
                bucket.try_consume()
            };

            if let Some(retry_secs) = retry_after {
                let retry_secs_ceil = retry_secs.ceil() as u64;
                tracing::warn!(
                    key = %key,
                    retry_after = retry_secs_ceil,
                    "Rate limit exceeded"
                );

                // FIX: explicit Ok::<Response, Infallible>(...) so Rust can infer the
                // error type of this early-return future independently of `inner.call`.
                return Ok::<Response, Infallible>(
                    http::Response::builder()
                        .status(StatusCode::TOO_MANY_REQUESTS)
                        .header(http::header::CONTENT_TYPE, "application/json")
                        .header("retry-after", retry_secs_ceil.to_string())
                        .header("x-ratelimit-limit", capacity.to_string())
                        .header("x-ratelimit-remaining", "0")
                        .body(Body::from(format!(
                            r#"{{"error":"Too Many Requests","code":429,"retry_after":{}}}"#,
                            retry_secs_ceil
                        )))
                        .unwrap(),
                );
            }
            inner.call(req).await
        })
    }
}
