use std::time::Duration;

use arvik::{
    Router, ShutdownConfig, default_shutdown_signal, get,
    serve_with_config_and_graceful_shutdown,
};

async fn home() -> &'static str {
    "Press Ctrl+C to stop gracefully"
}

async fn slow() -> &'static str {
    tokio::time::sleep(Duration::from_secs(2)).await;
    "slow request completed"
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let app = Router::new()
        .route("/", get(home))
        .route("/slow", get(slow));

    let shutdown = ShutdownConfig::default()
        .drain_timeout(Duration::from_secs(10))
        .on_connected(|info| println!("connected: {}", info.peer_addr))
        .on_disconnected(|info| println!("disconnected: {}", info.peer_addr));

    serve_with_config_and_graceful_shutdown(
        app,
        "127.0.0.1:8080",
        arvik::ServerConfig::default(),
        shutdown,
        default_shutdown_signal(),
    )
    .await
}
