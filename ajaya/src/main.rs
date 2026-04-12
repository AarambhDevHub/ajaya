//! Ajaya (अजय) — The Unconquerable Rust Web Framework
//!
//! Entry point binary. Starts the HTTP server on port 8080
//! with method-based routing and JSON response support.

use ajaya::{Error, Json, get, serve_router};
use http::StatusCode;
use tracing_subscriber::EnvFilter;

/// GET handler — returns a JSON health check using Result.
async fn health() -> Result<Json<serde_json::Value>, Error> {
    Ok(Json(serde_json::json!({
        "status": "healthy",
        "framework": "Ajaya",
        "version": "0.0.5"
    })))
}

/// POST handler — echoes a JSON response with created status.
async fn create() -> (StatusCode, Json<serde_json::Value>) {
    (
        StatusCode::CREATED,
        Json(serde_json::json!({
            "status": "created",
            "id": 42
        })),
    )
}

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
    ║     🔱  Ajaya (अजय) v0.0.5                   ║
    ║     The Unconquerable Rust Web Framework       ║
    ║                                               ║
    ║     → http://localhost:8080                    ║
    ║                                               ║
    ║     Routes:                                    ║
    ║       GET  /  → JSON health check              ║
    ║       POST /  → JSON created response          ║
    ║       *    /  → 405 Method Not Allowed          ║
    ║                                               ║
    ╚═══════════════════════════════════════════════╝
"#
    );

    // Create a method router with GET and POST handlers
    let router = get(health).post(create);

    if let Err(e) = serve_router("0.0.0.0:8080", router).await {
        tracing::error!("Server error: {}", e);
        std::process::exit(1);
    }
}
