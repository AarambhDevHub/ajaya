use arvik::{Router, RuntimeConfig, ServerConfig, get, serve_with_config};

async fn plaintext() -> &'static str {
    "Hello, World!"
}

fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    RuntimeConfig::benchmark_http1().build()?.block_on(async {
        let app = Router::new().route("/plaintext", get(plaintext));
        serve_with_config(app, "0.0.0.0:8080", ServerConfig::benchmark_http1()).await
    })
}
