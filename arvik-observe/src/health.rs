//! Health, liveness, readiness, and startup probes.

use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use arvik_core::{Response, ResponseBuilder};
use futures_util::future::join_all;
use http::StatusCode;
use once_cell::sync::Lazy;
use parking_lot::{Mutex, RwLock};
use serde::Serialize;

const DEFAULT_CHECK_TIMEOUT: Duration = Duration::from_secs(2);

type BoxCheckFuture = Pin<Box<dyn Future<Output = HealthCheckResult> + Send + 'static>>;
type CheckFn = dyn Fn() -> BoxCheckFuture + Send + Sync + 'static;

/// Registry for health checks and process probe state.
#[derive(Clone)]
pub struct HealthRegistry {
    inner: Arc<HealthInner>,
}

struct HealthInner {
    started_at: Instant,
    checks: RwLock<Vec<HealthCheck>>,
    check_timeout: RwLock<Duration>,
    startup_complete: AtomicBool,
    readiness_cache_ttl: RwLock<Duration>,
    readiness_cache: parking_lot::Mutex<Option<(Instant, Vec<CheckReport>)>>,
}

#[derive(Clone)]
struct HealthCheck {
    name: Arc<str>,
    run: Arc<CheckFn>,
}

/// Result returned by a readiness check.
#[derive(Debug, Clone, Serialize)]
pub struct HealthCheckResult {
    /// Whether this dependency is healthy.
    pub healthy: bool,
    /// Optional human-readable detail.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
}

impl HealthCheckResult {
    /// Construct a passing check result.
    pub fn ok() -> Self {
        Self {
            healthy: true,
            message: None,
        }
    }

    /// Construct a failing check result with a message.
    pub fn unhealthy(message: impl Into<String>) -> Self {
        Self {
            healthy: false,
            message: Some(message.into()),
        }
    }
}

/// Conversion trait accepted by [`HealthRegistry::add_check`].
pub trait IntoHealthCheckResult {
    /// Convert into a check result.
    fn into_health_check_result(self) -> HealthCheckResult;
}

impl IntoHealthCheckResult for HealthCheckResult {
    fn into_health_check_result(self) -> HealthCheckResult {
        self
    }
}

impl IntoHealthCheckResult for bool {
    fn into_health_check_result(self) -> HealthCheckResult {
        if self {
            HealthCheckResult::ok()
        } else {
            HealthCheckResult::unhealthy("check returned false")
        }
    }
}

impl<T, E> IntoHealthCheckResult for Result<T, E>
where
    E: std::fmt::Display,
{
    fn into_health_check_result(self) -> HealthCheckResult {
        match self {
            Ok(_) => HealthCheckResult::ok(),
            Err(err) => {
                // Driver/DSN text must not reach the public probe body during
                // an outage; keep the detail in logs for operators.
                tracing::error!(error = %err, "health check failed");
                HealthCheckResult::unhealthy("check failed")
            }
        }
    }
}

#[derive(Debug, Serialize)]
struct ProbeResponse {
    status: &'static str,
    uptime: u64,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    checks: Vec<CheckReport>,
}

#[derive(Debug, Clone, Serialize)]
struct CheckReport {
    name: String,
    status: &'static str,
    latency_ms: u128,
    #[serde(skip_serializing_if = "Option::is_none")]
    message: Option<String>,
}

/// How long a readiness probe result is reused before checks run again.
/// Bursts of kubelet/LB probes then cost one dependency round-trip instead of N.
const DEFAULT_READINESS_CACHE_TTL: Duration = Duration::from_secs(1);

impl HealthRegistry {
    /// Create a health registry. Startup is considered complete by default.
    pub fn new() -> Self {
        Self {
            inner: Arc::new(HealthInner {
                started_at: Instant::now(),
                checks: RwLock::new(Vec::new()),
                check_timeout: RwLock::new(DEFAULT_CHECK_TIMEOUT),
                startup_complete: AtomicBool::new(true),
                readiness_cache_ttl: RwLock::new(DEFAULT_READINESS_CACHE_TTL),
                readiness_cache: Mutex::new(None),
            }),
        }
    }

    /// Return the process-global health registry.
    pub fn global() -> Self {
        GLOBAL_HEALTH.clone()
    }

    /// Set the per-check readiness timeout.
    #[must_use]
    pub fn with_check_timeout(self, timeout: Duration) -> Self {
        *self.inner.check_timeout.write() = timeout;
        self
    }

    /// Make the startup probe fail until [`set_startup_complete`](Self::set_startup_complete).
    #[must_use]
    pub fn starting(self) -> Self {
        self.inner.startup_complete.store(false, Ordering::SeqCst);
        self
    }

    /// Mark startup as incomplete.
    pub fn mark_starting(&self) {
        self.inner.startup_complete.store(false, Ordering::SeqCst);
    }

    /// Mark startup as complete.
    pub fn set_startup_complete(&self) {
        self.inner.startup_complete.store(true, Ordering::SeqCst);
    }

    /// Return whether startup has completed.
    pub fn startup_complete(&self) -> bool {
        self.inner.startup_complete.load(Ordering::SeqCst)
    }

    /// Register a readiness check.
    pub fn add_check<F, Fut, R>(&self, name: impl Into<Arc<str>>, check: F) -> &Self
    where
        F: Fn() -> Fut + Send + Sync + 'static,
        Fut: Future<Output = R> + Send + 'static,
        R: IntoHealthCheckResult + 'static,
    {
        let check = Arc::new(move || {
            let future = check();
            Box::pin(async move { future.await.into_health_check_result() }) as BoxCheckFuture
        });
        self.inner.checks.write().push(HealthCheck {
            name: name.into(),
            run: check,
        });
        self
    }

    /// Build the general `/health` response.
    pub async fn health_response(&self) -> Response {
        self.json_response(
            StatusCode::OK,
            ProbeResponse {
                status: "ok",
                uptime: self.uptime_seconds(),
                checks: Vec::new(),
            },
        )
    }

    /// Build the `/health/live` response.
    pub async fn liveness_response(&self) -> Response {
        self.json_response(
            StatusCode::OK,
            ProbeResponse {
                status: "ok",
                uptime: self.uptime_seconds(),
                checks: Vec::new(),
            },
        )
    }

    /// Set how long a readiness result is reused before checks run again
    /// (default 1 s). A zero TTL disables caching.
    pub fn set_readiness_cache_ttl(&self, ttl: Duration) {
        *self.inner.readiness_cache_ttl.write() = ttl;
    }

    /// Build the `/health/ready` response.
    pub async fn readiness_response(&self) -> Response {
        let ttl = *self.inner.readiness_cache_ttl.read();

        let checks = if ttl.is_zero() {
            self.run_checks().await
        } else {
            let cached = self
                .inner
                .readiness_cache
                .lock()
                .clone()
                .filter(|(at, _)| at.elapsed() < ttl);
            match cached {
                Some((_, checks)) => checks,
                None => {
                    let checks = self.run_checks().await;
                    *self.inner.readiness_cache.lock() = Some((Instant::now(), checks.clone()));
                    checks
                }
            }
        };
        let healthy = checks.iter().all(|check| check.status == "ok");
        let status = if healthy {
            StatusCode::OK
        } else {
            StatusCode::SERVICE_UNAVAILABLE
        };
        self.json_response(
            status,
            ProbeResponse {
                status: if healthy { "ok" } else { "degraded" },
                uptime: self.uptime_seconds(),
                checks,
            },
        )
    }

    /// Build the `/health/startup` response.
    pub async fn startup_response(&self) -> Response {
        let complete = self.startup_complete();
        self.json_response(
            if complete {
                StatusCode::OK
            } else {
                StatusCode::SERVICE_UNAVAILABLE
            },
            ProbeResponse {
                status: if complete { "ok" } else { "starting" },
                uptime: self.uptime_seconds(),
                checks: Vec::new(),
            },
        )
    }

    async fn run_checks(&self) -> Vec<CheckReport> {
        let checks = self.inner.checks.read().clone();
        let timeout = *self.inner.check_timeout.read();

        join_all(checks.into_iter().map(|check| async move {
            let started = Instant::now();
            let result = tokio::time::timeout(timeout, (check.run)()).await;
            let latency_ms = started.elapsed().as_millis();
            match result {
                Ok(result) if result.healthy => CheckReport {
                    name: check.name.to_string(),
                    status: "ok",
                    latency_ms,
                    message: result.message,
                },
                Ok(result) => CheckReport {
                    name: check.name.to_string(),
                    status: "unhealthy",
                    latency_ms,
                    message: result.message,
                },
                Err(_) => CheckReport {
                    name: check.name.to_string(),
                    status: "timeout",
                    latency_ms,
                    message: Some(format!("check timed out after {}ms", timeout.as_millis())),
                },
            }
        }))
        .await
    }

    fn uptime_seconds(&self) -> u64 {
        self.inner.started_at.elapsed().as_secs()
    }

    fn json_response<T: Serialize>(&self, status: StatusCode, body: T) -> Response {
        ResponseBuilder::new().status(status).json(&body)
    }
}

impl Default for HealthRegistry {
    fn default() -> Self {
        Self::new()
    }
}

static GLOBAL_HEALTH: Lazy<HealthRegistry> = Lazy::new(HealthRegistry::new);

/// Register a readiness check on the process-global registry.
pub fn add_check<F, Fut, R>(name: impl Into<Arc<str>>, check: F)
where
    F: Fn() -> Fut + Send + Sync + 'static,
    Fut: Future<Output = R> + Send + 'static,
    R: IntoHealthCheckResult + 'static,
{
    HealthRegistry::global().add_check(name, check);
}

/// Process-global `/health` handler.
pub async fn health_handler() -> Response {
    HealthRegistry::global().health_response().await
}

/// Process-global `/health/live` handler.
pub async fn liveness_handler() -> Response {
    HealthRegistry::global().liveness_response().await
}

/// Process-global `/health/ready` handler.
pub async fn readiness_handler() -> Response {
    HealthRegistry::global().readiness_response().await
}

/// Process-global `/health/startup` handler.
pub async fn startup_handler() -> Response {
    HealthRegistry::global().startup_response().await
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::Value;

    async fn response_json(response: Response) -> Value {
        let bytes = response.into_body().to_bytes().await.unwrap();
        serde_json::from_slice(&bytes).unwrap()
    }

    #[tokio::test]
    async fn health_reports_uptime() {
        let registry = HealthRegistry::new();
        let response = registry.health_response().await;

        assert_eq!(response.status(), StatusCode::OK);
        let json = response_json(response).await;
        assert_eq!(json["status"], "ok");
        assert!(json["uptime"].as_u64().is_some());
    }

    #[tokio::test]
    async fn readiness_passes_when_checks_pass() {
        let registry = HealthRegistry::new();
        registry.add_check("database", || async { true });

        let response = registry.readiness_response().await;
        let status = response.status();
        let json = response_json(response).await;

        assert_eq!(status, StatusCode::OK);
        assert_eq!(json["checks"][0]["name"], "database");
        assert_eq!(json["checks"][0]["status"], "ok");
    }

    #[tokio::test]
    async fn readiness_fails_when_check_fails() {
        let registry = HealthRegistry::new();
        registry.add_check("redis", || async { HealthCheckResult::unhealthy("down") });

        let response = registry.readiness_response().await;
        let status = response.status();
        let json = response_json(response).await;

        assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE);
        assert_eq!(json["status"], "degraded");
        assert_eq!(json["checks"][0]["message"], "down");
    }

    #[tokio::test]
    async fn readiness_times_out_hung_checks() {
        let registry = HealthRegistry::new().with_check_timeout(Duration::from_millis(10));
        registry.add_check("external-api", || async {
            tokio::time::sleep(Duration::from_secs(10)).await;
            true
        });

        let response = registry.readiness_response().await;
        let status = response.status();
        let json = response_json(response).await;

        assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE);
        assert_eq!(json["checks"][0]["status"], "timeout");
    }

    #[tokio::test]
    async fn startup_can_be_gated() {
        let registry = HealthRegistry::new().starting();

        let response = registry.startup_response().await;
        assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);

        registry.set_startup_complete();
        let response = registry.startup_response().await;
        assert_eq!(response.status(), StatusCode::OK);
    }
}
