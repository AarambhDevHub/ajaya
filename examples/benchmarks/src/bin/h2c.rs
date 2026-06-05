use arvik::{Router, ServerConfig, get, serve_h2c_with_config};

async fn plaintext() -> &'static str {
    "Hello, World!"
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let app = Router::new().route("/plaintext", get(plaintext));
    serve_h2c_with_config(app, "0.0.0.0:8080", ServerConfig::http2_high_throughput()).await
}
