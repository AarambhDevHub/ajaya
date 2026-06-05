# Arvik — Full Architecture & Feature Specification

> *Fast, Typed, and Fearless Web Framework for Rust*
> Built on Tokio + Hyper. Engineered to beat Actix-web in benchmarks.
> Every feature Axum and Actix-web have — unified under one ergonomic API.

---

## Table of Contents

1. [Workspace Structure](#1-workspace-structure)
2. [Crate Responsibilities](#2-crate-responsibilities)
3. [Core Abstractions](#3-core-abstractions)
4. [Routing System](#4-routing-system)
5. [Handler System](#5-handler-system)
6. [Extractor System](#6-extractor-system)
7. [Response System](#7-response-system)
8. [Middleware & Layers](#8-middleware--layers)
9. [State Management](#9-state-management)
10. [Error Handling](#10-error-handling)
11. [WebSockets](#11-websockets)
12. [Server-Sent Events (SSE)](#12-server-sent-events-sse)
13. [Multipart & File Uploads](#13-multipart--file-uploads)
14. [Static File Serving](#14-static-file-serving)
15. [TLS / HTTPS](#15-tls--https)
16. [HTTP/2 & HTTP/3](#16-http2--http3)
17. [gRPC Support](#17-grpc-support)
18. [Testing Utilities](#18-testing-utilities)
19. [Proc Macros](#19-proc-macros)
20. [Configuration System](#20-configuration-system)
21. [Observability](#21-observability)
22. [Security Features](#22-security-features)
23. [Connection & Server Tuning](#23-connection--server-tuning)
24. [Data Formats](#24-data-formats)
25. [Database Integration](#25-database-integration)
26. [Performance Architecture](#26-performance-architecture)
27. [Full Dependency Graph](#27-full-dependency-graph)
28. [Feature Flags](#28-feature-flags)
29. [Comparison: Arvik vs Axum vs Actix](#29-comparison-arvik-vs-axum-vs-actix)

---

## 1. Workspace Structure

```
arvik/
├── Cargo.toml                  # Workspace root
├── ARCHITECTURE.md
├── ROADMAP.md
├── README.md
│
├── arvik/                      # Top-level facade crate (re-exports everything)
│   ├── Cargo.toml
│   └── src/
│       └── lib.rs
│
├── arvik-core/                 # Core traits, types, request/response primitives
│   ├── Cargo.toml
│   └── src/
│       ├── lib.rs
│       ├── request.rs
│       ├── response.rs
│       ├── body.rs
│       ├── handler.rs
│       ├── into_response.rs
│       └── error.rs
│
├── arvik-router/               # Radix trie router, method dispatch, nested routers
│   ├── Cargo.toml
│   └── src/
│       ├── lib.rs
│       ├── trie.rs
│       ├── node.rs
│       ├── params.rs
│       ├── method_router.rs
│       └── nested.rs
│
├── arvik-hyper/                # Hyper server integration, connection management
│   ├── Cargo.toml
│   └── src/
│       ├── lib.rs
│       ├── server.rs
│       ├── serve.rs
│       ├── acceptor.rs
│       └── graceful.rs
│
├── arvik-extract/              # All extractors: Path, Query, Json, Form, Headers...
│   ├── Cargo.toml
│   └── src/
│       ├── lib.rs
│       ├── path.rs
│       ├── query.rs
│       ├── json.rs
│       ├── form.rs
│       ├── headers.rs
│       ├── state.rs
│       ├── body.rs
│       ├── multipart.rs
│       ├── connect_info.rs
│       └── rejection.rs
│
├── arvik-middleware/           # Built-in middleware (logging, CORS, compression, etc.)
│   ├── Cargo.toml
│   └── src/
│       ├── lib.rs
│       ├── cors.rs
│       ├── compression.rs
│       ├── decompression.rs
│       ├── timeout.rs
│       ├── rate_limit.rs
│       ├── auth.rs
│       ├── logger.rs
│       ├── request_id.rs
│       ├── propagate_header.rs
│       ├── sensitive_headers.rs
│       ├── set_header.rs
│       ├── trace.rs
│       └── catch_panic.rs
│
├── arvik-ws/                   # WebSocket upgrade + messaging
│   ├── Cargo.toml
│   └── src/
│       ├── lib.rs
│       ├── upgrade.rs
│       ├── socket.rs
│       └── message.rs
│
├── arvik-sse/                  # Server-Sent Events
│   ├── Cargo.toml
│   └── src/
│       ├── lib.rs
│       └── event.rs
│
├── arvik-static/               # Static file serving + directory listing
│   ├── Cargo.toml
│   └── src/
│       ├── lib.rs
│       ├── serve_dir.rs
│       └── serve_file.rs
│
├── arvik-tls/                  # TLS via rustls + native-tls
│   ├── Cargo.toml
│   └── src/
│       ├── lib.rs
│       ├── rustls.rs
│       └── native_tls.rs
│
├── arvik-macros/               # Proc macros: #[handler], #[route], #[debug_handler]
│   ├── Cargo.toml
│   └── src/
│       ├── lib.rs
│       ├── handler.rs
│       ├── route.rs
│       └── debug_handler.rs
│
├── arvik-test/                 # Testing utilities
│   ├── Cargo.toml
│   └── src/
│       ├── lib.rs
│       └── client.rs
│
└── examples/
    ├── hello_world/
    ├── rest_api/
    ├── websocket_chat/
    ├── file_upload/
    ├── grpc_server/
    ├── sse_stream/
    ├── auth_jwt/
    ├── postgres_crud/
    └── benchmarks/
```

---

## 2. Crate Responsibilities

| Crate | Responsibility | Public API Surface |
|---|---|---|
| `arvik` | Facade re-exporter, feature-gated convenience | Everything users need |
| `arvik-core` | `Request`, `Response`, `Body`, `Handler`, `IntoResponse`, `AjayaError` | Core traits + types |
| `arvik-router` | Radix trie routing, method dispatch, nested routers, route params | `Router`, `MethodRouter` |
| `arvik-hyper` | Hyper 1.x server loop, TCP accept, graceful shutdown | `Server`, `serve()` |
| `arvik-extract` | All extractors: `Path`, `Query`, `Json`, `Form`, `State`, etc. | `FromRequest`, `FromRequestParts` |
| `arvik-middleware` | Tower-compatible built-in middleware | `CorsLayer`, `CompressionLayer`, etc. |
| `arvik-ws` | WebSocket handshake + message passing | `WebSocket`, `ws()` |
| `arvik-sse` | Server-Sent Events streaming | `Sse`, `Event` |
| `arvik-static` | Serve files and directories | `ServeDir`, `ServeFile` |
| `arvik-tls` | TLS acceptor (rustls / native-tls) | `TlsAcceptor` |
| `arvik-macros` | Proc macros for ergonomics | `#[handler]`, `#[route]` |
| `arvik-test` | In-process test client | `TestClient` |

---

## 3. Core Abstractions

### 3.1 Request

```rust
// arvik-core/src/request.rs
pub struct Request<B = Body> {
    inner: http::Request<B>,
    extensions: Extensions,
}

impl<B> Request<B> {
    // Access method, URI, version, headers, body, extensions
    pub fn method(&self) -> &Method;
    pub fn uri(&self) -> &Uri;
    pub fn headers(&self) -> &HeaderMap;
    pub fn extensions(&self) -> &Extensions;
    pub fn extensions_mut(&mut self) -> &mut Extensions;
    pub fn body(&self) -> &B;
    pub fn into_body(self) -> B;
    pub fn into_parts(self) -> (Parts, B);

    // Convenience: typed extension get
    pub fn extension<T: Clone + Send + Sync + 'static>(&self) -> Option<&T>;
}
```

### 3.2 Response

```rust
// arvik-core/src/response.rs
pub type Response<B = Body> = http::Response<B>;

// Builder pattern
pub struct ResponseBuilder {
    inner: http::response::Builder,
}

impl ResponseBuilder {
    pub fn status(self, status: StatusCode) -> Self;
    pub fn header<K, V>(self, key: K, value: V) -> Self;
    pub fn body<B: Into<Body>>(self, body: B) -> Result<Response, Error>;
    pub fn json<T: Serialize>(self, data: &T) -> Response;
    pub fn html(self, html: impl Into<String>) -> Response;
    pub fn text(self, text: impl Into<String>) -> Response;
    pub fn empty(self) -> Response;
    pub fn redirect(location: &str, status: StatusCode) -> Response;
}
```

### 3.3 Body

```rust
// arvik-core/src/body.rs
// Unified body type — wraps boxed bytes stream
pub struct Body(BoxBody);

impl Body {
    pub fn empty() -> Self;
    pub fn from_bytes(b: Bytes) -> Self;
    pub fn from_stream<S>(stream: S) -> Self
    where
        S: Stream<Item = Result<Bytes, BoxError>> + Send + 'static;

    pub async fn collect(self) -> Result<Collected<Bytes>, Error>;
    pub async fn to_bytes(self) -> Result<Bytes, Error>;
    pub async fn to_string(self) -> Result<String, Error>;
}

// LimitedBody — limits request body size (configurable per-route)
pub struct LimitedBody {
    inner: Body,
    remaining: usize,
}
```

---

## 4. Routing System

### 4.1 Router

```rust
// arvik-router/src/lib.rs

pub struct Router<S = ()> {
    inner: Arc<RouterInner<S>>,
}

impl<S: Clone + Send + Sync + 'static> Router<S> {
    pub fn new() -> Self;

    // Basic method routes
    pub fn route(self, path: &str, method_router: MethodRouter<S>) -> Self;

    // Nest sub-routers (with path prefix)
    pub fn nest(self, path: &str, router: Router<S>) -> Self;

    // Nest raw Tower services
    pub fn nest_service<T>(self, path: &str, service: T) -> Self
    where T: Service<Request> + Clone + Send + 'static;

    // Merge two routers (union of their routes)
    pub fn merge(self, other: Router<S>) -> Self;

    // Attach shared application state
    pub fn with_state(self, state: S) -> Router;

    // Apply a Tower layer to all routes
    pub fn layer<L>(self, layer: L) -> Self
    where L: Layer<Route> + Clone + Send + 'static;

    // Apply layer to routes matching a predicate
    pub fn route_layer<L>(self, layer: L) -> Self;

    // Fallback handler when no route matches
    pub fn fallback<H, T>(self, handler: H) -> Self
    where H: Handler<T, S>;

    // Fallback to a Tower service
    pub fn fallback_service<T>(self, service: T) -> Self;
}
```

### 4.2 Method Router

```rust
// Every HTTP method has a shorthand constructor
pub fn get<H, T, S>(handler: H) -> MethodRouter<S>
pub fn post<H, T, S>(handler: H) -> MethodRouter<S>
pub fn put<H, T, S>(handler: H) -> MethodRouter<S>
pub fn delete<H, T, S>(handler: H) -> MethodRouter<S>
pub fn patch<H, T, S>(handler: H) -> MethodRouter<S>
pub fn head<H, T, S>(handler: H) -> MethodRouter<S>
pub fn options<H, T, S>(handler: H) -> MethodRouter<S>
pub fn trace<H, T, S>(handler: H) -> MethodRouter<S>
pub fn any<H, T, S>(handler: H) -> MethodRouter<S>

// Chaining multiple methods on one path
pub fn on<H, T, S>(filter: MethodFilter, handler: H) -> MethodRouter<S>

impl MethodRouter<S> {
    pub fn get(self, handler: H) -> Self;
    pub fn post(self, handler: H) -> Self;
    // ... all methods
    pub fn layer<L>(self, layer: L) -> Self;        // per-route layer
    pub fn route_layer<L>(self, layer: L) -> Self;  // run before method dispatch
    pub fn fallback(self, handler: H) -> Self;      // method not allowed fallback
    pub fn with_state(self, state: S) -> MethodRouter;
}
```

### 4.3 Route Patterns

```
/                           → root
/users                      → static
/users/{id}                  → path parameter
/users/{id}/posts/{post_id}   → multiple params
/files/{*path}                → wildcard (captures rest of path)
/api/v{version}/users       → inline param
```

### 4.4 Radix Trie Internals

```
                    [/]
                   /    \
               [users]  [files]
               /    \       \
           [/{id}]  [/me]   [/{*path}]
            /
        [/posts]
            \
         [/{post_id}]
```

- Each node stores: `prefix: &'static str`, `handler: Option<MethodRouter>`, `children: SmallVec<[Node; 4]>`, `param_child: Option<Box<Node>>`, `wildcard_child: Option<Box<Node>>`
- Hot path: zero heap allocation for routing (params stored in stack-allocated `SmallVec`)
- Conflict detection at startup (`/users/{id}` vs `/users/me` → panic with clear message)

---

## 5. Handler System

### 5.1 Handler Trait

```rust
// arvik-core/src/handler.rs

pub trait Handler<T, S>: Clone + Send + Sized + 'static {
    type Future: Future<Output = Response> + Send + 'static;
    fn call(self, req: Request, state: S) -> Self::Future;
}

// Blanket impl for async functions with extractors
// Supports up to 16 extractors (generated via macro)
impl<F, Fut, S, T1, T2, ...> Handler<(T1, T2, ...), S> for F
where
    F: FnOnce(T1, T2, ...) -> Fut + Clone + Send + 'static,
    Fut: Future<Output = impl IntoResponse> + Send,
    T1: FromRequestParts<S>,
    T2: FromRequestParts<S>,
    ...
```

### 5.2 Concurrency Model

```rust
// Handlers run in Tokio tasks — each request gets its own task
// State is Arc-wrapped automatically for Send + Sync sharing
// No global mutable state — per-request extensions via TypeMap
```

### 5.3 Handler Combinators

```rust
// Map handler output
handler.map(|res| res.with_header("x-foo", "bar"))

// Chain handlers (like middleware, but without Tower)
handler.before(pre_handler)
handler.after(post_handler)

// Boxed handler (type-erased, for dynamic dispatch)
handler.boxed()
```

---

## 6. Extractor System

### 6.1 Extractor Traits

```rust
// Parts-only extractors (no body consumption)
pub trait FromRequestParts<S>: Sized {
    type Rejection: IntoResponse;
    async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, Self::Rejection>;
}

// Full request extractors (consumes body — only ONE per handler)
pub trait FromRequest<S, M = ViaRequest>: Sized {
    type Rejection: IntoResponse;
    async fn from_request(req: Request, state: &S) -> Result<Self, Self::Rejection>;
}
```

### 6.2 All Built-in Extractors

| Extractor | Trait | Description |
|---|---|---|
| `Path<T>` | `FromRequestParts` | URL path parameters, deserialized via serde |
| `Query<T>` | `FromRequestParts` | Query string parameters |
| `Headers` | `FromRequestParts` | Typed header access (via `headers` crate) |
| `TypedHeader<T>` | `FromRequestParts` | Single typed header |
| `Method` | `FromRequestParts` | HTTP method |
| `Uri` | `FromRequestParts` | Full request URI |
| `Version` | `FromRequestParts` | HTTP version |
| `OriginalUri` | `FromRequestParts` | URI before any path rewrites |
| `Extension<T>` | `FromRequestParts` | Typed request extension |
| `State<S>` | `FromRequestParts` | Shared app state |
| `ConnectInfo<T>` | `FromRequestParts` | Client connection info (IP, port) |
| `MatchedPath` | `FromRequestParts` | The matched route pattern |
| `RawPathParams` | `FromRequestParts` | Raw (untyped) path parameters |
| `Json<T>` | `FromRequest` | JSON body (requires `Content-Type: application/json`) |
| `Form<T>` | `FromRequest` | URL-encoded form body |
| `Multipart` | `FromRequest` | Multipart form data |
| `Bytes` | `FromRequest` | Raw body as `Bytes` |
| `String` | `FromRequest` | Raw body as `String` |
| `Body` | `FromRequest` | Raw streaming body |
| `Request` | `FromRequest` | Entire request (escape hatch) |

### 6.3 Custom Extractors

```rust
// Implement FromRequestParts for your own types
pub struct CurrentUser(User);

impl<S> FromRequestParts<S> for CurrentUser
where
    S: AsRef<UserStore> + Send + Sync,
{
    type Rejection = AuthError;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &S,
    ) -> Result<Self, Self::Rejection> {
        let token = TypedHeader::<Authorization<Bearer>>::from_request_parts(parts, state)
            .await
            .map_err(|_| AuthError::MissingToken)?;

        let user = state.as_ref().verify(token.token()).await?;
        Ok(CurrentUser(user))
    }
}
```

### 6.4 Optional Extractors

```rust
// Wrap any extractor in Option<T> — never rejects, returns None on failure
async fn handler(user: Option<CurrentUser>) { ... }

// Or Result<T, E> — gives you the rejection to inspect
async fn handler(json: Result<Json<Payload>, JsonRejection>) { ... }
```

---

## 7. Response System

### 7.1 IntoResponse Trait

```rust
pub trait IntoResponse {
    fn into_response(self) -> Response;
}

// Blanket impls:
// - StatusCode
// - (StatusCode, impl IntoResponse)
// - (StatusCode, HeaderMap, impl IntoResponse)
// - (Parts, impl IntoResponse)
// - String, &str, Bytes
// - Json<T: Serialize>
// - Html<T: Into<String>>
// - Result<T: IntoResponse, E: IntoResponse>
// - (impl IntoResponseParts, impl IntoResponse)  → appends headers
```

### 7.2 Built-in Response Types

```rust
// JSON response
Json(value)                         // 200 + Content-Type: application/json

// HTML response
Html("<h1>Hello</h1>")             // 200 + Content-Type: text/html

// Plain text
"Hello"                             // 200 + Content-Type: text/plain

// Empty body
StatusCode::NO_CONTENT              // 204

// Redirect
Redirect::to("/new-path")           // 303
Redirect::permanent("/new-path")    // 301
Redirect::temporary("/new-path")    // 307

// Stream response
StreamBody::new(stream)

// File download
(
    [("content-disposition", "attachment; filename=\"file.csv\"")],
    body,
)

// Custom status + headers + body
(StatusCode::CREATED, Json(created_resource))

(
    StatusCode::OK,
    [(header::CONTENT_TYPE, "application/xml")],
    xml_string,
)
```

### 7.3 IntoResponseParts (Append Headers)

```rust
// Append headers/cookies without losing body type
pub trait IntoResponseParts {
    type Error: Into<Response>;
    fn into_response_parts(self, res: ResponseParts) -> Result<ResponseParts, Self::Error>;
}

// Example: cookie jar as response part
impl IntoResponseParts for CookieJar {
    // Sets Set-Cookie headers
}
```

---

## 8. Middleware & Layers

Arvik uses Tower's `Service` + `Layer` model. Every middleware is a `Layer`.

### 8.1 Applying Middleware

```rust
// Global — applies to all routes
Router::new()
    .route("/", get(handler))
    .layer(CorsLayer::permissive())
    .layer(CompressionLayer::new())
    .layer(TimeoutLayer::new(Duration::from_secs(30)));

// Per-route
Router::new()
    .route(
        "/admin",
        get(admin_handler).layer(RequireAuthLayer::new()),
    );

// Ordered: outermost layer runs first on request, last on response
// .layer(A).layer(B).layer(C) → C(B(A(handler)))
```

### 8.2 Built-in Middleware

#### CORS (`arvik-middleware::cors`)
```rust
CorsLayer::new()
    .allow_origin(["https://example.com".parse()?])
    .allow_methods([Method::GET, Method::POST])
    .allow_headers([AUTHORIZATION, CONTENT_TYPE])
    .allow_credentials(true)
    .max_age(Duration::from_secs(3600))
    .expose_headers([X_REQUEST_ID])

// Presets
CorsLayer::permissive()     // allow everything (dev)
CorsLayer::very_permissive() // allow everything including credentials
```

#### Compression (`arvik-middleware::compression`)
```rust
CompressionLayer::new()
    .gzip(true)
    .br(true)
    .zstd(true)
    .deflate(true)
    .quality(CompressionLevel::Default)

DecompressionLayer::new()   // decompress request bodies
```

#### Timeout
```rust
TimeoutLayer::new(Duration::from_secs(30))
// Returns 408 Request Timeout automatically
```

#### Rate Limiting
```rust
RateLimitLayer::new(100, Duration::from_secs(1))   // 100 req/sec per IP
// Token bucket or sliding window algorithm (configurable)
// Keyed by: IP, API key, user ID (custom key extractor)
```

#### Request ID
```rust
RequestIdLayer::new()
// Generates UUID v4 per request, inserts as x-request-id header
// Propagates to response

PropagateRequestIdLayer::new()
// Copies incoming x-request-id to response
```

#### Tracing / Logging
```rust
TraceLayer::new_for_http()
    .make_span_with(DefaultMakeSpan::new().level(Level::INFO))
    .on_request(DefaultOnRequest::new())
    .on_response(DefaultOnResponse::new().latency_unit(LatencyUnit::Millis))
    .on_failure(DefaultOnFailure::new().level(Level::ERROR))
    .on_body_chunk(())
    .on_eos(())
```

#### Authentication
```rust
// JWT validation middleware
RequireAuthorizationLayer::bearer("my-secret")
RequireAuthorizationLayer::basic("user", "password")

// Custom auth
RequireAuthorizationLayer::custom(|req: &Request| async {
    // return Ok(()) or Err(response)
})
```

#### Security Headers
```rust
SensitiveHeadersLayer::new([AUTHORIZATION, COOKIE])
// Marks headers as sensitive so they're redacted in logs/traces

SetResponseHeaderLayer::new(
    header::X_FRAME_OPTIONS,
    HeaderValue::from_static("DENY"),
)
SetResponseHeaderLayer::overriding(...)    // always overwrite
SetResponseHeaderLayer::appending(...)     // append if exists

// Full security header suite
SecurityHeadersLayer::new()
// Sets: X-Frame-Options, X-Content-Type-Options,
//       X-XSS-Protection, Strict-Transport-Security,
//       Content-Security-Policy, Referrer-Policy
```

#### Catch Panic
```rust
CatchPanicLayer::new()
// Catches panics in handlers, returns 500 instead of crashing
CatchPanicLayer::custom(|panic_info| { ... })
```

#### Body Limit
```rust
RequestBodyLimitLayer::new(10 * 1024 * 1024)  // 10MB limit
// Returns 413 Payload Too Large automatically
```

#### Map Request/Response
```rust
MapRequestLayer::new(|req| { ... })
MapResponseLayer::new(|res| { ... })
MapRequestBodyLayer::new(|body| { ... })
MapResponseBodyLayer::new(|body| { ... })
```

---

## 9. State Management

### 9.1 App State

```rust
// Define your state
#[derive(Clone)]
struct AppState {
    db: PgPool,
    redis: RedisPool,
    config: Arc<Config>,
}

// Attach to router
let state = AppState { ... };
let app = Router::new()
    .route("/users", get(list_users))
    .with_state(state);

// Extract in handler
async fn list_users(State(state): State<AppState>) -> impl IntoResponse {
    // use state.db
}
```

### 9.2 Multiple State Types (FromRef)

```rust
// Access sub-parts of state without cloning everything
impl FromRef<AppState> for PgPool {
    fn from_ref(state: &AppState) -> PgPool {
        state.db.clone()
    }
}

// Now extractors can take the sub-type directly
async fn handler(State(db): State<PgPool>) { ... }
```

### 9.3 Request-Scoped Extensions

```rust
// Insert in middleware
req.extensions_mut().insert(CurrentUser { id: 42 });

// Extract in handler
Extension(user): Extension<CurrentUser>

// Or via custom extractor:
async fn handler(user: CurrentUser) { ... }
```

---

## 10. Error Handling

### 10.1 AjayaError

```rust
// arvik-core/src/error.rs

#[derive(Debug)]
pub struct Error {
    inner: Box<dyn std::error::Error + Send + Sync>,
    status: StatusCode,
    public_message: Option<String>,
}

impl Error {
    pub fn new(err: impl Into<BoxError>) -> Self;
    pub fn with_status(self, status: StatusCode) -> Self;
    pub fn with_message(self, msg: impl Into<String>) -> Self;
    pub fn status(&self) -> StatusCode;
}

impl IntoResponse for Error {
    fn into_response(self) -> Response {
        // Returns JSON: { "error": "...", "code": 500 }
    }
}
```

### 10.2 Result Return Types

```rust
// Return Result from handlers — E: IntoResponse
async fn handler() -> Result<Json<User>, AppError> {
    let user = db.find_user(id).await?;  // ? works if Error: Into<AppError>
    Ok(Json(user))
}

// Define your app error type
#[derive(Debug)]
pub enum AppError {
    NotFound,
    Unauthorized,
    Database(sqlx::Error),
    Internal(anyhow::Error),
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        match self {
            AppError::NotFound => (StatusCode::NOT_FOUND, "Not found").into_response(),
            AppError::Unauthorized => (StatusCode::UNAUTHORIZED, "Unauthorized").into_response(),
            AppError::Database(e) => {
                tracing::error!("DB error: {e}");
                StatusCode::INTERNAL_SERVER_ERROR.into_response()
            }
            AppError::Internal(e) => {
                tracing::error!("Internal: {e}");
                StatusCode::INTERNAL_SERVER_ERROR.into_response()
            }
        }
    }
}
```

### 10.3 Rejection Handling

```rust
// Customize how extractor rejections are handled
Router::new()
    .route("/", post(handler))
    .layer(HandleErrorLayer::new(|err: BoxError| async move {
        if err.is::<tower::timeout::error::Elapsed>() {
            return (StatusCode::REQUEST_TIMEOUT, "Request timed out");
        }
        (StatusCode::INTERNAL_SERVER_ERROR, "Unknown error")
    }));
```

---

## 11. WebSockets

`arvik-ws` provides full WebSocket support built on `tokio-tungstenite`.

**Auto ping/pong**: `WebSocket::recv()` automatically replies to `Ping` frames
with a matching `Pong` — no boilerplate needed in your handler.

```rust
// arvik-ws

// Handler upgrade
async fn ws_handler(ws: WebSocketUpgrade, State(state): State<AppState>) -> Response {
    ws.on_upgrade(|socket| handle_socket(socket, state))
}

// Simple echo — Ping/Pong handled automatically, no extra match arm needed
async fn handle_socket(mut socket: WebSocket, state: AppState) {
    while let Some(Ok(msg)) = socket.recv().await {
        match msg {
            Message::Text(text) => {
                socket.send(Message::Text(format!("Echo: {text}"))).await.ok();
            }
            Message::Binary(data) => {
                socket.send(Message::Binary(data)).await.ok();
            }
            Message::Close(_) => break,
            _ => {}
        }
    }
}

// Concurrent send + receive via split()
async fn handle_split(socket: WebSocket) {
    let (mut sender, mut receiver) = socket.split();

    let send_task = tokio::spawn(async move {
        sender.send(Message::Text("hello".into())).await.ok();
    });

    // In split mode: manually pong Ping frames via Sender
    while let Some(Ok(msg)) = receiver.next().await {
        if let Message::Ping(data) = msg {
            // forward pong via the Sender in your send task
            let _ = data; // handle as needed
        }
    }
    send_task.await.ok();
}

// WebSocketUpgrade options
ws.max_message_size(64 * 1024)      // 64KB max message
  .max_frame_size(16 * 1024)        // 16KB max frame
  .accept_unmasked_frames(false)    // RFC compliance
  .protocols(["chat", "json"])      // subprotocol negotiation
  .on_upgrade(handle_socket)
```

---

## 12. Server-Sent Events (SSE)

```rust
// arvik-sse

async fn sse_handler() -> Sse<impl Stream<Item = Result<Event, Infallible>>> {
    let stream = tokio_stream::wrappers::IntervalStream::new(
        tokio::time::interval(Duration::from_secs(1))
    )
    .map(|_| {
        Ok(Event::default()
            .data(format!("time: {}", chrono::Utc::now()))
            .id("1")
            .event("tick")
            .retry(Duration::from_secs(5)))
    });

    Sse::new(stream)
        .keep_alive(
            KeepAlive::new()
                .interval(Duration::from_secs(15))
                .text("keep-alive"),
        )
}
```

---

## 13. Multipart & File Uploads

```rust
// arvik-extract::multipart

async fn upload_handler(mut multipart: Multipart) -> Result<impl IntoResponse, AppError> {
    while let Some(field) = multipart.next_field().await? {
        if field.file_name().is_some() {
            // Stream directly to a secure temporary file. The file is deleted
            // on drop unless TempFile::persist is called.
            let temp = field.save_to_temp().await?;
            tracing::info!(
                file = ?temp.metadata().file_name(),
                bytes = temp.bytes_written(),
                "upload saved"
            );
        } else {
            // Or process field bytes with cumulative progress.
            let mut stream = field.into_progress_stream();
            while let Some(chunk) = stream.next().await {
                let chunk = chunk?;
                tracing::debug!(bytes_read = chunk.bytes_read());
            }
        }
    }
    Ok(StatusCode::OK)
}

// Config via request extensions, usually inserted by middleware.
req.extensions_mut().insert(
    MultipartConfig::new()
        .max_fields(20)
        .max_field_size(5 * 1024 * 1024)    // 5MB per field
        .max_total_size(50 * 1024 * 1024)   // 50MB total
        .with_temp_dir("/var/tmp/arvik-uploads"),
);

// Custom extractors/tests can construct directly:
Multipart::from_request_with_constraints(req, MultipartConstraints::new()).await?;
```

---

## 14. Static File Serving

```rust
// arvik-static
use arvik::{Embed, EmbeddedFileService, Router, ServeDir, ServeFile};
use std::path::PathBuf;

let assets = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("assets");
let favicon = assets.join("favicon.ico");
let not_found = assets.join("404.html");

// Serve a directory
Router::new()
    .nest_service("/static", ServeDir::new(assets)
        .not_found_service(ServeFile::new(not_found))
        .precompressed_gzip()
        .precompressed_br()
        .directory_listing(true)
        .cache_control("public, max-age=60")
        .call_fallback_on_method_not_allowed(true))

// Serve a single file
Router::new()
    .route_service("/favicon.ico", ServeFile::new(favicon))

// Embed files at compile time (no runtime FS access)
// via rust-embed integration
#[derive(Embed)]
#[folder = "assets"]
#[crate_path = "arvik"]
struct Assets;

Router::new()
    .nest_service("/assets", EmbeddedFileService::<Assets>::new()
        .precompressed_gzip()
        .precompressed_br()
        .cache_control("public, max-age=31536000, immutable"))
```

`ServeDir`, `ServeFile`, and `EmbeddedFileService` implement Tower `Service<Request>`.
They support MIME detection, `Last-Modified`, `ETag`, conditional `304`, single byte
ranges, optional directory listings, custom 404 fallback services, and `.br`/`.gz`
precompressed asset negotiation.

---

## 15. TLS / HTTPS

```rust
// arvik-tls — rustls backend
use arvik_tls::rustls::RustlsConfig;

// From PEM files
let config = RustlsConfig::from_pem_file("cert.pem", "key.pem").await?;

// From in-memory PEM
let config = RustlsConfig::from_pem(cert_pem, key_pem).await?;

// Self-signed (dev only)
let config = RustlsConfig::self_signed(["localhost", "127.0.0.1"]).await?;

// Serve
arvik::serve_tls(app, "0.0.0.0:443", config).await?;
arvik::serve_tls_with_config(app, "0.0.0.0:443", config, ServerConfig::new()).await?;

// Hot reload TLS certs without restart
let config = RustlsConfig::from_pem_file("cert.pem", "key.pem").await?;
config.reload_from_pem_file("cert.pem", "key.pem").await?;
let _watcher = config.watch_pem_files(
    "cert.pem",
    "key.pem",
    Duration::from_secs(1),
)?;

// native-tls backend (OpenSSL / SChannel / Secure Transport)
use arvik_tls::native::{NativeTlsConfig};
let config = NativeTlsConfig::from_pkcs12("identity.p12", "password")?;
arvik::serve_native_tls(app, "0.0.0.0:443", config).await?;
```

Rustls is the primary guaranteed HTTP/2 ALPN backend. Native TLS enables ALPN
best-effort where the platform backend supports it and falls back cleanly to
HTTP/1.1 otherwise.

---

## 16. HTTP/2 & HTTP/3

```rust
// HTTP/1.1 + HTTP/2 (via hyper — automatic ALPN negotiation with TLS)
let config = RustlsConfig::from_pem_file("cert.pem", "key.pem").await?;
arvik::serve_tls(app, addr, config).await?;  // auto-negotiates h1/h2

// HTTP/2 over cleartext (h2c) — for internal services / proxies
arvik::serve_h2c(app, addr).await?;
arvik::serve_h2c_with_config(app, addr, ServerConfig::new()).await?;

// HTTP/3 (QUIC via quinn — feature flag)
#[cfg(feature = "http3")]
arvik::serve_h3(app, addr, quic_config).await?;

// Server configuration
ServerConfig::new()
    .http1_keepalive(true)
    .http1_pipeline_flush(true)         // pipeline responses
    .http2_keep_alive_interval(Duration::from_secs(20))
    .http2_keep_alive_timeout(Duration::from_secs(10))
    .http2_initial_stream_window_size(1024 * 1024)
    .http2_initial_connection_window_size(4 * 1024 * 1024)
    .http2_max_concurrent_streams(1000)
    .http2_max_header_list_size(64 * 1024)
```

`ServerConfig` is only for low-level runtime/server tuning. File/env config
loading, including `arvik.toml`, belongs to the future 0.7.4 configuration
system.

HTTP/2 push promises are not implemented in Arvik Hyper mode because browser
support is deprecated and Hyper's service API does not expose a stable push
surface. Use `Preload` / `PreloadLink` to emit `Link: rel=preload` headers.

---

## 17. gRPC Support

```rust
// arvik integrates with tonic for gRPC via Tower compatibility
// Serve gRPC and REST on the same port

use tonic::transport::Server as TonicServer;

let grpc_service = TonicServer::builder()
    .add_service(UserServiceServer::new(UserServiceImpl))
    .into_service();

let rest_router = Router::new()
    .route("/health", get(health_handler));

let app = Router::new()
    .route_service("/users.UserService/*rpc", grpc_service)  // gRPC routes
    .merge(rest_router);  // REST routes

arvik::serve(app, addr).await?;
```

---

## 18. Testing Utilities

```rust
// arvik-test

// Build in-process test client (no network, no port)
let app = Router::new().route("/", get(|| async { "Hello" }));
let client = TestClient::new(app);

// GET request
let res = client.get("/").send().await;
assert_eq!(res.status(), 200);
assert_eq!(res.text().await, "Hello");

// POST JSON
let res = client.post("/users")
    .json(&serde_json::json!({ "name": "Alice" }))
    .send()
    .await;
assert_eq!(res.status(), 201);
let body: User = res.json().await;

// Headers
let res = client.get("/protected")
    .header("Authorization", "Bearer token123")
    .send()
    .await;

// WebSocket test
let mut ws = client.ws("/ws").await;
ws.send(Message::Text("hello".into())).await;
let reply = ws.recv().await.unwrap();

// Multipart
let res = client.post("/upload")
    .multipart(
        Form::new()
            .part("file", Part::bytes(file_bytes).file_name("test.txt"))
    )
    .send()
    .await;
```

---

## 19. Proc Macros

Proc macros are opt-in through `arvik = { features = ["macros"] }`.

### 19.1 `#[debug_handler]`

```rust
// Gives MUCH better error messages when handler type-checks fail
// (same as axum's #[debug_handler])
#[arvik::debug_handler(state = AppState)]
async fn handler(State(state): State<AppState>, Json(body): Json<Payload>) -> impl IntoResponse {
    // Without debug_handler, rustc error points to .route(), with it — points here
}
```

### 19.2 `#[route]`

```rust
// Attach routing metadata directly to functions
#[arvik::route(GET, "/users/{id}")]
async fn get_user(Path(id): Path<Uuid>) -> impl IntoResponse {
    // ...
}

#[arvik::post("/users")]
async fn create_user(Json(body): Json<CreateUser>) -> impl IntoResponse {
    // ...
}

// Collect and register all annotated routes
let app = Router::new().routes(arvik::collect_routes![get_user, create_user]);
```

`collect_routes!` detects duplicate path/method pairs within the same collection.
The route macros use router-native `{id}` and `{*path}` segments.

### 19.3 `#[handler]`

```rust
// Impl Handler trait for structs (useful for handlers that need fields)
#[derive(Clone)]
struct RateLimitedHandler {
    limiter: Arc<RateLimiter>,
}

#[arvik::handler]
impl RateLimitedHandler {
    async fn call(&self, req: Request) -> impl IntoResponse {
        if self.limiter.check().is_err() {
            return StatusCode::TOO_MANY_REQUESTS.into_response();
        }
        format!("accepted {}", req.uri().path())
    }
}
```

`#[handler]` is intentionally applied to the inherent `impl` block. A Rust
attribute on the struct cannot inspect a later `impl` block.

---

## 20. Configuration System

```rust
// arvik::config
// Future 0.7.4 file/env application config.
// This is separate from arvik_hyper::ServerConfig, which is only
// low-level connection/protocol tuning.

#[derive(Debug, Deserialize)]
pub struct AppConfig {
    pub host: String,
    pub port: u16,
    pub workers: Option<usize>,           // defaults to num CPUs
    pub backlog: u32,                     // TCP backlog (default: 1024)
    pub max_connections: Option<usize>,
    pub body_limit: usize,                // bytes (default: 10MB)
    pub tls: Option<TlsConfig>,
    pub shutdown_timeout: Duration,
}

// Load from file, env, or code
let config = AjayaConfig::builder()
    .file("arvik.toml")
    .env_prefix("AJAYA")
    .build()?;

// arvik.toml example
// [server]
// host = "0.0.0.0"
// port = 8080
// workers = 4
// body_limit = 10485760
```

---

## 21. Observability

### 21.1 Metrics

```rust
// Prometheus integration (feature = "metrics")
use arvik::metrics::PrometheusMetricsLayer;

Router::new()
    .route("/", get(handler))
    .layer(PrometheusMetricsLayer::new())
    .route("/metrics", get(arvik::metrics::metrics_handler))

// Automatically tracks:
// - arvik_requests_total (counter, by method + path + status)
// - arvik_request_duration_seconds (histogram)
// - arvik_requests_in_flight (gauge)
// - arvik_response_body_size_bytes (histogram)
// - arvik_request_body_size_bytes (histogram)
```

### 21.2 Distributed Tracing

```rust
// OpenTelemetry integration (feature = "opentelemetry")
use arvik::trace::OtelLayer;

Router::new()
    .layer(OtelLayer::new("my-service"))

// Propagates: W3C TraceContext, B3, Jaeger headers
// Exports to: OTLP (gRPC/HTTP), Jaeger, Zipkin, stdout
```

### 21.3 Health Checks

```rust
// Built-in health endpoint
Router::new()
    .route("/health", get(arvik::health::health_handler))
    .route("/health/live", get(arvik::health::liveness_handler))
    .route("/health/ready", get(arvik::health::readiness_handler))

// Custom readiness checks
arvik::health::add_check("database", || async {
    db.ping().await.is_ok()
});
```

---

## 22. Security Features

### 22.1 CSRF Protection

```rust
CsrfLayer::new("secret-key-32-bytes-long!!!!")
// Generates + validates CSRF tokens on state-changing routes
```

### 22.2 Cookie Security

```rust
use arvik::cookies::{CookieJar, Cookie, Key, SignedCookieJar, PrivateCookieJar};

// Signed cookies (tamper-proof)
async fn handler(jar: SignedCookieJar) -> (SignedCookieJar, Response) {
    let jar = jar.add(Cookie::new("session_id", "abc123"));
    (jar, StatusCode::OK.into_response())
}

// Encrypted cookies (tamper-proof + confidential)
async fn handler(jar: PrivateCookieJar) -> impl IntoResponse {
    let jar = jar.add(Cookie::new("user_id", "42"));
    (jar, StatusCode::OK.into_response())
}
```

### 22.3 Request Validation

```rust
// Via validator crate integration
#[derive(Deserialize, Validate)]
struct CreateUser {
    #[validate(length(min = 2, max = 50))]
    name: String,
    #[validate(email)]
    email: String,
    #[validate(range(min = 18, max = 120))]
    age: u8,
}

// ValidatedJson extractor (validates after parsing)
async fn create_user(ValidatedJson(body): ValidatedJson<CreateUser>) -> impl IntoResponse {
    // body is guaranteed valid
}
```

---

## 23. Connection & Server Tuning

```rust
// arvik-hyper/src/server.rs

Server::bind("0.0.0.0:8080")
    // Socket options
    .tcp_nodelay(true)                    // disable Nagle — lower latency
    .tcp_keepalive(Duration::from_secs(60))
    .tcp_keepalive_interval(Duration::from_secs(5))
    .tcp_keepalive_retries(3)
    .reuse_port(true)                     // SO_REUSEPORT — multi-core accept
    .reuse_address(true)
    .backlog(4096)

    // HTTP options
    .http1_half_close(true)
    .http1_title_case_headers(false)
    .http2_only(false)
    .http2_adaptive_window(true)          // adaptive flow control

    // Worker threads
    .worker_threads(num_cpus::get())
    .max_blocking_threads(512)

    // Connection limits
    .max_connections(10_000)
    .connection_timeout(Duration::from_secs(5))

    // Graceful shutdown
    .serve_with_graceful_shutdown(
        app,
        async { tokio::signal::ctrl_c().await.ok(); }
    )
    .await?;
```

---

## 24. Data Formats

```rust
// Built-in: JSON (serde_json), HTML (string), plain text, binary

// Feature flags for additional formats:

// MessagePack
MsgPack(value)        // serialize
MsgPack::<T>::from_request(req)  // deserialize

// CBOR
Cbor(value)

// XML
Xml(value)

// YAML
Yaml(value)

// CSV (streaming)
CsvBody::new(iter_of_records)

// Custom: implement IntoResponse + FromRequest for any format
```

---

## 25. Database Integration

Arvik is DB-agnostic. These are first-party example integrations:

```toml
# arvik/Cargo.toml feature flags
[features]
sqlx = ["dep:sqlx"]
diesel = ["dep:diesel"]
sea-orm = ["dep:sea-orm"]
mongodb = ["dep:mongodb"]
redis = ["dep:redis"]
```

```rust
// Typical pattern: DB pool in app state
#[derive(Clone)]
struct AppState {
    db: PgPool,  // sqlx
}

// Connection via extractor (DB connection from pool per request)
async fn handler(State(db): State<PgPool>) -> impl IntoResponse {
    let users = sqlx::query_as::<_, User>("SELECT * FROM users")
        .fetch_all(&db)
        .await?;
    Json(users)
}
```

---

## 26. Performance Architecture

### 26.1 Zero-Cost Hot Path

```
Request arrives (TCP)
  ↓
SO_REUSEPORT → per-CPU accept loop (no lock contention)
  ↓
Hyper 1.x connection handler (async, zero-copy)
  ↓
Radix trie router (O(log n), zero alloc, SmallVec params)
  ↓
Handler dispatch (monomorphized, no dyn dispatch on hot path)
  ↓
Extractor deserialization (direct from Bytes, no intermediate String)
  ↓
Response serialization (direct to Bytes, zero-copy)
  ↓
Write to TCP socket
```

### 26.2 Memory Strategy

| Allocation | Strategy |
|---|---|
| Route params | `SmallVec<[(&str, &str); 8]>` — stack allocated for ≤ 8 params |
| Request body | `bytes::Bytes` — ref-counted, zero-copy slicing |
| Response body | `bytes::BytesMut` — growable, no-copy finalize |
| Per-request extensions | `AHashMap` (faster than `HashMap` for small maps) |
| String interning | Route patterns interned at startup |

### 26.3 Tokio Tuning

```rust
// Main runtime
tokio::runtime::Builder::new_multi_thread()
    .worker_threads(num_cpus::get())
    .max_io_events_per_tick(1024)
    .event_interval(61)
    .global_queue_interval(61)
    .build()?
    .block_on(serve(app))
```

### 26.4 Benchmark Targets (TechEmpower Round 22 equivalent)

| Test | Target | Actix-web |
|---|---|---|
| Plaintext | 800K req/sec | 600K req/sec |
| JSON | 500K req/sec | 380K req/sec |
| Single query (Postgres) | 200K req/sec | 150K req/sec |
| Multiple queries | 30K req/sec | 22K req/sec |
| Fortunes (template) | 150K req/sec | 120K req/sec |

---

## 27. Full Dependency Graph

```toml
# Root workspace Cargo.toml
[workspace]
members = [
    "arvik",
    "arvik-core",
    "arvik-router",
    "arvik-hyper",
    "arvik-extract",
    "arvik-middleware",
    "arvik-ws",
    "arvik-sse",
    "arvik-static",
    "arvik-tls",
    "arvik-macros",
    "arvik-test",
]
resolver = "2"

[workspace.dependencies]
# Async runtime
tokio          = { version = "1", features = ["full"] }
tokio-stream   = "0.1"

# HTTP
hyper          = { version = "1", features = ["server", "http1", "http2"] }
hyper-util     = { version = "0.1", features = ["tokio", "server-auto"] }
http           = "1"
http-body      = "1"
http-body-util = "0.1"

# Tower
tower          = { version = "0.5", features = ["full"] }
tower-http     = { version = "0.6", features = ["full"] }
tower-layer    = "0.3"
tower-service  = "0.3"

# Bytes
bytes          = "1"

# Serde
serde          = { version = "1", features = ["derive"] }
serde_json     = "1"
serde_urlencoded = "0.7"

# WebSocket
tokio-tungstenite = "0.24"

# TLS
rustls         = "0.23"
tokio-rustls   = "0.26"
rustls-pemfile = "2"

# Multipart
multer         = "3"

# Compression
async-compression = { version = "0.4", features = ["gzip", "br", "zstd", "deflate"] }

# Routing
matchit        = "0.8"   # Radix trie (or implement own)

# Utilities
pin-project-lite = "0.2"
futures-util   = "0.3"
ahash          = "0.8"
smallvec       = { version = "1", features = ["union"] }
once_cell      = "1"
parking_lot    = "0.12"
thiserror      = "2"
tracing        = "0.1"
tracing-subscriber = { version = "0.3", features = ["env-filter", "json"] }
uuid           = { version = "1", features = ["v4"] }
mime           = "0.3"
percent-encoding = "2"
form_urlencoded = "1"
itoa           = "1"
tempfile       = "3"

# Proc macro (arvik-macros only)
syn            = { version = "2", features = ["full"] }
quote          = "1"
proc-macro2    = "1"
```

---

## 28. Feature Flags

```toml
# arvik/Cargo.toml
[features]
default = ["http1", "http2", "json", "form", "query", "multipart", "ws", "sse"]

# HTTP versions
http1      = []
http2      = ["hyper/http2"]
http3      = ["dep:quinn", "dep:h3", "dep:h3-quinn"]

# Data formats
json       = ["dep:serde_json"]
msgpack    = ["dep:rmp-serde"]
cbor       = ["dep:ciborium"]
xml        = ["dep:quick-xml"]
yaml       = ["dep:serde_yaml"]

# Body parsers
form       = ["dep:serde_urlencoded"]
query      = ["dep:serde_urlencoded"]
multipart  = ["dep:multer"]

# Protocols
ws         = ["dep:tokio-tungstenite"]
sse        = []
grpc       = ["dep:tonic"]

# TLS
tls        = ["rustls", "tokio-rustls"]
native-tls = ["dep:native-tls", "dep:tokio-native-tls"]

# Compression
compression   = ["dep:async-compression", "dep:tower-http/compression-full"]
decompression = ["dep:async-compression", "dep:tower-http/decompression-full"]

# Security
cookies    = ["dep:cookie"]
csrf       = ["cookies"]

# Observability
metrics    = ["dep:prometheus", "dep:metrics"]
opentelemetry = ["dep:opentelemetry", "dep:tracing-opentelemetry"]

# Static files
static-files = ["dep:arvik-static", "arvik-static/fs"]
embedded-static = ["static-files", "arvik-static/embed"]

# Testing
test-utils = []

# Proc macros
macros     = ["dep:arvik-macros"]

# Validation
validator  = ["dep:validator"]

# Database pools (convenience re-exports)
sqlx       = ["dep:sqlx"]
```

---

## 29. Comparison: Arvik vs Axum vs Actix

| Feature | **Arvik** | Axum | Actix-web |
|---|---|---|---|
| HTTP/1.1 | ✅ | ✅ | ✅ |
| HTTP/2 | ✅ | ✅ | ✅ |
| HTTP/3 | ✅ (feature flag) | ❌ | ❌ |
| Router | Radix trie | matchit | Regex-based |
| Zero-alloc routing | ✅ | ✅ | ❌ |
| Tower compatibility | ✅ | ✅ | Partial |
| Typed extractors | ✅ | ✅ | ✅ |
| Macro-free handlers | ✅ | ✅ | ❌ |
| WebSockets | ✅ | ✅ | ✅ |
| SSE | ✅ | ✅ | ✅ |
| Multipart | ✅ | ✅ | ✅ |
| Static files | ✅ | ✅ | ✅ |
| gRPC (Tonic) | ✅ | ✅ | ❌ |
| rustls + native-tls | ✅ | ✅ | ✅ |
| Cookie (signed + private) | ✅ | ✅ | ✅ |
| CSRF protection | ✅ | Via tower-sessions | ✅ |
| Built-in rate limiting | ✅ | Via tower | Via actix-limitation |
| Request validation | ✅ (validator) | Via axum-valid | Via actix-validate |
| Prometheus metrics | ✅ | Via axum-prometheus | ✅ |
| OpenTelemetry | ✅ | Via tracing | Via actix-web-opentelemetry |
| `#[debug_handler]` | ✅ | ✅ | ❌ |
| `#[route]` macro | ✅ | ❌ (planned) | ✅ |
| SO_REUSEPORT | ✅ | ❌ (manual) | ✅ |
| In-process test client | ✅ | ✅ | ✅ |
| Connection draining | ✅ | ✅ | ✅ |
| Actor model | ❌ | ❌ | ✅ (Actix) |
| MessagePack / CBOR | ✅ (feature) | Via crates | Via crates |
| Body size limit | ✅ (built-in) | ✅ | ✅ |
| Catch panic middleware | ✅ | ✅ | ✅ |

---

*Arvik — Fast, Typed, and Fearless.*
*Built by Aarambh Dev Hub.*
