use arvik::{Router, ServeDir, ServeFile, get, serve_app};
use std::path::PathBuf;

async fn home() -> &'static str {
    "Open http://127.0.0.1:8080/static/"
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    tracing_subscriber::fmt().init();

    let assets = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("assets");
    let favicon = assets.join("favicon.ico");

    let app = Router::new()
        .route("/", get(home))
        .route_service("/favicon.ico", ServeFile::new(favicon))
        .nest_service(
            "/static",
            ServeDir::new(assets)
                .precompressed_gzip()
                .precompressed_br()
                .directory_listing(true)
                .cache_control("public, max-age=60"),
        );

    serve_app("0.0.0.0:8080", app).await
}
