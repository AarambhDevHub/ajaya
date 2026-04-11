//! Ajaya (अजय) — The Unconquerable Rust Web Framework
//!
//! Entry point binary. Starts the HTTP server on port 8080.

use tracing_subscriber::EnvFilter;

#[tokio::main]
async fn main() {
    // Initialize tracing subscriber with env filter
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")),
        )
        .with_target(false)
        .init();

    println!(
        r#"
    ╔═══════════════════════════════════════════════╗
    ║                                               ║
    ║     🔱  Ajaya (अजय) v0.0.1                   ║
    ║     The Unconquerable Rust Web Framework       ║
    ║                                               ║
    ║     → http://localhost:8080                    ║
    ║                                               ║
    ╚═══════════════════════════════════════════════╝
"#
    );

    if let Err(e) = ajaya::serve("0.0.0.0:8080").await {
        tracing::error!("Server error: {}", e);
        std::process::exit(1);
    }
}
