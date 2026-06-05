use arvik::health::{add_check, health_handler, liveness_handler, readiness_handler, startup_handler};
use arvik::{Router, get, serve_app};

async fn home() -> &'static str {
    "health checks example"
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    add_check("database", || async { true });
    add_check("redis", || async { Ok::<_, &'static str>(()) });

    let app = Router::new()
        .route("/", get(home))
        .route("/health", get(health_handler))
        .route("/health/live", get(liveness_handler))
        .route("/health/ready", get(readiness_handler))
        .route("/health/startup", get(startup_handler));

    serve_app("0.0.0.0:8080", app).await?;
    Ok(())
}
