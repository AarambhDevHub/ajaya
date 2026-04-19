//! Ajaya (अजय) — The Unconquerable Rust Web Framework
//!
//! Entry point binary demonstrating path-based routing
//! with parameters, extractors, and wildcards.

use ajaya::{Error, Json, Multipart, Path, Query, Router, State, get, post, serve_app};
use http::StatusCode;
use serde::{Deserialize, Serialize};
use tracing_subscriber::EnvFilter;

#[derive(Clone)]
struct AppState {
    app_name: String,
}

#[derive(Serialize)]
struct User {
    id: u64,
    name: String,
}

#[derive(Deserialize)]
struct CreateUser {
    name: String,
}

#[derive(Deserialize)]
struct SearchParams {
    query: String,
}

/// GET /state — read app state.
async fn read_state(State(state): State<AppState>) -> String {
    format!("App name from state: {}", state.app_name)
}

/// POST /upload — multipart upload.
async fn upload(mut multipart: Multipart) -> String {
    let mut count = 0;
    while let Ok(Some(_field)) = multipart.next_field().await {
        count += 1;
    }
    format!("Received {} fields", count)
}

/// GET / — JSON health check.
async fn health() -> Result<Json<serde_json::Value>, Error> {
    Ok(Json(serde_json::json!({
        "status": "healthy",
        "framework": "Ajaya",
        "version": "0.2.x"
    })))
}

/// GET /users — list users (with optional query parameter).
async fn list_users(query: Option<Query<SearchParams>>) -> Json<serde_json::Value> {
    match query {
        Some(Query(params)) => Json(serde_json::json!({
            "message": format!("Searching users for: {}", params.query),
            "users": []
        })),
        None => Json(serde_json::json!({
            "users": [
                { "id": 1, "name": "Alice" },
                { "id": 2, "name": "Bob" }
            ]
        })),
    }
}

/// POST /users — create a user from JSON body.
async fn create_user(Json(body): Json<CreateUser>) -> (StatusCode, Json<User>) {
    (
        StatusCode::CREATED,
        Json(User {
            id: 3,
            name: body.name,
        }),
    )
}

/// GET /users/:id — get user by ID (type-safe path parameter extraction).
async fn get_user(Path(id): Path<u64>) -> Json<User> {
    Json(User {
        id,
        name: "User from path param".to_string(),
    })
}

/// GET /files/*path — wildcard catch-all.
async fn serve_file(Path(path): Path<String>) -> String {
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
    ║     🔱  Ajaya (अजय) v0.2.x                   ║
    ║     The Unconquerable Rust Web Framework       ║
    ║                                               ║
    ║     → http://localhost:8080                    ║
    ║                                               ║
    ║     Routes:                                    ║
    ║       GET  /           → health check          ║
    ║       GET  /state      → read app state        ║
    ║       GET  /users      → list users (query)    ║
    ║       POST /users      → create user (json)    ║
    ║       GET  /users/:id  → get user by ID        ║
    ║       POST /upload     → multipart upload      ║
    ║       GET  /files/*p   → wildcard file serve   ║
    ║       *    *           → 404 Not Found          ║
    ║                                               ║
    ╚═══════════════════════════════════════════════╝
"#
    );

    let state = AppState {
        app_name: "Ajaya Framework (v0.2.6)".to_string(),
    };

    let app = Router::new()
        .route("/", get(health))
        .route("/state", get(read_state))
        .route("/users", get(list_users).post(create_user))
        .route("/users/{id}", get(get_user))
        .route("/upload", post(upload))
        .route("/files/{*path}", get(serve_file))
        .fallback(not_found)
        .with_state(state);

    if let Err(e) = serve_app("0.0.0.0:8080", app).await {
        tracing::error!("Server error: {}", e);
        std::process::exit(1);
    }
}
