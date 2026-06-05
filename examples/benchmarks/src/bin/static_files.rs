use std::path::PathBuf;

use arvik::{Router, ServeDir, serve_app};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let assets = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("assets");
    let app = Router::new().nest_service(
        "/static",
        ServeDir::new(assets)
            .append_index_html_on_directories(true)
            .cache_control("public, max-age=60"),
    );

    serve_app("0.0.0.0:8080", app).await
}
