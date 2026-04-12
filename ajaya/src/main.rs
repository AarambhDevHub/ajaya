//! Ajaya (अजय) — The Unconquerable Rust Web Framework
//!
//! Entry point binary demonstrating path-based routing
//! with parameters and wildcards.

use ajaya::{Error, Json, PathParams, Request, Router, get, serve_app};
use http::StatusCode;
use tracing_subscriber::EnvFilter;

/// GET / — JSON health check.
async fn health() -> Result<Json<serde_json::Value>, Error> {
    Ok(Json(serde_json::json!({
        "status": "healthy",
        "framework": "Ajaya",
        "version": "0.1.x"
    })))
}

/// GET /users — list users.
async fn list_users() -> Json<serde_json::Value> {
    Json(serde_json::json!({
        "users": [
            { "id": 1, "name": "Alice" },
            { "id": 2, "name": "Bob" }
        ]
    }))
}

/// POST /users — create a user.
async fn create_user() -> (StatusCode, Json<serde_json::Value>) {
    (
        StatusCode::CREATED,
        Json(serde_json::json!({
            "id": 3,
            "name": "Charlie",
            "status": "created"
        })),
    )
}

/// GET /users/:id — get user by ID (path parameter).
async fn get_user(req: Request) -> Json<serde_json::Value> {
    let id = req
        .extension::<PathParams>()
        .and_then(|p| p.get("id"))
        .unwrap_or("unknown");

    Json(serde_json::json!({
        "id": id,
        "name": "User from path param"
    }))
}

/// GET /files/*path — wildcard catch-all.
async fn serve_file(req: Request) -> String {
    let path = req
        .extension::<PathParams>()
        .and_then(|p| p.get("path"))
        .unwrap_or("unknown");

    format!("Serving file: {path}")
}

/// Custom 404 handler.
async fn not_found() -> (StatusCode, &'static str) {
    (StatusCode::NOT_FOUND, "🔱 Ajaya: Page not found")
}

#[tokio::main]
async fn main() {
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
    ║     🔱  Ajaya (अजय) v0.1.x                   ║
    ║     The Unconquerable Rust Web Framework       ║
    ║                                               ║
    ║     → http://localhost:8080                    ║
    ║                                               ║
    ║     Routes:                                    ║
    ║       GET  /           → health check          ║
    ║       GET  /users      → list users            ║
    ║       POST /users      → create user           ║
    ║       GET  /users/:id  → get user by ID        ║
    ║       GET  /files/*p   → wildcard file serve   ║
    ║       *    *           → 404 Not Found          ║
    ║                                               ║
    ╚═══════════════════════════════════════════════╝
"#
    );

    let app = Router::new()
        .route("/", get(health))
        .route("/users", get(list_users).post(create_user))
        .route("/users/:id", get(get_user))
        .route("/files/*path", get(serve_file))
        .fallback(not_found);

    if let Err(e) = serve_app("0.0.0.0:8080", app).await {
        tracing::error!("Server error: {}", e);
        std::process::exit(1);
    }
}
