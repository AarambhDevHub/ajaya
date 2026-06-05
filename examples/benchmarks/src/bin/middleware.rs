use arvik::{CompressionLayer, RequestIdLayer, Router, SecurityHeadersLayer, get, serve_app};

async fn payload() -> &'static str {
    "Arvik benchmark response body repeated enough to cross compression thresholds. \
     Arvik benchmark response body repeated enough to cross compression thresholds."
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let app = Router::new()
        .route("/middleware", get(payload))
        .layer(SecurityHeadersLayer::new())
        .layer(RequestIdLayer::new())
        .layer(CompressionLayer::new().min_size(64));

    serve_app("0.0.0.0:8080", app).await
}
