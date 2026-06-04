use arvik::{Router, RustlsConfig, get, serve_tls};

async fn hello() -> &'static str {
    "Hello from Arvik over rustls TLS"
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    tracing_subscriber::fmt().init();

    let app = Router::new().route("/", get(hello));
    let tls = RustlsConfig::self_signed(["localhost", "127.0.0.1"]).await?;

    serve_tls(app, "0.0.0.0:8443", tls).await
}
