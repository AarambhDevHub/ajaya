//! Token-bucket rate limiting middleware.
//!
//! Limits request rates per client IP (or custom key). Returns
//! `429 Too Many Requests` with a `Retry-After` header when the
//! limit is exceeded.
//!
//! # Client identification
//!
//! By default clients are keyed by **socket address** — client-supplied
//! `X-Forwarded-For` / `X-Real-IP` headers are ignored, so a client cannot
//! mint fresh buckets by rotating header values. When deployed behind a
//! trusted reverse proxy, declare it with [`RateLimitLayer::trust_proxies`];
//! the limiter then applies the standard *rightmost-untrusted-hop* rule to
//! `X-Forwarded-For`.
//!
//! # Example
//!
//! ```rust,ignore
//! use arvik_middleware::rate_limit::RateLimitLayer;
//! use std::time::Duration;
//!
//! // 100 requests per second per IP
//! Router::new()
//!     .route("/api", get(handler))
//!     .layer(RateLimitLayer::new(100, Duration::from_secs(1)));
//!
//! // 600 requests per minute per IP, behind one known proxy
//! Router::new()
//!     .route("/api", get(handler))
//!     .layer(RateLimitLayer::new(600, Duration::from_secs(60))
//!         .trust_proxies(["10.0.0.9".parse::<std::net::IpAddr>().unwrap()]));
//! ```

use std::collections::HashMap;
use std::collections::hash_map::RandomState;
use std::convert::Infallible;
use std::future::Future;
use std::hash::BuildHasher;
use std::pin::Pin;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::task::{Context, Poll};
use std::time::{Duration, Instant};

use arvik_core::{Body, Request, Response};
use http::StatusCode;
use parking_lot::Mutex;
use tower_layer::Layer;
use tower_service::Service;

/// Number of independently locked bucket shards.
const SHARDS: usize = 32;

/// Run an idle-bucket sweep roughly once per this many requests.
const SWEEP_EVERY: u64 = 4096;

/// Buckets idle longer than this are evicted from the shared state.
const IDLE_TTL: Duration = Duration::from_secs(300);

/// Upper bound on custom-header key length (bytes).
const MAX_HEADER_KEY_LEN: usize = 128;

// ── Token bucket ─────────────────────────────────────────────────────────────

struct Bucket {
    tokens: f64,
    #[allow(dead_code)]
    capacity: f64,
    refill_rate: f64, // tokens per second = capacity / window
    last_refill: Instant,
    last_seen: Instant,
}

impl Bucket {
    fn new(capacity: u64, window: Duration) -> Self {
        let secs = window.as_secs_f64();
        debug_assert!(secs > 0.0, "rate limit window must be greater than zero");
        Self {
            tokens: capacity as f64,
            capacity: capacity as f64,
            refill_rate: capacity as f64 / secs.max(f64::MIN_POSITIVE),
            last_refill: Instant::now(),
            last_seen: Instant::now(),
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
        self.last_seen = now;

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

// ── Shared, sharded bucket state ─────────────────────────────────────────────

/// One independently locked shard plus its own sweep counter — no cross-core
/// atomic traffic, and sweeps spread across whichever threads touch the shard.
struct Shard {
    buckets: Mutex<HashMap<String, Bucket>>,
    calls: AtomicU64,
}

/// Sharded bucket store: concurrent requests with different keys never
/// serialize behind one mutex.
struct BucketStore {
    shards: Vec<Shard>,
    hasher: RandomState,
}

impl BucketStore {
    fn new(_window: Duration) -> Self {
        Self {
            shards: (0..SHARDS)
                .map(|_| Shard {
                    buckets: Mutex::new(HashMap::new()),
                    calls: AtomicU64::new(0),
                })
                .collect(),
            hasher: RandomState::new(),
        }
    }

    fn shard_for(&self, key: &str) -> &Shard {
        let idx = (self.hasher.hash_one(key) % self.shards.len() as u64) as usize;
        &self.shards[idx]
    }

    /// Consume one token for `key`, creating the bucket on first sight.
    ///
    /// Returns `None` when allowed, or `Some(retry_after_secs)` when limited.
    fn try_consume(&self, key: &str, capacity: u64, window: Duration) -> Option<f64> {
        let shard = self.shard_for(key);

        // Amortized idle-bucket sweep scoped to this shard — bounded memory
        // under key churn without a global contended counter.
        if shard.calls.fetch_add(1, Ordering::Relaxed) % SWEEP_EVERY == 0 {
            shard
                .buckets
                .lock()
                .retain(|_, bucket| bucket.last_seen.elapsed() < IDLE_TTL);
        }

        let mut map = shard.buckets.lock();
        // Hot path: existing bucket, no allocation.
        if let Some(bucket) = map.get_mut(key) {
            return bucket.try_consume();
        }
        let mut bucket = Bucket::new(capacity, window);
        let outcome = bucket.try_consume();
        map.insert(key.to_string(), bucket);
        outcome
    }
}

// ── Key extraction ────────────────────────────────────────────────────────────

/// How much the limiter trusts client-supplied IP headers.
///
/// Defaults to [`ProxyTrust::Never`] — clients cannot mint new rate-limit
/// buckets by sending different `X-Forwarded-For` values.
#[derive(Debug, Clone, Default)]
pub enum ProxyTrust {
    /// Ignore `X-Forwarded-For` / `X-Real-IP`; key by socket address.
    #[default]
    Never,
    /// Honor proxy headers **only** when the immediate peer is one of these
    /// addresses. The client is the rightmost `X-Forwarded-For` entry that is
    /// not itself a trusted proxy (the standard rightmost-untrusted-hop rule).
    Trusted(Vec<std::net::IpAddr>),
}

/// Strategy for extracting a rate-limit key from a request.
#[derive(Debug, Clone)]
pub enum KeyExtractor {
    /// Rate limit per client IP address (default). Identification honors the
    /// layer's [`ProxyTrust`] setting.
    IpAddress,
    /// Custom header value as the key. Values longer than 128 bytes are
    /// truncated (long unique values must not grow the bucket map).
    Header(String),
    /// Rate limit globally (all requests share one bucket).
    Global,
}

impl KeyExtractor {
    fn extract_key(&self, req: &Request, trust: &ProxyTrust) -> String {
        match self {
            KeyExtractor::IpAddress => extract_ip(req, trust),
            KeyExtractor::Header(name) => {
                let raw = req
                    .headers()
                    .get(name.as_str())
                    .and_then(|v| v.to_str().ok())
                    .unwrap_or("unknown");
                let mut key = raw.trim().to_string();
                key.truncate(MAX_HEADER_KEY_LEN);
                key
            }
            KeyExtractor::Global => "__global__".to_string(),
        }
    }
}

/// Resolve the client IP for rate limiting.
///
/// With [`ProxyTrust::Never`] this is always the socket address. With
/// [`ProxyTrust::Trusted`], header values are considered only when the
/// immediate peer is a declared proxy, and only well-formed IPs are accepted —
/// garbage entries fall back to the peer address rather than becoming keys.
fn extract_ip(req: &Request, trust: &ProxyTrust) -> String {
    let peer = req
        .extensions()
        .get::<std::net::SocketAddr>()
        .map(|addr| addr.ip());

    let Some(peer_ip) = peer else {
        // No connection info available (e.g. direct-call test paths).
        return "unknown".to_string();
    };

    let ProxyTrust::Trusted(proxies) = trust else {
        return peer_ip.to_string();
    };
    if !proxies.contains(&peer_ip) {
        return peer_ip.to_string();
    }

    // Peer is a trusted proxy: walk XFF right-to-left for the first entry
    // that is not itself a trusted proxy address.
    if let Some(forwarded) = req
        .headers()
        .get("x-forwarded-for")
        .and_then(|v| v.to_str().ok())
    {
        for candidate in forwarded.split(',').rev() {
            let candidate = candidate.trim();
            if candidate.is_empty() {
                continue;
            }
            match candidate.parse::<std::net::IpAddr>() {
                Ok(ip) if !proxies.contains(&ip) => return ip.to_string(),
                Ok(_) => continue, // another trusted hop; keep walking left
                Err(_) => break,   // malformed entry — do not trust the rest
            }
        }
    }

    // Every reported hop was trusted (or the header was absent/malformed):
    // attribute to the proxy itself rather than to attacker-controlled text.
    peer_ip.to_string()
}

// ── RateLimitLayer ────────────────────────────────────────────────────────────

/// Tower layer that enforces a token-bucket rate limit.
#[derive(Clone)]
pub struct RateLimitLayer {
    capacity: u64,
    window: Duration,
    extractor: KeyExtractor,
    proxy_trust: ProxyTrust,
    store: Arc<BucketStore>,
}

impl RateLimitLayer {
    /// Create a rate limiter allowing `capacity` requests per `window`,
    /// refilling continuously (e.g. `new(600, 60s)` admits bursts up to 600
    /// and sustains 10 requests/second).
    ///
    /// # Panics
    ///
    /// Panics if `window` is zero.
    pub fn new(capacity: u64, window: Duration) -> Self {
        assert!(
            !window.is_zero(),
            "RateLimitLayer window must be greater than zero"
        );
        Self {
            capacity,
            window,
            extractor: KeyExtractor::IpAddress,
            proxy_trust: ProxyTrust::default(),
            store: Arc::new(BucketStore::new(window)),
        }
    }

    /// Declare trusted reverse proxies for client-IP resolution.
    ///
    /// Only these peers may set `X-Forwarded-For`; the client becomes the
    /// rightmost non-trusted entry. Without this, IP keys come from the
    /// socket address and proxy headers are ignored.
    pub fn trust_proxies(mut self, proxies: impl IntoIterator<Item = std::net::IpAddr>) -> Self {
        self.proxy_trust = ProxyTrust::Trusted(proxies.into_iter().collect());
        self
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
            window: self.window,
            extractor: self.extractor.clone(),
            proxy_trust: self.proxy_trust.clone(),
            store: Arc::clone(&self.store),
        }
    }
}

/// Tower service produced by [`RateLimitLayer`].
#[derive(Clone)]
pub struct RateLimitService<S> {
    inner: S,
    capacity: u64,
    window: Duration,
    extractor: KeyExtractor,
    proxy_trust: ProxyTrust,
    store: Arc<BucketStore>,
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
        let key = self.extractor.extract_key(&req, &self.proxy_trust);
        let capacity = self.capacity;
        let window = self.window;
        let store = Arc::clone(&self.store);

        let cloned = self.inner.clone();
        let mut inner = std::mem::replace(&mut self.inner, cloned);
        Box::pin(async move {
            // Short synchronous critical section on one shard only.
            let retry_after: Option<f64> = store.try_consume(&key, capacity, window);

            if let Some(retry_secs) = retry_after {
                let retry_secs_ceil = retry_secs.ceil() as u64;
                tracing::warn!(
                    key = %key,
                    retry_after = retry_secs_ceil,
                    "Rate limit exceeded"
                );

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

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::SocketAddr;

    fn request_with_peer_and_xff(peer: SocketAddr, xff: Option<&str>) -> Request {
        let mut builder = http::Request::builder().method(http::Method::GET).uri("/");
        if let Some(xff) = xff {
            builder = builder.header("x-forwarded-for", xff);
        }
        let mut req = Request::new(builder.body(Body::empty()).unwrap());
        req.extensions_mut().insert(peer);
        req
    }

    fn peer_addr(ip: [u8; 4], port: u16) -> SocketAddr {
        SocketAddr::from((ip, port))
    }

    // ── H6: client identification ────────────────────────────────────────────

    #[test]
    fn spoofed_forwarded_headers_are_ignored_by_default() {
        let trust = ProxyTrust::default();
        let req = request_with_peer_and_xff(
            peer_addr([203, 0, 113, 7], 50_000),
            Some("1.2.3.4, 5.6.7.8"),
        );
        assert_eq!(extract_ip(&req, &trust), "203.0.113.7");
    }

    #[test]
    fn missing_connection_info_falls_back_to_unknown() {
        let req = Request::new(
            http::Request::builder()
                .method(http::Method::GET)
                .uri("/")
                .body(Body::empty())
                .unwrap(),
        );
        assert_eq!(extract_ip(&req, &ProxyTrust::default()), "unknown");
    }

    #[test]
    fn untrusted_proxy_cannot_set_client_identity() {
        let trust = ProxyTrust::Trusted(vec!["10.0.0.9".parse().unwrap()]);
        let req = request_with_peer_and_xff(
            peer_addr([198, 51, 100, 4], 40_000), // NOT a trusted proxy
            Some("1.2.3.4"),
        );
        assert_eq!(extract_ip(&req, &trust), "198.51.100.4");
    }

    #[test]
    fn trusted_proxy_yields_rightmost_untrusted_hop() {
        let trust = ProxyTrust::Trusted(vec!["10.0.0.9".parse().unwrap()]);
        let req = request_with_peer_and_xff(
            peer_addr([10, 0, 0, 9], 40_000), // trusted proxy
            Some("203.0.113.5, 10.0.0.9"),    // client, then the proxy chain
        );
        assert_eq!(extract_ip(&req, &trust), "203.0.113.5");
    }

    #[test]
    fn fully_trusted_chain_attributes_to_the_proxy() {
        let trust = ProxyTrust::Trusted(vec!["10.0.0.9".parse().unwrap()]);
        let req =
            request_with_peer_and_xff(peer_addr([10, 0, 0, 9], 40_000), Some("10.0.0.9, 10.0.0.9"));
        assert_eq!(extract_ip(&req, &trust), "10.0.0.9");
    }

    #[test]
    fn malformed_forwarded_entries_are_not_keys() {
        let trust = ProxyTrust::Trusted(vec!["10.0.0.9".parse().unwrap()]);
        // Attacker-controlled garbage must not become a bucket key.
        let req = request_with_peer_and_xff(
            peer_addr([10, 0, 0, 9], 40_000),
            Some("<script>alert(1)</script>, 10.0.0.9"),
        );
        assert_eq!(extract_ip(&req, &trust), "10.0.0.9");
    }

    #[test]
    fn header_keys_are_trimmed_and_bounded() {
        let extractor = KeyExtractor::Header("x-api-key".to_string());
        let long_value = "k".repeat(500);
        let mut req = Request::new(
            http::Request::builder()
                .method(http::Method::GET)
                .uri("/")
                .header("x-api-key", long_value.as_str())
                .body(Body::empty())
                .unwrap(),
        );
        req.extensions_mut().insert(peer_addr([1, 2, 3, 4], 80));
        assert_eq!(
            extractor.extract_key(&req, &ProxyTrust::default()).len(),
            128
        );
    }

    // ── M8: window drives refill rate ────────────────────────────────────────

    #[test]
    fn refill_rate_matches_capacity_per_window() {
        let minute = Bucket::new(600, Duration::from_secs(60));
        assert!((minute.refill_rate - 10.0).abs() < 1e-9); // 10/s sustained

        let second = Bucket::new(100, Duration::from_secs(1));
        assert!((second.refill_rate - 100.0).abs() < 1e-9);
    }

    #[tokio::test]
    async fn burst_cap_and_retry_after_reflect_the_window() {
        let mut svc = RateLimitLayer::new(5, Duration::from_secs(50))
            .global()
            .layer(PassthroughService);

        // Burn the burst allowance.
        for _ in 0..5 {
            let res = svc
                .call(Request::new(
                    http::Request::builder()
                        .uri("/")
                        .body(Body::empty())
                        .unwrap(),
                ))
                .await
                .unwrap();
            assert_eq!(res.status(), StatusCode::OK);
        }

        // Next request is limited; Retry-After must be on the order of the
        // window (≈50s), not the old hardcoded 1-second refill.
        let res = svc
            .call(Request::new(
                http::Request::builder()
                    .uri("/")
                    .body(Body::empty())
                    .unwrap(),
            ))
            .await
            .unwrap();
        assert_eq!(res.status(), StatusCode::TOO_MANY_REQUESTS);
        let retry_after: u64 = res
            .headers()
            .get("retry-after")
            .unwrap()
            .to_str()
            .unwrap()
            .parse()
            .unwrap();
        // Time to the next token is window/capacity = 50/5 = 10s — an order of
        // magnitude beyond the old hardcoded 1-second refill, proving the
        // window parameter now drives the refill rate.
        assert!(
            (9..=50).contains(&retry_after),
            "retry-after={retry_after}s should be ~window/capacity = 10s"
        );
    }

    // ── M9: sharding + eviction ──────────────────────────────────────────────

    #[test]
    fn idle_buckets_are_evicted() {
        let store = BucketStore::new(Duration::from_secs(1));
        assert_eq!(
            store.try_consume("stale-key", 10, Duration::from_secs(1)),
            None
        );

        // Age every bucket past the TTL.
        for shard in &store.shards {
            for bucket in shard.buckets.lock().values_mut() {
                bucket.last_seen -= IDLE_TTL + Duration::from_secs(1);
            }
        }

        // Sweep each shard (mirrors the in-line sweep in try_consume).
        for shard in &store.shards {
            shard
                .buckets
                .lock()
                .retain(|_, bucket| bucket.last_seen.elapsed() < IDLE_TTL);
        }
        let live: usize = store.shards.iter().map(|s| s.buckets.lock().len()).sum();
        assert_eq!(live, 0, "idle buckets should be gone");

        // A fresh bucket is created for the next request.
        assert_eq!(
            store.try_consume("stale-key", 10, Duration::from_secs(1)),
            None
        );
        let live: usize = store.shards.iter().map(|s| s.buckets.lock().len()).sum();
        assert_eq!(live, 1);
    }

    #[test]
    fn distinct_keys_land_on_independent_locks_without_error() {
        let store = Arc::new(BucketStore::new(Duration::from_secs(1)));
        let handles: Vec<_> = (0..8)
            .map(|i| {
                let store = Arc::clone(&store);
                std::thread::spawn(move || {
                    for j in 0..250 {
                        store.try_consume(
                            format!("key-{i}-{j}").as_str(),
                            10,
                            Duration::from_secs(1),
                        );
                    }
                })
            })
            .collect();
        for handle in handles {
            handle.join().unwrap();
        }
        let total: usize = store.shards.iter().map(|s| s.buckets.lock().len()).sum();
        assert_eq!(total, 8 * 250);
    }

    // ── test double ──────────────────────────────────────────────────────────

    #[derive(Clone)]
    struct PassthroughService;

    impl Service<Request> for PassthroughService {
        type Response = Response;
        type Error = Infallible;
        type Future = Pin<Box<dyn Future<Output = Result<Response, Infallible>> + Send + 'static>>;

        fn poll_ready(&mut self, _: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
            Poll::Ready(Ok(()))
        }

        fn call(&mut self, _: Request) -> Self::Future {
            Box::pin(async {
                Ok(http::Response::builder()
                    .status(StatusCode::OK)
                    .body(Body::empty())
                    .unwrap())
            })
        }
    }
}
