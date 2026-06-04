use arvik::{Embed, EmbeddedFileService, Router, get, serve_app};

#[derive(Embed)]
#[folder = "assets"]
#[crate_path = "arvik"]
struct Assets;

async fn home() -> &'static str {
    "Open http://127.0.0.1:8080/assets/"
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    tracing_subscriber::fmt().init();

    let app = Router::new().route("/", get(home)).nest_service(
        "/assets",
        EmbeddedFileService::<Assets>::new()
            .precompressed_gzip()
            .precompressed_br()
            .cache_control("public, max-age=31536000, immutable"),
    );

    serve_app("0.0.0.0:8080", app).await
}
