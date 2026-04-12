# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

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

[Unreleased]: https://github.com/AarambhDevHub/ajaya/compare/v0.0.5...HEAD
[0.0.5]: https://github.com/AarambhDevHub/ajaya/compare/v0.0.4...v0.0.5
[0.0.4]: https://github.com/AarambhDevHub/ajaya/compare/v0.0.3...v0.0.4
[0.0.3]: https://github.com/AarambhDevHub/ajaya/compare/v0.0.2...v0.0.3
[0.0.2]: https://github.com/AarambhDevHub/ajaya/compare/v0.0.1...v0.0.2
[0.0.1]: https://github.com/AarambhDevHub/ajaya/releases/tag/v0.0.1
