use arvik::metrics::{PrometheusMetricsLayer, metrics_handler};
use arvik::{Router, get, serve_app};

async fn home() -> &'static str {
    "metrics example"
}

async fn user() -> &'static str {
    "user"
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let metrics = PrometheusMetricsLayer::new()
        .service_name("arvik-metrics")
        .version(env!("CARGO_PKG_VERSION"))
        .environment("development");

    let app = Router::new()
        .route("/", get(home))
        .route("/users/{id}", get(user))
        .route("/metrics", get(metrics_handler))
        .layer(metrics);

    serve_app("0.0.0.0:8080", app).await?;
    Ok(())
}
