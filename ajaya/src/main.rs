//! Ajaya (अजय) — The Unconquerable Rust Web Framework
//!
//! Entry point binary demonstrating path-based routing
//! with parameters, extractors, and wildcards.

use ajaya::{
    AppendHeaders, Cookie, CookieJar, CookieKey, Error, ErrorResponse, FromRef, IntoResponse, Json,
    Multipart, Path, Query, Router, SignedCookieJar, State, StreamBody, get, post, serve_app,
};
use bytes::Bytes;
use futures_util::stream;
use http::StatusCode;
use http::header::CACHE_CONTROL;
use serde::{Deserialize, Serialize};
use std::io;
use tracing_subscriber::EnvFilter;

#[derive(Clone)]
struct AppState {
    app_name: String,
    cookie_key: CookieKey,
}

impl FromRef<AppState> for CookieKey {
    fn from_ref(state: &AppState) -> Self {
        state.cookie_key.clone()
    }
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

/// GET /stream — Streaming a large file
async fn stream_data() -> StreamBody<impl futures_util::Stream<Item = Result<Bytes, io::Error>>> {
    // In real code: tokio_util::io::ReaderStream::new(file)
    let chunks = stream::iter(vec![
        Ok(Bytes::from("chunk 1 ")),
        Ok(Bytes::from("chunk 2")),
    ]);
    StreamBody::new(chunks)
}

/// GET /cached — Appending headers
async fn cached_data() -> impl IntoResponse {
    (
        AppendHeaders([(CACHE_CONTROL, "public, max-age=3600")]),
        Json(serde_json::json!({ "data": "cached value" })),
    )
}

/// POST /login — Cookie session login
async fn login(jar: CookieJar) -> (CookieJar, &'static str) {
    let jar = jar.add(
        Cookie::build(("session", "s3cr3t"))
            .http_only(true)
            .secure(true)
            .same_site(cookie::SameSite::Strict)
            .build(),
    );
    (jar, "Logged in!")
}

/// POST /logout — Cookie session logout
async fn logout(jar: CookieJar) -> (CookieJar, &'static str) {
    let jar = jar.remove(Cookie::from("session"));
    (jar, "Logged out!")
}

/// POST /set_user — Signed cookie with app state
async fn set_user(jar: SignedCookieJar) -> (SignedCookieJar, &'static str) {
    let jar = jar.add(Cookie::new("user_id", "42"));
    (jar, "User cookie signed and set!")
}

/// GET /get_user — Signed cookie with app state
async fn get_user_cookie(jar: SignedCookieJar) -> String {
    jar.get("user_id")
        .map(|c| format!("user_id={}", c.value()))
        .unwrap_or_else(|| "no session".into())
}

/// Structured error responses
#[derive(Debug)]
enum _AppError {
    NotFound(String),
    Unauthorized,
    Internal(Box<dyn std::error::Error + Send + Sync>),
}

impl IntoResponse for _AppError {
    fn into_response(self) -> ajaya::Response {
        match self {
            _AppError::NotFound(msg) => ErrorResponse::new(StatusCode::NOT_FOUND)
                .message(msg)
                .into_response(),
            _AppError::Unauthorized => ErrorResponse::new(StatusCode::UNAUTHORIZED)
                .message("Unauthorized")
                .into_response(),
            _AppError::Internal(e) => {
                tracing::error!("Internal error: {e}");
                ErrorResponse::new(StatusCode::INTERNAL_SERVER_ERROR)
                    .message("Internal server error")
                    .into_response()
            }
        }
    }
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
        cookie_key: CookieKey::generate(),
    };

    let app = Router::new()
        .route("/", get(health))
        .route("/state", get(read_state))
        .route("/users", get(list_users).post(create_user))
        .route("/users/{id}", get(get_user))
        .route("/upload", post(upload))
        .route("/stream", get(stream_data))
        .route("/cached", get(cached_data))
        .route("/login", post(login))
        .route("/logout", post(logout))
        .route("/set_user", post(set_user))
        .route("/get_user", get(get_user_cookie))
        .route("/files/{*path}", get(serve_file))
        .fallback(not_found)
        .with_state(state);

    if let Err(e) = serve_app("0.0.0.0:8080", app).await {
        tracing::error!("Server error: {}", e);
        std::process::exit(1);
    }
}
