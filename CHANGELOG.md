# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

---

## [0.4.11] — 2026-MM-DD — CSRF Protection

### Added
- `ajaya_middleware::csrf::CsrfLayer` — double-submit cookie CSRF protection
- `CsrfToken` type — generated/verified per request, available as `Extension<CsrfToken>`
- Automatic CSRF cookie generation (`csrf_token` cookie)  
- State-changing method enforcement (POST, PUT, PATCH, DELETE require matching `x-csrf-token` header)
- Safe method passthrough (GET, HEAD, OPTIONS, TRACE are never checked)
- `.secure(bool)` and `.same_site(SameSite)` builder options

---

## [0.4.10] — 2026-MM-DD — Map Body Middleware

### Added
- `MapRequestBodyLayer` — transform request body bytes before the handler
- `MapResponseBodyLayer` — transform response body bytes after the handler
- Both support async closures: `|bytes: Bytes| async move { transform(bytes) }`

---

## [0.4.9] — 2026-MM-DD — Body Limit & Panic Recovery

### Added
- `RequestBodyLimitLayer::new(bytes)` — enforces max request body size
  - Checks `Content-Length` header immediately for early rejection
  - Enforces streaming limit during body collection
  - Returns `413 Payload Too Large` with JSON error body
- `CatchPanicLayer::new()` — catches handler panics via `tokio::task::spawn`
  - Returns `500 Internal Server Error` instead of crashing the task
  - `CatchPanicLayer::custom(fn)` — custom panic response closure
  - Logs panic message at `ERROR` level via tracing

---

## [0.4.8] — 2026-MM-DD — Auth Middleware

### Added
- `RequireAuthorizationLayer::bearer(token)` — validates static Bearer token
- `RequireAuthorizationLayer::basic(username, password)` — HTTP Basic auth
- `RequireAuthorizationLayer::custom(fn)` — synchronous custom validator
- Returns `401 Unauthorized` with proper `WWW-Authenticate` header
- JSON error response body for API compatibility

---

## [0.4.7] — 2026-MM-DD — Rate Limiting

### Added
- `RateLimitLayer::new(capacity, window)` — token bucket per key
- Key extraction strategies: `IpAddress` (default), `Header(name)`, `Global`
- `.by_header(name)` — rate limit by a request header value
- `.global()` — shared bucket for all requests
- Returns `429 Too Many Requests` with `Retry-After` and `X-RateLimit-*` headers
- Reads `X-Forwarded-For` / `X-Real-IP` headers for IP detection behind proxies
- Thread-safe via `parking_lot::Mutex`

---

## [0.4.6] — 2026-MM-DD — Security Headers

### Added
- `SecurityHeadersLayer` — injects the full OWASP recommended header suite:
  - `X-Frame-Options: DENY`
  - `X-Content-Type-Options: nosniff`
  - `X-XSS-Protection: 1; mode=block`
  - `Strict-Transport-Security: max-age=31536000; includeSubDomains`
  - `Referrer-Policy: strict-origin-when-cross-origin`
  - `Permissions-Policy: geolocation=(), microphone=(), camera=()`
  - `Content-Security-Policy` (configurable)
- `SetResponseHeaderLayer::if_not_present(name, value)` — conservative header setting
- `SetResponseHeaderLayer::overriding(name, value)` — always overwrite
- `SetResponseHeaderLayer::appending(name, value)` — append without removing existing
- `SetRequestHeaderLayer` — same modes, applied to requests
- `SensitiveHeadersLayer` — marks headers for log redaction via extensions
- All builder methods: `.content_security_policy()`, `.hsts_max_age()`, `.frame_options()`

---

## [0.4.5] — 2026-MM-DD — Tracing Middleware

### Added
- `TraceLayer::new_for_http()` — creates a tracing span per request
- Span fields: `http.method`, `http.path`, `http.version`, `http.status_code`, `latency`
- `DefaultMakeSpan` with configurable log level and header inclusion
- `LatencyUnit` enum: `Millis` (default), `Micros`, `Seconds`
- Automatic log levels: INFO (2xx), WARN (4xx), ERROR (5xx)
- `.make_span_with()`, `.latency_unit()`, `.log_failures()` builder methods

---

## [0.4.4] — 2026-MM-DD — Request ID Middleware

### Added
- `RequestIdLayer` — generates UUID v4 per request
  - Inserts `x-request-id` request and response header
  - Inserts `Extension<RequestId>` for handler access
  - Reuses incoming `x-request-id` if present (passthrough)
- `PropagateRequestIdLayer` — copies incoming `x-request-id` to response
- `RequestId` newtype wrapping `String`

---

## [0.4.3] — 2026-MM-DD — Timeout Middleware

### Added
- `TimeoutLayer::new(Duration)` — enforces request completion deadline
- Returns `408 Request Timeout` with JSON body on timeout
- Includes timeout duration in error message (`{N}ms time limit`)
- Per-route via `MethodRouter::layer(TimeoutLayer::new(...))`

---

## [0.4.2] — 2026-MM-DD — Compression & Decompression

### Added
- `CompressionLayer` — transparent response compression
  - Supported encodings: gzip, brotli, zstd, deflate
  - Reads `Accept-Encoding`, sets `Content-Encoding` and `Vary: Accept-Encoding`
  - Preference order: zstd > br > gzip > deflate
  - Skips already-encoded responses and non-compressible content types
  - Configurable minimum size (`min_size`, default 1024 bytes)
- `DecompressionLayer` — decompresses request bodies
  - Reads `Content-Encoding`, removes header after decompression
- `CompressionLevel` enum: `Default`, `Fastest`, `Best`
- Builder API: `.gzip()`, `.br()`, `.zstd()`, `.deflate()`, `.quality()`, `.min_size()`

---

## [0.4.1] — 2026-04-20 — CORS Middleware

### Added

- **`from_fn(f)`** — create Tower-compatible middleware from a plain async function.
  No `Service` or `Layer` trait implementations needed.

  ```rust
  // Before (v0.4.1): ~35 lines of Service + Layer boilerplate
  // After  (v0.4.2): 4 lines
  async fn log_requests(req: Request, next: Next) -> Response {
      let path = req.uri().path().to_string();
      let res  = next.run(req).await;
      tracing::info!("{} → {}", path, res.status());
      res
  }
  Router::new().layer(from_fn(log_requests));
  ```

- **`from_fn_with_state(state, f)`** — stateful middleware; the state is cloned
  once per request. Replaces the need for `Arc`-wrapped service structs.

  ```rust
  async fn require_api_key(state: AppState, req: Request, next: Next) -> impl IntoResponse {
      if req.headers().get("x-api-key").and_then(|v| v.to_str().ok())
          == Some(state.api_key.as_str())
      {
          next.run(req).await
      } else {
          StatusCode::UNAUTHORIZED.into_response()
      }
  }
  Router::new().layer(from_fn_with_state(my_state, require_api_key));
  ```

- **`Next`** — represents the remaining middleware + handler chain. Call
  `next.run(req)` to proceed. Short-circuit by returning early without calling it.

- **`map_request(f)`** — lightweight middleware that transforms only the request.
  More efficient than `from_fn` when no response mutation is needed.

  ```rust
  Router::new().layer(map_request(|mut req: Request| async move {
      req.headers_mut().insert("x-request-source", "ajaya".parse().unwrap());
      req
  }));
  ```

- **`map_response(f)`** — lightweight middleware that transforms only the response.

  ```rust
  Router::new().layer(map_response(|mut res: Response| async move {
      res.headers_mut().insert("x-powered-by", "ajaya".parse().unwrap());
      res
  }));
  ```

- **`ajaya::middleware` module** — all four helpers + `Next` are re-exported
  under `ajaya::middleware` in the facade crate. Import pattern:
  `use ajaya::middleware::{from_fn, from_fn_with_state, map_request, map_response, Next};`

- `ajaya-middleware/src/from_fn.rs` — new module implementing all four types
  and their Tower `Layer` + `Service` impls.

### Changed

- `ajaya/src/main.rs` — `RequestIdLayer` (35-line Tower boilerplate) replaced with
  a 4-line `from_fn(attach_request_id)` middleware. Also added `count_requests`
  (stateful, using `from_fn_with_state`) and `add_powered_by_header` (`map_response`)
  to demonstrate the full middleware DSL.

- `ajaya-middleware/src/lib.rs` — updated module table, added `from_fn` exports.

- `ajaya/src/lib.rs` — added `pub mod middleware` with all helper re-exports,
  added doc comment explaining the new middleware API.

- `ajaya-middleware/src/from_fn.rs` — **refactored** from a single 1103-line file
  into a modular structure for better maintainability:

  - `src/next.rs` — `Next` struct (handle to remaining middleware chain)
  - `src/middleware_fn.rs` — `MiddlewareFn` trait + blanket impls for 0–16 extractors
  - `src/from_fn.rs` — `from_fn`, `from_fn_with_state`, `FromFnLayer`, `FromFnService`
  - `src/map_request.rs` — `map_request`, `map_request_with_state` + Layer/Service types
  - `src/map_response.rs` — `map_response`, `map_response_with_state` + Layer/Service types

  All modules include `//!` module-level doc comments matching the original documentation style.

### Migration Guide

If you wrote a custom middleware using the old Tower boilerplate:

```rust
// OLD — v0.4.1
#[derive(Clone)]
struct MyLayer;
#[derive(Clone)]
struct MyService<S>(S);

impl<S> Layer<S> for MyLayer {
    type Service = MyService<S>;
    fn layer(&self, inner: S) -> Self::Service { MyService(inner) }
}

impl<S> Service<Request> for MyService<S>
where
    S: Service<Request, Response = Response, Error = Infallible> + Clone + Send + 'static,
    S::Future: Send + 'static,
{
    type Response = Response;
    type Error = Infallible;
    type Future = Pin<Box<dyn Future<Output = Result<Response, Infallible>> + Send + 'static>>;

    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.0.poll_ready(cx)
    }

    fn call(&mut self, req: Request) -> Self::Future {
        let cloned = self.0.clone();
        let mut inner = std::mem::replace(&mut self.0, cloned);
        Box::pin(async move {
            // your logic here
            inner.call(req).await
        })
    }
}
```

```rust
// NEW — v0.4.1
use ajaya::middleware::{from_fn, Next};

async fn my_middleware(req: Request, next: Next) -> Response {
    // your logic here
    next.run(req).await
}
Router::new().layer(from_fn(my_middleware));
```

### Added
- `ajaya_middleware::cors::CorsLayer` — full CORS spec implementation
- `CorsLayer::new()` — base constructor (no origins configured by default)
- `CorsLayer::permissive()` — allow all origins, methods, headers; no credentials
- `CorsLayer::very_permissive()` — same but with credentials (mirrors origin)
- Builder API: `.allow_origin()`, `.allow_methods()`, `.allow_headers()`, `.expose_headers()`, `.allow_credentials()`, `.max_age()`
- Automatic preflight `OPTIONS` request handling → `204 No Content`
- `Vary: Origin` header on all non-wildcard-origin responses
- `IntoAllowOrigin` trait for ergonomic origin configuration
- `ajaya::CorsLayer` re-export from facade crate
- `ajaya-middleware` added as workspace dependency

---

## [0.4.0] — 2026-04-20 — Tower Integration

### Added
- `Router::layer(layer)` — apply a Tower `Layer` to **all** requests (including 404/405)
- `Router::route_layer(layer)` — apply a Tower `Layer` to **matched routes only**
- `MethodRouter::layer(layer)` — apply a Tower `Layer` to a specific route's handlers
- `Router::into_service()` — convert `Router<()>` into a `BoxCloneService` with all layers baked in
- `Server::serve_service(svc)` — serve any pre-built `BoxCloneService` directly
- `serve_service(addr, svc)` — convenience free function for `serve_service`
- `ajaya_router::layer::BoxCloneService` — our own type-erased, clone-friendly Tower service
- `ajaya_router::layer::LayerFn` — `Arc<dyn Fn(BoxCloneService) -> BoxCloneService>` type alias
- `ajaya_router::layer::into_layer_fn(layer)` — convert any Tower `Layer` into a `LayerFn`
- `ajaya_router::layer::apply_layers(base, layers)` — apply a slice of `LayerFn` to a service
- `ajaya_router::layer::oneshot(svc, req)` — poll-ready + call helper
- `MethodRouter<S>: Clone` — required for route-layer composition
- `ajaya::BoxCloneService` and `ajaya::LayerFn` re-exports

### Changed
- `serve_app` now calls `Router::into_service()` internally — all layers are applied automatically
- `Server::serve_app` delegates to `Server::serve_service`
- `ajaya-router/Cargo.toml`: added `tower-layer` dependency
- `ajaya-hyper/Cargo.toml`: added `tower-service` dependency

---

## [0.3.4] — 2026-04-20 — Error Handling Polish

### Added
- `ErrorResponse` builder — produces standardised JSON error bodies
  `{ "error": "...", "code": 404, "request_id": "..." (optional) }`
- `ErrorResponse::request_id()` — attach a tracing ID (ready for 0.4.x RequestIdLayer)
- `impl From<Box<dyn Error + Send + Sync>> for Error`
- `Error::inner()` — access the wrapped error for logging

### Changed
- `Error::into_response()` now delegates to `ErrorResponse` for consistent formatting

---

## [0.3.3] — 2026-04-20 — Cookie Support

### Added
- `CookieJar` extractor — reads `Cookie` header, writes `Set-Cookie` via `IntoResponseParts`
- `SignedCookieJar` — HMAC-SHA256 signed cookies, requires `cookie::Key` in app state
- `PrivateCookieJar` — AES-256-GCM encrypted + authenticated cookies
- All three implement both `FromRequestParts<S>` and `IntoResponseParts`
- `cookie::Key` re-exported as `ajaya::CookieKey`
- `cookie::Cookie` re-exported as `ajaya::Cookie`
- `cookie = "0.18"` added to workspace dependencies

---

## [0.3.2] — 2026-04-20 — IntoResponseParts

### Added
- `IntoResponseParts` trait — append headers to a response without touching the body
- `ResponseParts` — accumulates extra headers during tuple processing
- `AppendHeaders<I>` — append any iterator of `(HeaderName, HeaderValue)` pairs
- `impl IntoResponseParts for http::HeaderMap`
- `(impl IntoResponseParts, impl IntoResponse): IntoResponse`
- `(P1, P2, impl IntoResponse): IntoResponse` — two header sets + body

---

## [0.3.1] — 2026-04-20 — Streaming Responses

### Added
- `StreamBody<S>` — zero-copy streaming body backed by `Stream<Item = Result<Bytes, E>>`
- `Body::from_stream()` — create a `Body` directly from any compatible stream
- `impl IntoResponse for StreamBody<S>` — return a stream directly from handlers

---

## [0.3.0] — 2026-04-20 — Response System Enhancements

### Added
- `impl IntoResponse for (http::HeaderMap, T)` — set arbitrary response headers
- `impl IntoResponse for (StatusCode, http::HeaderMap, T)` — status + headers + body

---

## [0.2.6] — 2026-04-16 — Multipart Extractor

### Added
- `Multipart` extractor — wraps `multer` crate for streaming multipart parsing
- `Multipart::next_field()` — async iteration over multipart fields
- `Field` type with `.name()`, `.file_name()`, `.content_type()`, `.bytes()`, `.text()`, `.chunk()`
- `MultipartConstraints` — configurable limits (max fields: 100, max field: 5MB, max total: 50MB)
- `MultipartRejection` — validates `Content-Type: multipart/form-data` and boundary extraction
- `MultipartRejection::PayloadTooLarge` — Returns `413 Payload Too Large` when constraints are exceeded

---

## [0.2.5] — 2026-04-16 — State Extractor

### Added
- `State<S>` extractor — clones application state from router configuration
- `FromRef<T>` trait — extract sub-types from application state
- Identity `FromRef<T> for T` blanket impl (clone the whole state)
- `Router::with_state` and `MethodRouter::with_state` methods for attaching application state

---

## [0.2.4] — 2026-04-16 — JSON, Form & Body Extractors

### Added
- `Json<T>` extractor — parses JSON body with `Content-Type: application/json` validation
  - Also implements `IntoResponse` for symmetric use as both extractor and response type
  - Supports `application/*+json` subtypes (e.g., `application/vnd.api+json`)
- `Form<T>` extractor — parses `application/x-www-form-urlencoded` body via `serde_urlencoded`
- `Bytes` extractor — raw body as `bytes::Bytes` (implemented in `ajaya-core`)
- `String` extractor — raw body as UTF-8 string (implemented in `ajaya-core`)
- `Body` extractor — raw streaming body escape hatch (implemented in `ajaya-core`)
- `Request` extractor — full request escape hatch (implemented in `ajaya-core`)
- Body consumption enforced: only one `FromRequest` extractor per handler (last parameter)

---

## [0.2.3] — 2026-04-16 — Request Metadata Extractors

### Added
- `http::Method` extractor — infallible, returns request method
- `http::Uri` extractor — infallible, returns request URI
- `http::Version` extractor — infallible, returns HTTP version
- `OriginalUri` extractor — URI before path rewrites by nesting
- `MatchedPath` extractor — the route pattern that matched (e.g., `/users/{id}`)
- `ConnectInfo<T>` extractor — client connection info (e.g., `SocketAddr`)
- `Extension<T>` extractor — typed request extension set by middleware
- Router inserts `MatchedPathExt` into request extensions during dispatch
- `MatchedPathExt` type exported from `ajaya-router`

---

## [0.2.2] — 2026-04-16 — Header Extractors

### Added
- `TypedHeader<T>` extractor — uses `headers` crate for strongly-typed header parsing
  - Supports all `headers::Header` types: `Authorization`, `ContentType`, `Host`, etc.
- `http::HeaderMap` extractor — clones the full header map (implemented in `ajaya-core`)
- `headers` crate (`v0.4`) added as workspace dependency

---

## [0.2.1] — 2026-04-16 — Path & Query Extractors

### Added
- `Path<T>` extractor — type-safe path parameter extraction via custom serde deserializer
  - Single value: `Path<u32>`, tuple: `Path<(u32, String)>`, struct: `Path<UserParams>`
  - Clear error messages on deserialization failures
- `Query<T>` extractor — query string parsing via `serde_urlencoded`
- `RawPathParams` extractor — untyped `Vec<(String, String)>` path param pairs
- Custom `PathDeserializer` with support for structs, tuples, enums, and all primitive types

---

## [0.2.0] — 2026-04-16 — Extractor Traits & Handler Macro

### Added
- **`FromRequestParts<S>` trait** — for extractors that don't consume the body
- **`FromRequest<S, M>` trait** — for body-consuming extractors (must be last handler param)
- `ViaParts` / `ViaRequest` marker types for blanket impl disambiguation
- Blanket impl: every `FromRequestParts` is also a `FromRequest` (via `ViaParts` marker)
- `Option<T>` wrapper — never rejects, returns `None` on extraction failure
- `Result<T, T::Rejection>` wrapper — gives handler access to the rejection error
- **Handler macro** — `impl_handler!` generates blanket impls for 0–16 extractors
  - T1..T(N-1) extracted from `RequestParts` via `FromRequestParts`
  - Last param TN extracted from full `Request` via `FromRequest`
- `RequestParts` struct — framework-aware request parts (HTTP parts + extensions)
- `Request::into_request_parts()` / `Request::from_request_parts()` — decompose/reconstruct
- `IntoResponse for Infallible` — for extractors that never fail
- **Rejection types** — per-extractor rejection enums implementing `IntoResponse`:
  - `PathRejection`, `QueryRejection`, `JsonRejection`, `FormRejection`
  - `TypedHeaderRejection`, `ExtensionRejection`, `StateRejection`
  - `BodyRejection`, `StringRejection`, `MultipartRejection`
  - `MatchedPathRejection`, `ConnectInfoRejection`

### Changed
- **BREAKING:** Handler blanket impls rewritten — now macro-generated for 0–16 extractors
  - Previous: only `fn()` and `fn(Request)` were supported
  - Now: any combination of extractors up to 16 parameters
- `Handler` trait now requires `S: Clone + Send + Sync` (state must be cloneable)
- Workspace version bumped from `0.1.6` to `0.2.0`
- All internal crate versions updated to `0.2.0`

### Dependencies
- Added `headers = "0.4"` to workspace

## [0.1.6] — 2026-04-12 — Tower Service Nesting

### Added
- `Router::route_service(path, service)` — mount Tower services at exact paths
- `Router::nest_service(prefix, service)` — mount Tower services under path prefixes
- `ServiceHandler<T>` adapter wrapping Tower `Service` into Ajaya `Handler`
- `tower-service` dependency added to `ajaya-router`

---

## [0.1.5] — 2026-04-12 — Router Merge & Fallback

### Added
- `Router::merge(other)` — combine routes from two routers (panic on conflict)
- `Router::fallback(handler)` — custom fallback handler for unmatched paths
- Default 404 plain text response for unmatched paths

---

## [0.1.4] — 2026-04-12 — Nested Routers

### Added
- `Router::nest(prefix, sub_router)` — compose routers under path prefixes
- Flatten strategy: nested routes inserted into parent trie at registration time
- Path parameters in prefixes work: `.nest("/users/{id}", user_router)`

---

## [0.1.3] — 2026-04-12 — Wildcard Routes

### Added
- Wildcard catch-all segments: `/files/{*path}`
- Priority ordering: static > param > wildcard (native matchit behavior)
- Wildcard values URL-decoded automatically

---

## [0.1.2] — 2026-04-12 — Path Parameters

### Added
- Path parameter extraction: `/users/{id}` extracts `id` into `PathParams`
- `PathParams::get(key)` — retrieve parameter by name
- `PathParams::iter()` — iterate over all parameters
- URL percent-decoding of parameter values
- Multiple parameters: `/users/{id}/posts/{post_id}`
- `PathParams` inserted into request extensions during dispatch

---

## [0.1.1] — 2026-04-12 — Radix Trie Router

### Changed
- **BREAKING:** Internal router storage switched from `HashMap` to `matchit` radix trie
- Zero-allocation route lookup per request
- Route conflict detection at startup (panics with clear message)

### Added
- `matchit` dependency for radix trie routing
- `PathParams` struct in `ajaya-router::params`

---

## [0.1.0] — 2026-04-12 — Static Router

### Added
- `Router<S>` — path-based HTTP router with `.route(path, method_router)` API
- `serve_app(addr, router)` convenience function in `ajaya-hyper`
- `Server::serve_app(router)` method for Router-based serving
- Path normalization (trailing slash stripping)
- Re-exported `Router` and `serve_app` from the `ajaya` facade crate

---

## [0.0.5] — 2026-04-12

### Added

- **Error Foundation** — Complete error handling system
  - `Error` type now implements `IntoResponse` — produces JSON error bodies: `{"error": "message", "code": 404}`
  - `Result<T: IntoResponse, E: IntoResponse>` implements `IntoResponse` — enables `?` propagation in handlers
  - `From` impls for `std::io::Error`, `serde_json::Error`, `http::Error`, `String`, `&str`
  - `Error::from_status(StatusCode)` convenience constructor
  - Internal error details are never leaked to clients — only public messages are exposed
- **`Json<T>` response type** — Serialize any `T: Serialize` as JSON with `Content-Type: application/json`
- **`Html<T>` response type** — Return HTML with `Content-Type: text/html; charset=utf-8`

---

## [0.0.4] — 2026-04-12

### Added

- **Method Dispatch** — Differentiate HTTP methods at the server level
  - `MethodFilter` — bitflag enum for matching HTTP methods (`GET`, `POST`, `PUT`, `DELETE`, `PATCH`, `HEAD`, `OPTIONS`, `TRACE`, `ANY`)
  - `MethodRouter<S>` — stores one handler per HTTP method with dispatch
  - Top-level constructor functions: `get()`, `post()`, `put()`, `delete()`, `patch()`, `head()`, `options()`, `trace_method()`, `any()`, `on()`
  - Method chaining: `get(handler).post(handler).delete(handler)`
  - Returns `405 Method Not Allowed` with `Allow` header for unmatched methods
- **`serve_router(addr, router)`** — serve a `MethodRouter` directly
- **`Server::serve_method_router(router)`** — lower-level method router serving

---

## [0.0.3] — 2026-04-12

### Added

- **Handler Trait** — Core request handling abstraction
  - `Handler<T, S>` trait definition with `call(self, req, state) -> Future<Output = Response>`
  - Blanket impl for `async fn() -> impl IntoResponse` (zero-argument handlers)
  - Blanket impl for `async fn(Request) -> impl IntoResponse` (request handlers)
  - Type-erased handler storage via `ErasedHandler` trait for dynamic dispatch
- **`IntoResponse` trait** — Full implementations for common types:
  - `Response` (identity), `StatusCode` (empty body), `String`, `&'static str` (text/plain)
  - `Bytes`, `Vec<u8>` (application/octet-stream), `()` (200 OK empty)
  - `(StatusCode, T)` tuple for custom status codes
  - `(StatusCode, [(K, V); N], T)` tuple for status + headers + body
  - `([(K, V); N], T)` tuple for headers + body
- **Handler-based `serve()`** — `serve(addr, handler)` now accepts any `Handler<T>`
- **Updated `ajaya` facade** — Re-exports `Handler`, `IntoResponse`, `ResponseBuilder`, `Redirect`

### Changed

- `Server::serve()` now requires a handler argument (breaking change from v0.0.1)
- `serve()` now takes two arguments: `serve(addr, handler)` instead of `serve(addr)`

---

## [0.0.2] — 2026-04-12

### Added

- **Real `Body` type** — Replaced `Full<Bytes>` type alias with a proper struct
  - `Body` wraps a type-erased `Pin<Box<dyn http_body::Body>>` for any body source
  - `Body::empty()` — zero-byte body
  - `Body::from_bytes(Bytes)` — body from raw bytes
  - `Body::to_bytes()` — async collect to `Bytes`
  - `Body::to_string()` — async collect to UTF-8 `String`
  - `From` impls: `String`, `&'static str`, `Bytes`, `Vec<u8>`, `Full<Bytes>`, `()`
  - Implements `http_body::Body` for direct Hyper integration
- **`ResponseBuilder`** — Fluent API for response construction
  - `.status()`, `.header()`, `.body()`, `.json()`, `.html()`, `.text()`, `.empty()`
- **`Redirect`** — convenience redirect responses: `Redirect::to()`, `::permanent()`, `::temporary()`
- **Enhanced `Request<B>`**
  - `Request::from_hyper()` — convert `hyper::Request<Incoming>` to Ajaya's `Request<Body>`
  - `into_parts()` — decompose into `(Parts, Body)`
  - `version()` — HTTP version accessor
  - `extension::<T>()` — typed extension getter
  - `headers_mut()` — mutable header access
  - `map_body()` — transform body type
- **`IntoResponse` trait** — Stub definition (implementations in v0.0.3)

### Changed

- `ajaya-hyper` server now converts incoming Hyper requests to `ajaya_core::Request` and returns `Response<Body>` using `ResponseBuilder`

---

## [0.0.1] — 2026-04-11

### Added

- **Workspace Bootstrap** — Cargo workspace with all 12 crates initialized
- **ajaya** — Facade crate with re-exports and binary entry point
- **ajaya-core** — Core type stubs: `Request`, `Response`, `Body`, `Error`
  - `Request<B>` wrapper around `http::Request<B>` with extensions
  - `Response<B>` type alias for `http::Response<B>`
  - `Body` type alias for `Full<Bytes>` (will be replaced with `BoxBody` in v0.0.2)
  - `Error` struct with HTTP status code, inner error, and public message
- **ajaya-hyper** — Working Hyper 1.x TCP server
  - `Server::bind(addr)` — binds to a TCP address
  - `Server::serve()` — infinite accept loop with per-connection Tokio tasks
  - `serve(addr)` — convenience one-liner to start the server
  - Responds "Hello from Ajaya" to every HTTP request
  - `Content-Type: text/plain; charset=utf-8` and `Server: Ajaya/0.0.1` headers
- **Stub crates** — Empty `lib.rs` with documentation for future implementation:
  - `ajaya-router` — Radix trie router (planned for v0.1.x)
  - `ajaya-extract` — Request extractors (planned for v0.2.x)
  - `ajaya-middleware` — Built-in middleware (planned for v0.4.x)
  - `ajaya-ws` — WebSocket support (planned for v0.5.x)
  - `ajaya-sse` — Server-Sent Events (planned for v0.5.x)
  - `ajaya-static` — Static file serving (planned for v0.6.x)
  - `ajaya-tls` — TLS support (planned for v0.6.x)
  - `ajaya-macros` — Proc macros (planned for v0.7.x)
  - `ajaya-test` — Testing utilities (planned for v0.7.x)
- **CI** — GitHub Actions workflow: `cargo check`, `cargo clippy`, `cargo test`, `cargo fmt`
- **Documentation** — `README.md`, `ARCHITECTURE.md`, `ROADMAP.md`
- **License** — MIT + Apache 2.0 dual license

### Infrastructure

- Workspace dependencies defined in root `Cargo.toml` (70+ shared dependency versions)
- All crates inherit `version`, `edition`, `license`, `repository`, `authors`, `rust-version` from workspace
- `resolver = "2"` for proper feature unification

---

[Unreleased]: https://github.com/AarambhDevHub/ajaya/compare/v0.2.6...HEAD
[0.2.6]: https://github.com/AarambhDevHub/ajaya/compare/v0.2.5...v0.2.6
[0.2.5]: https://github.com/AarambhDevHub/ajaya/compare/v0.2.4...v0.2.5
[0.2.4]: https://github.com/AarambhDevHub/ajaya/compare/v0.2.3...v0.2.4
[0.2.3]: https://github.com/AarambhDevHub/ajaya/compare/v0.2.2...v0.2.3
[0.2.2]: https://github.com/AarambhDevHub/ajaya/compare/v0.2.1...v0.2.2
[0.2.1]: https://github.com/AarambhDevHub/ajaya/compare/v0.2.0...v0.2.1
[0.2.0]: https://github.com/AarambhDevHub/ajaya/compare/v0.1.6...v0.2.0
[0.1.6]: https://github.com/AarambhDevHub/ajaya/compare/v0.1.5...v0.1.6
[0.0.5]: https://github.com/AarambhDevHub/ajaya/compare/v0.0.4...v0.0.5
[0.0.4]: https://github.com/AarambhDevHub/ajaya/compare/v0.0.3...v0.0.4
[0.0.3]: https://github.com/AarambhDevHub/ajaya/compare/v0.0.2...v0.0.3
[0.0.2]: https://github.com/AarambhDevHub/ajaya/compare/v0.0.1...v0.0.2
[0.0.1]: https://github.com/AarambhDevHub/ajaya/releases/tag/v0.0.1
