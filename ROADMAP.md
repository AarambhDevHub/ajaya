# Ajaya (अजय) — Public Roadmap

> *From first TCP byte to unconquerable framework.*
> Every version is a shipping, usable increment. No big-bang releases.

---

## Version Philosophy

```
0.0.x  →  Foundation & Core Primitives
0.1.x  →  Routing & Handlers
0.2.x  →  Extractors
0.3.x  →  Responses & Error Handling
0.4.x  →  Middleware
0.5.x  →  Protocols (WS, SSE, Multipart)
0.6.x  →  TLS, HTTP/2, Static Files
0.7.x  →  Macros, Testing, Config
0.8.x  →  Observability & Security
0.9.x  →  Performance Sprint & Benchmarks
0.10.x →  Stabilization & Docs
```

---

## 0.0.x — Foundation

### `0.0.1` — Workspace Bootstrap
**Goal:** Repo exists, compiles, does nothing useful yet.

- [x] Initialize Cargo workspace with all 12 crates
- [x] All `Cargo.toml` files with correct dependencies
- [x] `ajaya-core`: empty `lib.rs` stubs for `Request`, `Response`, `Body`, `Error`
- [x] `ajaya-hyper`: raw Tokio + Hyper 1.x TCP listener (hardcoded "Hello World" response)
- [x] CI: GitHub Actions — `cargo check`, `cargo clippy`, `cargo test`
- [x] `README.md` skeleton

**Deliverable:** `cargo run` → server starts on port 8080, returns "Hello from Ajaya" to every request.

---

### `0.0.2` — Core Types
**Goal:** Real `Request` and `Response` types replacing raw Hyper types.

- [x] `ajaya-core`: `Request<B>` wrapper around `http::Request<B>`
- [x] `ajaya-core`: `Response<B>` type alias + `ResponseBuilder`
- [x] `ajaya-core`: `Body` unified type (wraps `BoxBody`)
- [x] `Body::empty()`, `Body::from_bytes()`, `Body::to_bytes()` async
- [x] `Extensions` typed map on `Request`
- [x] Convert raw Hyper request/response ↔ Ajaya types in `ajaya-hyper`

**Deliverable:** Handlers receive `Request`, return `Response`. Fully typed.

---

### `0.0.3` — Handler Trait
**Goal:** First version of the `Handler` trait.

- [x] `ajaya-core`: `Handler<T, S>` trait definition
- [x] Blanket impl for `async fn() -> impl IntoResponse` (zero extractors)
- [x] Blanket impl for `async fn(Request) -> impl IntoResponse` (raw request)
- [x] `IntoResponse` trait + impls for `StatusCode`, `String`, `&str`, `Bytes`, `(StatusCode, String)`
- [x] Wire handler into `ajaya-hyper` serve loop

**Deliverable:** Write a bare async fn, pass it to the server, it works.

---

### `0.0.4` — Method Dispatch Skeleton
**Goal:** Differentiate GET vs POST at the server level.

- [x] `ajaya-core`: `MethodFilter` bitflag enum
- [x] `ajaya-router`: `MethodRouter` struct (stores handlers per HTTP method)
- [x] `get()`, `post()`, `put()`, `delete()`, `patch()` constructor functions
- [x] Return `405 Method Not Allowed` when method doesn't match
- [x] Return `404 Not Found` for unknown paths (hardcoded fallback)

**Deliverable:** `get(handler)` and `post(handler)` work as distinct routes.

---

### `0.0.5` — Error Foundation
**Goal:** Proper error type + `?` propagation in handlers.

- [x] `ajaya-core`: `Error` struct with status + message
- [x] `AjayaError` implements `std::error::Error` + `IntoResponse`
- [x] `Result<T: IntoResponse, E: IntoResponse>` implements `IntoResponse`
- [x] Handlers can return `Result<impl IntoResponse, impl IntoResponse>`
- [x] Internal server errors don't leak details in response body

**Deliverable:** `async fn handler() -> Result<Json<T>, AppError>` compiles and works.

---

## 0.1.x — Routing System

### `0.1.0` — Static Router
**Goal:** Route requests to different handlers based on path.

- [x] `ajaya-router`: `Router<S>` struct
- [x] `.route(path, method_router)` — registers a route
- [x] Static path matching: `/`, `/users`, `/users/list`
- [x] Internal `HashMap<&str, MethodRouter>` (not trie yet — keep it simple first)
- [x] `Router` implements Tower `Service`
- [x] Wire `Router` into `ajaya-hyper` serve loop

**Deliverable:** Multiple routes work. `/users` → one handler, `/posts` → another.

---

### `0.1.1` — Radix Trie Router
**Goal:** Replace HashMap router with a real radix trie.

- [x] `ajaya-router/src/trie.rs`: `TrieNode` with prefix, children, handler
- [x] `ajaya-router/src/node.rs`: node insert + lookup logic
- [x] `ajaya-router/src/params.rs`: `PathParams` — `SmallVec<[(&str, &str); 8]>`
- [x] Route conflict detection at startup (panic with clear message)
- [x] Benchmark: route lookup must be zero heap allocation

**Deliverable:** 1000 routes registered — lookup still O(log n), zero alloc per request.

---

### `0.1.2` — Path Parameters
**Goal:** `{param}` segments in routes.

- [x] Parse `{name}` segments during route registration
- [x] Extract param values during lookup
- [x] Store in `PathParams` on request extensions
- [x] Support multiple params: `/users/{id}/posts/{post_id}`
- [x] URL decode param values

**Deliverable:** `/users/42` matches `/users/{id}` and extracts `id = "42"`.

---

### `0.1.3` — Wildcard Routes
**Goal:** `{*path}` catch-all segments.

- [x] Parse `*name` wildcard during registration
- [x] Wildcard captures remainder of path including slashes
- [x] Wildcard has lowest priority (static > param > wildcard)
- [x] `/files/{*path}` matches `/files/a/b/c.txt` → `path = "a/b/c.txt"`

**Deliverable:** Wildcard routes work, priority order correct.

---

### `0.1.4` — Nested Routers
**Goal:** Compose routers with path prefixes.

- [x] `Router::nest(prefix, sub_router)` — mounts sub-router under prefix
- [x] Path params in prefix work: `.nest("/users/:id", user_router)`
- [x] `OriginalUri` extension preserved after nesting
- [x] Nested routers inherit parent layers

**Deliverable:** API versioning via `.nest("/api/v1", v1_router)`.

---

### `0.1.5` — Router Merge & Fallback
**Goal:** Combine multiple routers, handle 404s.

- [x] `Router::merge(other)` — union of routes (panic on conflict)
- [x] `Router::fallback(handler)` — custom 404 handler
- [x] `Router::fallback_service(service)` — fallback to Tower service
- [x] Default fallback: `404 Not Found` plain text response

**Deliverable:** Split router definitions across files, merge at startup.

---

### `0.1.6` — Tower Service Nesting
**Goal:** Mount any Tower service inside the router.

- [x] `Router::nest_service(path, service)` — mounts raw service via wildcard
- [x] `Router::route_service(path, service)` — like route but for services
- [x] `ServiceHandler<T>` adapter wrapping Tower Service → Handler

**Deliverable:** Mount a separate Tonic gRPC service on a sub-path.

---

## 0.2.x — Extractor System

### `0.2.0` — Extractor Traits
**Goal:** `FromRequestParts` and `FromRequest` trait definitions.

- [x] `ajaya-core`: `FromRequestParts<S>` trait
- [x] `ajaya-core`: `FromRequest<S, M>` trait
- [x] `rejection.rs`: `Rejection` type + all built-in rejection variants
- [x] `Rejection` implements `IntoResponse` with appropriate status codes
- [x] Handler blanket impl updated to support up to 16 extractors (macro-generated)
- [x] `Option<T>` wrapper — never rejects, returns `None` on failure
- [x] `Result<T, E>` wrapper — returns rejection as `Err`

**Deliverable:** Extractor trait system in place. Ready to implement all extractors.

---

### `0.2.1` — Path & Query Extractors
**Goal:** Type-safe path params and query strings.

- [x] `Path<T: DeserializeOwned>` — deserializes path params via serde
- [x] `Query<T: DeserializeOwned>` — deserializes query string via serde
- [x] Clear rejection messages: "missing field `id`", "invalid type: expected u32"
- [x] `RawPathParams` — untyped `(String, String)` pairs

**Deliverable:** `Path<Uuid>`, `Query<SearchParams>` work in handlers.

---

### `0.2.2` — Header Extractors
**Goal:** Access request headers in handlers.

- [x] `TypedHeader<T>` — uses `headers` crate for typed header parsing
- [x] Common typed headers: `Authorization`, `ContentType`, `Accept`, `UserAgent`, `Cookie`, `Host`, `Origin`, `Referer`, `ContentLength`
- [x] `HeaderMap` extractor — raw access to all headers

**Deliverable:** `TypedHeader<Authorization<Bearer>>` works in handlers.

---

### `0.2.3` — Request Metadata Extractors
**Goal:** Access method, URI, version, connection info.

- [x] `Method` extractor
- [x] `Uri` extractor
- [x] `Version` extractor (HTTP/1.0, HTTP/1.1, HTTP/2)
- [x] `OriginalUri` extractor (before path rewrites)
- [x] `MatchedPath` extractor (the route pattern that matched)
- [x] `ConnectInfo<T>` extractor — client socket address (requires `serve` config)
- [x] `Extension<T>` extractor — typed request extension

**Deliverable:** Can access all request metadata without taking `Request` directly.

---

### `0.2.4` — JSON & Form Extractors
**Goal:** Parse request bodies.

- [x] `Json<T: DeserializeOwned>` — parses JSON body, validates Content-Type
- [x] `Form<T: DeserializeOwned>` — parses `application/x-www-form-urlencoded`
- [x] `Bytes` extractor — raw body bytes
- [x] `String` extractor — raw body as UTF-8 string
- [x] `Body` extractor — raw streaming body (escape hatch)
- [x] `Request` extractor — full request (ultimate escape hatch)
- [x] Body is consumed once — enforce single body extractor per handler

**Deliverable:** `Json<CreateUser>` and `Form<LoginForm>` work as handler params.

---

### `0.2.5` — State Extractor
**Goal:** Access shared application state from handlers.

- [x] `State<S>` extractor — clones `S` from router state
- [x] `FromRef<S>` trait — extract sub-types from app state
- [ ] `with_state(s)` on `Router` and `MethodRouter`
- [x] State must be `Clone + Send + Sync + 'static`
- [ ] Error if state not set: clear compile-time message via `#[debug_handler]`

**Deliverable:** `State(db): State<PgPool>` works in handlers.

---

### `0.2.6` — Multipart Extractor
**Goal:** Handle file uploads.

- [x] `Multipart` extractor — wraps `multer` crate
- [x] `multipart.next_field()` async iteration
- [x] Field: `.name()`, `.file_name()`, `.content_type()`, `.bytes()`, `.chunk()` stream
- [x] `MultipartConstraints` — max fields, max field size, max total size
- [ ] Returns 413 if limits exceeded

**Deliverable:** File upload endpoint works with streaming field reading.

---

## 0.3.x — Response System

### `0.3.0` — Response Builder & Helpers
**Goal:** Ergonomic response construction.

- [ ] `ResponseBuilder` with `.status()`, `.header()`, `.body()`, `.json()`, `.html()`, `.text()`
- [ ] `Json<T: Serialize>` response type
- [ ] `Html<T: Into<String>>` response type
- [ ] `Redirect::to()`, `Redirect::permanent()`, `Redirect::temporary()`
- [ ] `StatusCode` alone as response (empty body)
- [ ] Tuple impls: `(StatusCode, impl IntoResponse)`, `(HeaderMap, impl IntoResponse)`, `(StatusCode, HeaderMap, impl IntoResponse)`

**Deliverable:** All common response patterns work without boilerplate.

---

### `0.3.1` — Streaming Responses
**Goal:** Stream response bodies.

- [ ] `StreamBody<S>` — wraps `Stream<Item = Result<Bytes, E>>` as response body
- [ ] `Body::from_stream()` constructor
- [ ] Proper `Transfer-Encoding: chunked` for HTTP/1.1 streamed responses
- [ ] Backpressure: don't buffer full response in memory

**Deliverable:** Stream large files / generated data without memory bloat.

---

### `0.3.2` — IntoResponseParts
**Goal:** Append headers/cookies without losing body type.

- [ ] `IntoResponseParts` trait
- [ ] `ResponseParts` builder accumulates extra headers
- [ ] `(impl IntoResponseParts, impl IntoResponse)` tuple impl
- [ ] Multiple parts: `(part1, part2, impl IntoResponse)`
- [ ] `AppendHeaders<I>` — append multiple headers at once

**Deliverable:** Return cookies + JSON body as a single tuple without fighting types.

---

### `0.3.3` — Cookie Support
**Goal:** Read and write cookies.

- [ ] `CookieJar` extractor — reads cookies from `Cookie` header
- [ ] `CookieJar` as `IntoResponseParts` — sets `Set-Cookie` headers
- [ ] `SignedCookieJar` — HMAC-signed cookies (tamper-proof)
- [ ] `PrivateCookieJar` — encrypted cookies (tamper-proof + confidential)
- [ ] `Key` type for signing/encryption
- [ ] Cookie builder: `.path()`, `.domain()`, `.max_age()`, `.secure()`, `.http_only()`, `.same_site()`

**Deliverable:** Sessions via encrypted cookies, no external session store needed.

---

### `0.3.4` — Error Handling Polish
**Goal:** Complete error handling system.

- [ ] `HandleErrorLayer` — convert `BoxError` (from Tower layers) into responses
- [ ] `ajaya::error::ErrorResponse` — standard JSON error body `{ error, code, request_id }`
- [ ] Map rejection types to custom error responses
- [ ] `IntoResponse` for `anyhow::Error` (behind feature flag)

**Deliverable:** Timeout errors, body limit errors all return proper JSON responses.

---

## 0.4.x — Middleware

### `0.4.0` — Tower Integration
**Goal:** First-class Tower `Layer` + `Service` support.

- [ ] `Router::layer()` applies to all routes
- [ ] `Router::route_layer()` applies to matched routes only
- [ ] `MethodRouter::layer()` per-route layers
- [ ] Layer ordering documentation (outermost first)
- [ ] `ServiceBuilder` usage pattern documented in examples

**Deliverable:** Any Tower middleware works with Ajaya routers.

---

### `0.4.1` — CORS Middleware
**Goal:** Full CORS spec implementation.

- [ ] `CorsLayer` with builder API
- [ ] Allow origin: exact, list, any, predicate
- [ ] Allow methods, allow headers, expose headers
- [ ] Allow credentials, max age
- [ ] Handle preflight `OPTIONS` requests automatically
- [ ] `CorsLayer::permissive()` preset for development
- [ ] `CorsLayer::very_permissive()` preset

**Deliverable:** Single-origin, multi-origin, and wildcard CORS all work correctly.

---

### `0.4.2` — Compression & Decompression
**Goal:** Transparent body compression.

- [ ] `CompressionLayer` — compress response based on `Accept-Encoding`
- [ ] Supports: gzip, brotli, zstd, deflate
- [ ] `CompressionLevel` — default, fastest, best
- [ ] `DecompressionLayer` — decompress request bodies
- [ ] Skip compression for small bodies (< 1KB)
- [ ] Skip compression for already-compressed content types

**Deliverable:** gzip/br compression on all responses automatically.

---

### `0.4.3` — Timeout Middleware
**Goal:** Request timeout enforcement.

- [ ] `TimeoutLayer::new(Duration)` — wraps Tower's timeout
- [ ] Returns `408 Request Timeout` automatically
- [ ] Per-route timeout via `MethodRouter::layer(TimeoutLayer::new(...))`
- [ ] Graceful: in-flight responses complete, new requests rejected during shutdown

**Deliverable:** Every route has a configurable timeout.

---

### `0.4.4` — Request ID Middleware
**Goal:** Unique ID per request for tracing.

- [ ] `RequestIdLayer` — generates UUID v4, inserts as `x-request-id` header
- [ ] `PropagateRequestIdLayer` — forwards incoming `x-request-id` to response
- [ ] Configurable header name
- [ ] Custom ID generator (pluggable)
- [ ] Available as `Extension<RequestId>` in handlers

**Deliverable:** Every request has a unique traceable ID.

---

### `0.4.5` — Tracing Middleware
**Goal:** Structured request/response logging via `tracing`.

- [ ] `TraceLayer::new_for_http()`
- [ ] Span per request: method, path, status, latency
- [ ] Configurable `make_span_with`, `on_request`, `on_response`, `on_failure`
- [ ] `DefaultMakeSpan`, `DefaultOnRequest`, `DefaultOnResponse`, `DefaultOnFailure`
- [ ] Log level: INFO for success, WARN for 4xx, ERROR for 5xx
- [ ] Latency units: millis, micros, seconds

**Deliverable:** Every request logs method + path + status + latency as structured tracing span.

---

### `0.4.6` — Security Header Middleware
**Goal:** HTTP security headers out of the box.

- [ ] `SensitiveHeadersLayer` — redacts headers in traces/logs
- [ ] `SetResponseHeaderLayer` — set/override/append response headers
- [ ] `SetRequestHeaderLayer` — set/override/append request headers
- [ ] `SecurityHeadersLayer` — full suite: `X-Frame-Options`, `X-Content-Type-Options`, `Strict-Transport-Security`, `Content-Security-Policy`, `Referrer-Policy`

**Deliverable:** One layer call adds all OWASP-recommended security headers.

---

### `0.4.7` — Rate Limiting Middleware
**Goal:** Protect endpoints from abuse.

- [ ] `RateLimitLayer::new(count, duration)` — token bucket per IP
- [ ] Sliding window algorithm option
- [ ] Key extractor: IP, API key, user ID (custom closure)
- [ ] Returns `429 Too Many Requests` with `Retry-After` header
- [ ] In-memory store (default) + Redis backend (feature flag)

**Deliverable:** 100 req/sec per IP limit on any route with one `.layer()` call.

---

### `0.4.8` — Auth Middleware
**Goal:** Authentication enforcement layer.

- [ ] `RequireAuthorizationLayer::bearer(token)` — static bearer token
- [ ] `RequireAuthorizationLayer::basic(user, pass)` — HTTP Basic
- [ ] `RequireAuthorizationLayer::custom(async_fn)` — custom async validator
- [ ] Returns `401 Unauthorized` with `WWW-Authenticate` header
- [ ] JWT validation (feature = "jwt"): `JwtLayer::new(secret)`

**Deliverable:** Protect admin routes with one middleware line.

---

### `0.4.9` — Body & Panic Middleware
**Goal:** Safety middleware.

- [ ] `RequestBodyLimitLayer::new(bytes)` — limit request body size
- [ ] Returns `413 Payload Too Large`
- [ ] `CatchPanicLayer::new()` — catch handler panics, return 500
- [ ] `CatchPanicLayer::custom(fn)` — custom panic response
- [ ] Panic info available in custom handler

**Deliverable:** Server never crashes from a panicking handler or oversized upload.

---

### `0.4.10` — Map Middleware
**Goal:** Lightweight request/response transformation.

- [ ] `MapRequestLayer::new(fn)` — transform request before handler
- [ ] `MapResponseLayer::new(fn)` — transform response after handler
- [ ] `MapRequestBodyLayer::new(fn)` — transform request body
- [ ] `MapResponseBodyLayer::new(fn)` — transform response body
- [ ] All async-capable

**Deliverable:** Add/remove headers, rewrite paths, modify bodies without a full service.

---

### `0.4.11` — CSRF Middleware
**Goal:** Protect state-changing routes from CSRF attacks.

- [ ] `CsrfLayer::new(secret)` — generates + validates CSRF tokens
- [ ] Double-submit cookie pattern
- [ ] Skip safe methods (GET, HEAD, OPTIONS)
- [ ] Token available as `Extension<CsrfToken>` in handlers
- [ ] Customizable token header name

**Deliverable:** POST/PUT/DELETE routes require valid CSRF token.

---

## 0.5.x — Protocols

### `0.5.0` — WebSocket Support
**Goal:** Full WebSocket upgrade and messaging.

- [ ] `ajaya-ws`: `WebSocketUpgrade` extractor
- [ ] `.on_upgrade(async fn(WebSocket))` callback
- [ ] `WebSocket`: `.send(Message)`, `.recv() -> Option<Result<Message>>`, `.close()`
- [ ] `Message` variants: `Text`, `Binary`, `Ping`, `Pong`, `Close`
- [ ] Split socket: `socket.split() -> (Sender, Receiver)` for concurrent send/recv
- [ ] Config: `max_message_size`, `max_frame_size`, `protocols`
- [ ] Auto-respond to Ping with Pong

**Deliverable:** WebSocket echo server, chat server examples working.

---

### `0.5.1` — Server-Sent Events
**Goal:** One-directional event streaming to clients.

- [ ] `ajaya-sse`: `Sse<S>` response type
- [ ] `Event` builder: `.data()`, `.id()`, `.event()`, `.retry()`
- [ ] `KeepAlive` — sends comment lines to prevent connection timeout
- [ ] Works with any `Stream<Item = Result<Event, E>>`
- [ ] Proper `Content-Type: text/event-stream` + `Cache-Control: no-cache`

**Deliverable:** Live feed / notification stream to browser EventSource.

---

### `0.5.2` — Multipart Polish
**Goal:** Production-ready multipart handling.

- [ ] Streaming multipart (no full-body buffering)
- [ ] Save field to temp file automatically
- [ ] Progress tracking via stream
- [ ] Reject non-multipart requests with clear error
- [ ] Integration test with actual browser form upload

**Deliverable:** Upload a 100MB file without 100MB RAM usage.

---

## 0.6.x — TLS, HTTP/2, Static Files

### `0.6.0` — rustls TLS
**Goal:** HTTPS with rustls (no OpenSSL dependency).

- [ ] `ajaya-tls`: `RustlsConfig` from PEM files
- [ ] `RustlsConfig::from_pem()` — in-memory PEM
- [ ] `RustlsConfig::self_signed()` — dev mode self-signed cert
- [ ] `ajaya::serve_tls(app, addr, config)` entry point
- [ ] ALPN negotiation: prefer HTTP/2, fall back to HTTP/1.1

**Deliverable:** `cargo run` → HTTPS server on port 443.

---

### `0.6.1` — TLS Hot Reload
**Goal:** Rotate TLS certs without downtime.

- [ ] `RustlsConfig::reload_from_pem_file()` — swap cert/key at runtime
- [ ] Watch file changes (via `notify` crate, feature flag)
- [ ] Existing connections unaffected; new connections use new cert
- [ ] Log cert expiry warnings

**Deliverable:** Let's Encrypt cert renewal works without server restart.

---

### `0.6.2` — native-tls Backend
**Goal:** OpenSSL / SChannel / Secure Transport support.

- [ ] `ajaya-tls`: `NativeTlsConfig` from PKCS12
- [ ] `NativeTlsConfig::from_pkcs12(data, password)`
- [ ] `ajaya::serve_native_tls(app, addr, config)` entry point
- [ ] Feature flag: `native-tls` (disabled by default)

**Deliverable:** TLS on Windows/macOS without bundling rustls.

---

### `0.6.3` — HTTP/2 Tuning
**Goal:** Optimal HTTP/2 performance settings.

- [ ] `ServerConfig` HTTP/2 options: window sizes, concurrent streams, keep-alive
- [ ] `ajaya::serve_h2c(app, addr)` — HTTP/2 over cleartext
- [ ] Push promises (server push) API
- [ ] HTTP/2 trailers support

**Deliverable:** h2c internal service-to-service communication works.

---

### `0.6.4` — Static File Serving
**Goal:** Serve files and directories efficiently.

- [ ] `ajaya-static`: `ServeDir::new(path)` — serve directory tree
- [ ] `ServeFile::new(path)` — serve single file
- [ ] MIME type detection from extension
- [ ] `Last-Modified` + `ETag` headers
- [ ] `If-Modified-Since` / `If-None-Match` conditional GET → 304
- [ ] Range requests (`Range: bytes=0-1023`) → 206 Partial Content
- [ ] `.not_found_service()` — custom 404 handler
- [ ] `.precompressed_gzip()` — serve `.gz` pre-compressed file if exists
- [ ] `.precompressed_br()` — serve `.br` pre-compressed file if exists
- [ ] Directory listing (opt-in)

**Deliverable:** Full static file CDN-like behavior.

---

### `0.6.5` — Embedded Static Files
**Goal:** Bundle assets into binary at compile time.

- [ ] `rust-embed` integration
- [ ] `EmbeddedFileService<A: RustEmbed>` — Tower service from embedded assets
- [ ] Proper Content-Type, ETag (hash of content), cache headers
- [ ] Works same as `ServeDir` but from binary

**Deliverable:** Single-binary deployment with bundled frontend assets.

---

## 0.7.x — Macros, Testing, Config

### `0.7.0` — `#[debug_handler]` Macro
**Goal:** Dramatically better compile errors.

- [ ] `ajaya-macros`: `#[debug_handler]` proc macro
- [ ] Points error at the offending extractor, not at `.route()` call site
- [ ] Detects multiple body extractors
- [ ] Detects missing state
- [ ] Works with all extractor types

**Deliverable:** A wrong handler signature gives a clear error with line number.

---

### `0.7.1` — `#[route]` Macro
**Goal:** Attach routing metadata to functions.

- [ ] `#[route(GET, "/path")]` attribute macro
- [ ] `#[get("/path")]`, `#[post("/path")]` shorthand macros
- [ ] `ajaya::collect_routes![fn1, fn2, fn3]` — gather all annotated handlers
- [ ] `Router::routes(collected)` — register all at once
- [ ] Conflict detection at macro expansion time

**Deliverable:** Actix-style attribute routing works as an opt-in style.

---

### `0.7.2` — `#[handler]` Macro
**Goal:** Implement `Handler` for structs.

- [ ] `#[handler]` derives `Handler` for structs with `async fn call(&self, req: Request) -> Response`
- [ ] Useful for handlers with fields (rate limiter, cache, etc.)
- [ ] Works alongside `State<S>` pattern

**Deliverable:** Struct-based handlers work as first-class citizens.

---

### `0.7.3` — Test Client
**Goal:** In-process testing without spinning up a real server.

- [ ] `ajaya-test`: `TestClient::new(app)` — wraps router in memory
- [ ] `client.get(path)`, `.post(path)`, `.put(path)`, `.delete(path)`, `.patch(path)`
- [ ] Request builder: `.header()`, `.json()`, `.form()`, `.body()`, `.query()`
- [ ] Response: `.status()`, `.headers()`, `.text().await`, `.json::<T>().await`, `.bytes().await`
- [ ] WebSocket test: `client.ws(path).await → TestWebSocket`
- [ ] Cookie jar: automatically maintains cookies across requests

**Deliverable:** Handler tests run in microseconds, no port needed.

---

### `0.7.4` — Configuration System
**Goal:** Centralized server configuration.

- [ ] `ServerConfig` struct with all tuning options
- [ ] `AjayaConfig::builder()` with `.file()`, `.env_prefix()`, `.defaults()`, `.build()`
- [ ] Support: TOML, JSON, env vars
- [ ] Hot-reload config (debounced file watcher)
- [ ] Config schema validation with human-readable errors

**Deliverable:** `ajaya.toml` controls all server behavior, env vars override for production.

---

### `0.7.5` — Graceful Shutdown
**Goal:** Zero-downtime deploys and clean process exit.

- [ ] `serve_with_graceful_shutdown(app, signal_future)` — stop accepting new connections
- [ ] In-flight requests complete (with timeout)
- [ ] Configurable drain timeout
- [ ] Signal handling: `SIGTERM`, `SIGINT` (Unix), `CTRL_C` (Windows)
- [ ] Connection state: `on_connected`, `on_disconnected` hooks

**Deliverable:** `kill -TERM` → server drains and exits cleanly.

---

## 0.8.x — Observability & Security

### `0.8.0` — Prometheus Metrics
**Goal:** Production metrics out of the box.

- [ ] `PrometheusMetricsLayer` — instruments all requests
- [ ] Metrics: `ajaya_requests_total`, `ajaya_request_duration_seconds`, `ajaya_requests_in_flight`, `ajaya_response_body_size_bytes`, `ajaya_request_body_size_bytes`
- [ ] `GET /metrics` endpoint (Prometheus scrape)
- [ ] Custom labels: service name, version, environment
- [ ] Per-route metrics (label by matched path, not raw path — prevents cardinality explosion)

**Deliverable:** Grafana dashboard works from day one.

---

### `0.8.1` — OpenTelemetry Tracing
**Goal:** Distributed tracing integration.

- [ ] `OtelLayer::new(service_name)` — instruments all requests
- [ ] W3C TraceContext propagation (`traceparent`, `tracestate`)
- [ ] B3 propagation (Zipkin-compatible)
- [ ] Jaeger propagation
- [ ] OTLP exporter (gRPC + HTTP)
- [ ] Stdout/JSON exporter (dev mode)
- [ ] Span attributes: `http.method`, `http.url`, `http.status_code`, `http.user_agent`

**Deliverable:** Traces flow from gateway → service → service in Jaeger/Tempo.

---

### `0.8.2` — Health Check Endpoints
**Goal:** Standard health/liveness/readiness endpoints.

- [ ] `GET /health` → `{ status: "ok", uptime: 123 }`
- [ ] `GET /health/live` → 200 always (process alive)
- [ ] `GET /health/ready` → 200 only if all checks pass
- [ ] `ajaya::health::add_check(name, async_fn)` — register readiness checks
- [ ] Checks: DB ping, Redis ping, external API reachability
- [ ] `GET /health/startup` — one-time startup probe

**Deliverable:** Kubernetes liveness + readiness probes work out of the box.

---

### `0.8.3` — Request Validation
**Goal:** Declarative input validation.

- [ ] `ValidatedJson<T: Validate>` extractor — parse + validate JSON
- [ ] `ValidatedForm<T: Validate>` extractor — parse + validate form
- [ ] `ValidatedQuery<T: Validate>` extractor — parse + validate query
- [ ] `ValidationRejection` → `422 Unprocessable Entity` with field error details
- [ ] Uses `validator` crate: `#[validate(email)]`, `#[validate(length(min=2))]`, etc.
- [ ] Nested validation

**Deliverable:** Input never reaches handler logic if it fails validation.

---

### `0.8.4` — Structured Logging
**Goal:** Production-ready logging setup.

- [ ] `AjayaLogger::init()` — sets up `tracing_subscriber` with sensible defaults
- [ ] JSON log format (production), pretty format (dev), detect via env
- [ ] Log level from `RUST_LOG` env var
- [ ] Request ID propagated through all log lines in a request
- [ ] Sensitive header masking in logs

**Deliverable:** `docker logs myapp | jq` works perfectly in production.

---

## 0.9.x — Performance Sprint

### `0.9.0` — Connection Tuning
**Goal:** Maximize connections per second.

- [ ] `SO_REUSEPORT` — per-CPU accept loops (no mutex on accept)
- [ ] `TCP_NODELAY` — disable Nagle's algorithm
- [ ] `TCP_KEEPALIVE` — configurable keepalive probes
- [ ] Backlog tuning (`listen(fd, backlog)`)
- [ ] `SO_RCVBUF` + `SO_SNDBUF` tuning

**Deliverable:** Saturate all CPU cores on accept without lock contention.

---

### `0.9.1` — Zero-Copy Body Handling
**Goal:** Eliminate unnecessary memory copies.

- [ ] `bytes::Bytes` throughout body pipeline (ref-counted, cheap clone)
- [ ] `BytesMut` for response building (no realloc on finalize)
- [ ] `writev` scatter-gather I/O for multi-buffer responses
- [ ] Buffer pool for request bodies (avoid allocate-per-request)

**Deliverable:** Body reading and writing uses zero extra copies.

---

### `0.9.2` — Router Hot Path Audit
**Goal:** Confirm zero allocation on every request.

- [ ] Profile with `heaptrack` / `dhat` — confirm no alloc in router lookup
- [ ] `SmallVec<[(&str, &str); 8]>` for path params — stack for ≤8 params
- [ ] String interning for route patterns at startup
- [ ] Avoid `String::clone()` anywhere in hot path

**Deliverable:** `heaptrack` shows zero heap allocs for a route lookup.

---

### `0.9.3` — Benchmark Suite
**Goal:** TechEmpower-equivalent benchmarks in-repo.

- [ ] `examples/benchmarks/` — plaintext, JSON, DB single query, DB multiple queries, fortunes
- [ ] `wrk` + `hey` scripts
- [ ] CI job: run benchmarks on PR, comment results
- [ ] Comparison baseline: Axum 0.8, Actix-web 4
- [ ] Platform: 32-core, `SO_REUSEPORT` enabled

**Deliverable:** Numbers in README. Ajaya beats Actix on plaintext + JSON.

---

### `0.9.4` — HTTP/2 Performance
**Goal:** Maximize HTTP/2 throughput.

- [ ] Adaptive flow control window sizing
- [ ] HTTP/2 connection coalescing
- [ ] HPACK header compression tuning
- [ ] Concurrent stream multiplexing benchmark

**Deliverable:** HTTP/2 benchmark numbers in README.

---

### `0.9.5` — Async I/O Tuning
**Goal:** Squeeze last percentage points from Tokio.

- [ ] `io_uring` backend (feature = "io-uring", Linux only)
- [ ] Tokio runtime tuning: `event_interval`, `global_queue_interval`
- [ ] `tokio-metrics` integration — runtime health dashboard
- [ ] Identify and eliminate any blocking in async context

**Deliverable:** io_uring mode shows measurable improvement on Linux.

---

## 0.10.x — Stabilization

### `0.10.0` — API Freeze
**Goal:** No more breaking changes after this.

- [ ] Audit all public APIs — remove anything experimental
- [ ] `#[non_exhaustive]` on all public enums
- [ ] Deprecate any APIs that will be removed
- [ ] SemVer compatibility promise documented

---

### `0.10.1` — Documentation
**Goal:** Every public item documented.

- [ ] `//!` crate-level docs for all 12 crates
- [ ] `///` doc comments on every `pub` item
- [ ] `# Examples` section on every major type/function
- [ ] `docs.rs` rendering verified
- [ ] mdBook guide: Getting Started, Routing, Extractors, Middleware, Deployment

---

### `0.10.2` — Example Apps
**Goal:** Real-world reference apps.

- [ ] `examples/rest_api/` — CRUD REST API with SQLx + PostgreSQL
- [ ] `examples/websocket_chat/` — multi-room chat server
- [ ] `examples/file_upload/` — streaming upload to disk
- [ ] `examples/sse_notifications/` — live notification feed
- [ ] `examples/auth_jwt/` — JWT auth + refresh tokens
- [ ] `examples/grpc_rest/` — gRPC + REST on same port
- [ ] `examples/static_spa/` — serve React/Svelte SPA

---

### `0.10.3` — Migration Guide
**Goal:** Help Axum and Actix users migrate.

- [ ] `MIGRATING_FROM_AXUM.md` — side-by-side code comparison
- [ ] `MIGRATING_FROM_ACTIX.md` — side-by-side code comparison
- [ ] Common patterns: handler, extractor, middleware, state — all compared

---

### `0.10.4` — crates.io Publish
**Goal:** All crates published and versioned.

- [ ] All 12 crates published to crates.io
- [ ] `ajaya` facade crate as primary entry point
- [ ] GitHub release with changelog
- [ ] Announcement: YouTube video, Medium article, Reddit r/rust post

---

*Ajaya (अजय) — Unconquerable.*
*Built by Aarambh Dev Hub.*
