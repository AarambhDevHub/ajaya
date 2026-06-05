use arvik::{
    ArvikConfig, RequestBodyLimitLayer, Router, default_shutdown_signal, get,
    serve_with_config_and_graceful_shutdown,
};

async fn home() -> &'static str {
    "Hello from configured Arvik"
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let config_path = concat!(env!("CARGO_MANIFEST_DIR"), "/arvik.toml");
    let config = ArvikConfig::builder().file(config_path).build()?;

    let mut app = Router::new().route("/", get(home));
    if let Some(limit) = config.server.body_limit {
        app = app.layer(RequestBodyLimitLayer::new(limit));
    }

    println!("listening on http://{}", config.bind_addr_string());
    serve_with_config_and_graceful_shutdown(
        app,
        &config.bind_addr_string(),
        config.server_config(),
        config.shutdown_config(),
        default_shutdown_signal(),
    )
    .await
}
