use std::time::Duration;

use arvik::{Router, ServerConfig, get, serve_h2c_with_config};

async fn hello() -> &'static str {
    "Hello from Arvik over h2c"
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    tracing_subscriber::fmt().init();

    let app = Router::new().route("/", get(hello));
    let config = ServerConfig::new()
        .http2_adaptive_window(true)
        .http2_max_concurrent_streams(1_000)
        .http2_keep_alive_interval(Duration::from_secs(20))
        .http2_keep_alive_timeout(Duration::from_secs(10));

    serve_h2c_with_config(app, "0.0.0.0:8080", config).await
}
