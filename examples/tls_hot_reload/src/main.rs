use std::time::Duration;

use arvik::{Router, RustlsConfig, get, serve_tls};

async fn hello() -> &'static str {
    "Hello from Arvik with hot-reloadable TLS"
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    tracing_subscriber::fmt().init();

    let cert_path = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "cert.pem".to_string());
    let key_path = std::env::args()
        .nth(2)
        .unwrap_or_else(|| "key.pem".to_string());

    let tls = RustlsConfig::from_pem_file(&cert_path, &key_path).await?;
    let _watcher = tls.watch_pem_files(&cert_path, &key_path, Duration::from_secs(1))?;

    let app = Router::new().route("/", get(hello));
    serve_tls(app, "0.0.0.0:8443", tls).await
}
