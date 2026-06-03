use arvik::{NativeTlsConfig, Router, get, serve_native_tls};

async fn hello() -> &'static str {
    "Hello from Arvik over native-tls"
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    tracing_subscriber::fmt().init();

    let pkcs12_path = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "identity.p12".to_string());
    let password = std::env::args().nth(2).unwrap_or_default();

    let tls = NativeTlsConfig::from_pkcs12_file(pkcs12_path, &password).await?;
    let app = Router::new().route("/", get(hello));

    serve_native_tls(app, "0.0.0.0:8443", tls).await
}
