//! Ajaya (अजय) — The Unconquerable Rust Web Framework
//!
//! Entry point binary demonstrating path-based routing with parameters,
//! extractors, wildcards, cookies, streaming, and — in v0.4.2 — the new
//! function-based middleware API.

use ajaya::{
    AppendHeaders, Cookie, CookieJar, CookieKey, Error, ErrorResponse, FromRef, IntoResponse, Json,
    Multipart, Path, Query, Request, Response, Router, SignedCookieJar, State, StreamBody, get,
    middleware::{Next, from_fn, from_fn_with_state, map_response},
    post, serve_app,
};
use bytes::Bytes;
use futures_util::stream;
use http::{StatusCode, header::CACHE_CONTROL};
use serde::{Deserialize, Serialize};
use std::{io, sync::Arc};
use tracing_subscriber::EnvFilter;

// ── Application State ─────────────────────────────────────────────────────────

#[derive(Clone)]
struct AppState {
    app_name: String,
    cookie_key: CookieKey,
    /// Shared request counter — demonstrates Arc-wrapped mutable state.
    request_count: Arc<std::sync::atomic::AtomicU64>,
}

impl FromRef<AppState> for CookieKey {
    fn from_ref(state: &AppState) -> Self {
        state.cookie_key.clone()
    }
}

// ── Middleware (v0.4.2 style — plain async functions) ─────────────────────────

/// Attach a UUID to every request via extensions.
///
/// Before (v0.4.1): 30+ lines of Service + Layer boilerplate.
/// After  (v0.4.2): 4 lines.
async fn attach_request_id(mut req: Request, next: Next) -> Response {
    req.extensions_mut()
        .insert(uuid::Uuid::new_v4().to_string());
    next.run(req).await
}

/// Log method, path and status for every request.
async fn log_requests(req: Request, next: Next) -> Response {
    let method = req.method().clone();
    let path = req.uri().path().to_string();
    let res = next.run(req).await;
    tracing::info!("{} {} → {}", method, path, res.status());
    res
}

/// Count requests using shared state via `from_fn_with_state`.
async fn count_requests(State(state): State<AppState>, req: Request, next: Next) -> Response {
    let n = state
        .request_count
        .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    tracing::debug!("Request #{}", n + 1);
    next.run(req).await
}

/// Append `x-powered-by: ajaya` to every response using `map_response`.
async fn add_powered_by_header(mut res: Response) -> Response {
    res.headers_mut()
        .insert("x-powered-by", "ajaya".parse().unwrap());
    res
}

// ── Route types ───────────────────────────────────────────────────────────────

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

// ── Route handlers ────────────────────────────────────────────────────────────

/// GET / — JSON health check.
async fn health() -> Result<Json<serde_json::Value>, Error> {
    Ok(Json(serde_json::json!({
        "status": "healthy",
        "framework": "Ajaya",
        "version": "0.4.2"
    })))
}

/// GET /state — read app state.
async fn read_state(State(state): State<AppState>) -> String {
    format!("App: {}", state.app_name)
}

/// GET /users — list users (optional query).
async fn list_users(query: Option<Query<SearchParams>>) -> Json<serde_json::Value> {
    match query {
        Some(Query(p)) => Json(serde_json::json!({
            "message": format!("Searching: {}", p.query),
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

/// POST /users — create user from JSON body.
async fn create_user(Json(body): Json<CreateUser>) -> (StatusCode, Json<User>) {
    (
        StatusCode::CREATED,
        Json(User {
            id: 3,
            name: body.name,
        }),
    )
}

/// GET /users/:id — get user by ID.
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

/// GET /stream — streaming response.
async fn stream_data() -> StreamBody<impl futures_util::Stream<Item = Result<Bytes, io::Error>>> {
    let chunks = stream::iter(vec![
        Ok(Bytes::from("chunk 1 ")),
        Ok(Bytes::from("chunk 2")),
    ]);
    StreamBody::new(chunks)
}

/// GET /cached — response with cache headers.
async fn cached_data() -> impl IntoResponse {
    (
        AppendHeaders([(CACHE_CONTROL, "public, max-age=3600")]),
        Json(serde_json::json!({ "data": "cached value" })),
    )
}

/// POST /upload — multipart upload.
async fn upload(mut multipart: Multipart) -> String {
    let mut count = 0;
    while let Ok(Some(_field)) = multipart.next_field().await {
        count += 1;
    }
    format!("Received {} fields", count)
}

/// POST /login — plain cookie session.
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

/// POST /logout — remove session cookie.
async fn logout(jar: CookieJar) -> (CookieJar, &'static str) {
    let jar = jar.remove(Cookie::from("session"));
    (jar, "Logged out!")
}

/// POST /set_user — signed cookie.
async fn set_user(jar: SignedCookieJar) -> (SignedCookieJar, &'static str) {
    let jar = jar.add(Cookie::new("user_id", "42"));
    (jar, "User cookie signed and set!")
}

/// GET /get_user — read signed cookie.
async fn get_user_cookie(jar: SignedCookieJar) -> String {
    jar.get("user_id")
        .map(|c| format!("user_id={}", c.value()))
        .unwrap_or_else(|| "no session".into())
}

/// Custom 404 fallback.
async fn not_found() -> (StatusCode, &'static str) {
    (StatusCode::NOT_FOUND, "🔱 Ajaya: Page not found")
}

// ── Custom AppError (demonstrating structured errors) ─────────────────────────

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

// ── Main ──────────────────────────────────────────────────────────────────────

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
    ║     🔱  Ajaya (अजय) v0.4.2                   ║
    ║     The Unconquerable Rust Web Framework       ║
    ║                                               ║
    ║     → http://localhost:8080                    ║
    ║                                               ║
    ║     Middleware Stack (outermost → innermost):  ║
    ║       add_powered_by_header (map_response)     ║
    ║       log_requests          (from_fn)          ║
    ║       count_requests        (from_fn_w_state)  ║
    ║       attach_request_id     (from_fn)          ║
    ║       [route handler]                          ║
    ║                                               ║
    ║     Routes:                                    ║
    ║       GET  /            → health check         ║
    ║       GET  /state       → read app state       ║
    ║       GET  /users       → list users           ║
    ║       POST /users       → create user (json)   ║
    ║       GET  /users/:id  → get user by ID       ║
    ║       POST /upload      → multipart upload     ║
    ║       GET  /stream      → streaming body       ║
    ║       GET  /cached      → cache headers        ║
    ║       POST /login       → cookie login         ║
    ║       POST /logout      → cookie logout        ║
    ║       POST /set_user    → signed cookie        ║
    ║       GET  /get_user    → read signed cookie   ║
    ║       GET  /files/*p    → wildcard file        ║
    ║       *    *            → 404 Not Found        ║
    ║                                               ║
    ╚═══════════════════════════════════════════════╝
"#
    );

    let state = AppState {
        app_name: "Ajaya Framework (v0.4.2)".to_string(),
        cookie_key: CookieKey::generate(),
        request_count: Arc::new(std::sync::atomic::AtomicU64::new(0)),
    };

    let app = Router::new()
        // ── Routes ──────────────────────────────────────────────────────────
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
        .with_state(state.clone())
        // ── Middleware stack (last .layer() = outermost = first to run) ─────
        //
        //   v0.4.2 style: plain async functions, zero boilerplate.
        //
        // Innermost — runs last on request, first on response:
        .layer(from_fn(attach_request_id))
        // Runs after request_id is attached:
        .layer(from_fn_with_state(state, count_requests))
        // Runs next — logs method, path, status:
        .layer(from_fn(log_requests))
        // Outermost — appends x-powered-by to every response:
        .layer(map_response(add_powered_by_header));

    if let Err(e) = serve_app("0.0.0.0:8080", app).await {
        tracing::error!("Server error: {}", e);
        std::process::exit(1);
    }
}
