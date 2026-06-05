use arvik::logging::{ArvikLogger, StructuredLoggingLayer};
use arvik::{Router, get, serve_app};

async fn home() -> &'static str {
    "structured logging example"
}

async fn user() -> &'static str {
    "user"
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    ArvikLogger::init()?;

    let app = Router::new()
        .route("/", get(home))
        .route("/users/{id}", get(user))
        .layer(StructuredLoggingLayer::new());

    serve_app("0.0.0.0:8080", app).await?;
    Ok(())
}
