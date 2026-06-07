use std::path::PathBuf;

use arvik::{Router, RuntimeConfig, ServeDir, ServerConfig, serve_with_config};

fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    RuntimeConfig::benchmark_http1().build()?.block_on(async {
        let assets = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("assets");
        let app = Router::new().nest_service(
            "/static",
            ServeDir::new(assets)
                .append_index_html_on_directories(true)
                .cache_control("public, max-age=60"),
        );

        serve_with_config(app, "0.0.0.0:8080", ServerConfig::benchmark_http1()).await
    })
}
