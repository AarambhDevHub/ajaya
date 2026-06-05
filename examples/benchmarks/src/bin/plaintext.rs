use arvik::{Router, get, serve_app};

async fn plaintext() -> &'static str {
    "Hello, World!"
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let app = Router::new().route("/plaintext", get(plaintext));
    serve_app("0.0.0.0:8080", app).await
}
