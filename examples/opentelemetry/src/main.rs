use arvik::trace::{OtelConfig, OtelLayer};
use arvik::{Router, get, serve_app};

async fn home() -> &'static str {
    "opentelemetry example"
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let _otel = OtelConfig::new("arvik-opentelemetry").stdout().install()?;

    let app = Router::new()
        .route("/", get(home))
        .layer(OtelLayer::new("arvik-opentelemetry"));

    serve_app("0.0.0.0:8080", app).await?;
    Ok(())
}
