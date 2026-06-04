# Arvik — Fast, Typed, and Fearless Web Framework for Rust

<div align="center">

**⚡ A · R · V · I · K — Async Rust Velocity Integration Kit**

*Built on Tokio + Hyper. Engineered for maximum performance.*

[![Rust](https://img.shields.io/badge/rust-1.85%2B-orange.svg)](https://www.rust-lang.org)
[![License](https://img.shields.io/badge/license-MIT%2FApache--2.0-blue.svg)](LICENSE-MIT)
[![Version](https://img.shields.io/badge/version-0.7.2-green.svg)](CHANGELOG.md)
[![Discord](https://img.shields.io/discord/placeholder?label=discord&logo=discord&logoColor=white)](https://discord.gg/HDth6PfCnp)

</div>

---

## What is Arvik?

**Arvik** stands for **A**sync **R**ust **V**elocity **I**ntegration **K**it.

It is a high-performance Rust web framework built from the ground up on **Tokio** and **Hyper 1.x**, designed to unify the best features of Axum and Actix-web under one ergonomic, blazing-fast API.

> ⚡ **v0.7.2 — Proc Macros** Arvik now ships rustls HTTPS, HTTP/2 tuning, static assets, and opt-in `#[debug_handler]`, `#[route]`, and `#[handler]` macros. Follow along on [YouTube](https://youtube.com/@AarambhDevHub) or join the [Discord](https://discord.gg/HDth6PfCnp) to track progress.

---

## Quick Start

```bash
# Clone the repo
git clone https://github.com/AarambhDevHub/arvik.git
cd arvik

# Run the server
cargo run -p arvik
```

Then in another terminal:

```bash
curl http://localhost:8080/
# => {"status":"healthy","framework":"Arvik","version":"0.7.2"}

curl http://localhost:8080/users/42
# => {"id":"42","name":"User from path param"}

curl http://localhost:8080/not-a-route
# => 404 Not Found
```

## Running Examples

The examples are kept outside the main workspace checks, so run them with their
own manifests:

```bash
# Full REST API demo
cargo run --manifest-path examples/rest_api/Cargo.toml

# WebSocket chat demo
cargo run --manifest-path examples/websocket_chat/Cargo.toml

# Server-Sent Events demo
cargo run --manifest-path examples/sse_demo/Cargo.toml

# HTTP/2 cleartext, requires the example's http2 feature dependency
cargo run --manifest-path examples/h2c/Cargo.toml

# rustls HTTPS, uses a generated self-signed development certificate
cargo run --manifest-path examples/tls_rustls/Cargo.toml

# rustls HTTPS with hot reload, requires PEM certificate and key paths
cargo run --manifest-path examples/tls_hot_reload/Cargo.toml -- cert.pem key.pem

# native-tls HTTPS, requires a PKCS#12 identity and optional password
cargo run --manifest-path examples/native_tls/Cargo.toml -- identity.p12 password

# Filesystem static files, requires static-files
cargo run --manifest-path examples/static_files/Cargo.toml

# Embedded static files, requires embedded-static
cargo run --manifest-path examples/embedded_static/Cargo.toml

# Better handler diagnostics, requires macros
cargo run --manifest-path examples/debug_handler/Cargo.toml

# Attribute routes, requires macros
cargo run --manifest-path examples/route_macro/Cargo.toml

# Struct handlers, requires macros
cargo run --manifest-path examples/handler_macro/Cargo.toml
```

The TLS examples listen on `0.0.0.0:8443`; the plain HTTP examples listen on
their own configured ports. Native TLS ALPN support depends on the platform TLS
stack, while rustls is the guaranteed HTTP/2 ALPN backend.

---

## Features (v0.7.2)

### ✅ Type-Safe Extractors

Extract typed data from requests with compile-time safety. Handlers support up to 16 extractors.

```rust
use arvik::{Router, get, post, Json, Path, Query, State};
use serde::{Deserialize, Serialize};

#[derive(Deserialize)]
struct SearchParams { q: String, page: Option<u32> }

#[derive(Deserialize)]
struct CreateUser { name: String, email: String }

#[derive(Serialize)]
struct User { id: u32, name: String }

// Path + Query extractors
async fn search(Path(id): Path<u32>, Query(params): Query<SearchParams>) -> String {
    format!("User {id} searching: {}", params.q)
}

// JSON body extractor
async fn create_user(Json(body): Json<CreateUser>) -> Json<User> {
    Json(User { id: 1, name: body.name })
}
```

### ✅ Available Extractors

| Extractor | Source | Notes |
|---|---|---|
| `Path<T>` | URL path params | Serde deserialization |
| `Query<T>` | Query string | Via `serde_urlencoded` |
| `Json<T>` | Request body | Validates Content-Type |
| `Form<T>` | Request body | URL-encoded forms |
| `State<S>` | Router state | Shared app state |
| `TypedHeader<T>` | Request headers | Via `headers` crate |
| `Extension<T>` | Extensions map | Middleware data |
| `MatchedPath` | Router | Route pattern |
| `ConnectInfo<T>` | Connection | Client address |
| `Multipart` | Request body | File uploads |
| `Method` / `Uri` / `Version` | Request | HTTP metadata |
| `Bytes` / `String` / `Body` | Request body | Raw access |

### ✅ Powerful Routing System

Zero-allocation request matching, dynamic path parameters, and catch-all wildcards.

```rust
use arvik::{Router, get, post, Json, Path};

async fn get_user(Path(id): Path<u32>) -> Json<serde_json::Value> {
    Json(serde_json::json!({ "user_id": id }))
}

#[tokio::main]
async fn main() {
    let app = Router::new()
        .route("/", get(|| async { "Home" }))
        .route("/users/{id}", get(get_user))
        .route("/files/{*path}", get(|| async { "File content" }));

    arvik::serve_app("0.0.0.0:8080", app).await.unwrap();
}
```

### ✅ Router Composition

Seamlessly nest routers underneath prefixes or merge them flatly:

```rust
let api = Router::new().route("/users", get(list_users));
let admin = Router::new().route("/dashboard", get(dashboard));

let app = Router::new()
    .nest("/api/v1", api)
    .merge(admin);
```

### ✅ Response Types

| Return Type | Content-Type | Status |
|---|---|---|
| `&'static str` / `String` | `text/plain` | 200 |
| `Json<T: Serialize>` | `application/json` | 200 |
| `Html<T: Into<String>>` | `text/html` | 200 |
| `StatusCode` | — (empty body) | Any |
| `(StatusCode, T)` | Inherits from `T` | Custom |
| `Result<T, E>` | Inherits from `Ok`/`Err` | Auto |
| `Bytes` / `Vec<u8>` | `application/octet-stream` | 200 |

### ✅ Error Handling

Handlers can return `Result<T, Error>` and use `?` for error propagation. Errors produce secure JSON responses — internal details are never leaked.

### ✅ Complete Middleware System

| Layer | Description |
|---|---|
| `CorsLayer` | Full CORS spec, `permissive()` and `very_permissive()` presets |
| `CompressionLayer` | gzip, brotli, zstd, deflate response compression |
| `DecompressionLayer` | Request body decompression |
| `TimeoutLayer` | 408 on slow handlers |
| `RequestIdLayer` | UUID v4 per request in `x-request-id` header |
| `TraceLayer` | Structured tracing spans (method, path, status, latency) |
| `SecurityHeadersLayer` | Full OWASP header suite (X-Frame-Options, HSTS, CSP...) |
| `SetResponseHeaderLayer` | Set/override/append response headers |
| `SensitiveHeadersLayer` | Redact sensitive headers in logs |
| `RateLimitLayer` | Token bucket per IP / header / global |
| `RequireAuthorizationLayer` | Bearer, Basic, or custom auth |
| `RequestBodyLimitLayer` | 413 on oversized request bodies |
| `CatchPanicLayer` | 500 on handler panics, no server crash |
| `MapRequestBodyLayer` | Transform request body bytes |
| `MapResponseBodyLayer` | Transform response body bytes |
| `CsrfLayer` | Double-submit cookie CSRF protection |
| `from_fn` | Middleware from a plain async function |
| `from_fn_with_state` | Same, with access to router state |
| `map_request` | Transform request only (no response) |
| `map_response` | Transform response only (no request) |

### ✅ Multipart Uploads

Multipart uploads are parsed as a stream. Large files can be processed chunk by chunk or written to a secure temporary file without buffering the full upload in memory.

```rust
use arvik::{Multipart, MultipartError};
use futures_util::StreamExt as _;

async fn upload(mut multipart: Multipart) -> Result<String, MultipartError> {
    let mut files = 0;

    while let Some(field) = multipart.next_field().await? {
        if field.file_name().is_some() {
            let temp = field.save_to_temp().await?;
            files += 1;
            println!(
                "saved {:?}: {} bytes",
                temp.metadata().file_name(),
                temp.bytes_written()
            );
        } else {
            let mut stream = field.into_progress_stream();
            while let Some(chunk) = stream.next().await {
                let chunk = chunk?;
                println!("field progress: {} bytes", chunk.bytes_read());
            }
        }
    }

    Ok(format!("received {files} files"))
}
```

Configure limits and temp-file placement through request extensions from middleware:

```rust
use arvik::{middleware::map_request, MultipartConfig, Router, post};

let app = Router::new()
    .route("/upload", post(upload))
    .layer(map_request(|mut req: arvik::Request| async move {
        req.extensions_mut().insert(
            MultipartConfig::new()
                .max_fields(20)
                .max_field_size(10 * 1024 * 1024)
                .max_total_size(100 * 1024 * 1024)
                .with_temp_dir("/var/tmp/arvik-uploads"),
        );
        req
    }));
```

### ✅ WebSocket Support

Full WebSocket support via `arvik-ws`, built on `tokio-tungstenite`. Auto-pong keeps connections alive with zero application boilerplate.

> **WebSocket is opt-in.** Enable it by adding the `ws` feature to your `Cargo.toml`:
>
> ```toml
> # Opt in to WebSocket
> arvik = { version = "0.7", features = ["ws"] }
> ```
>
> Default build (`arvik = "0.6"`) is HTTP-only — no WebSocket compiled in.

```rust
use arvik::{Router, get};
use arvik::ws::{WebSocket, WebSocketUpgrade, Message};

async fn ws_handler(ws: WebSocketUpgrade) -> impl arvik::IntoResponse {
    ws.on_upgrade(|mut socket| async move {
        // Ping/Pong handled automatically — no extra match arm needed
        while let Some(Ok(msg)) = socket.recv().await {
            match msg {
                Message::Text(text) => {
                    socket.send(Message::Text(format!("echo: {text}"))).await.ok();
                }
                Message::Binary(data) => {
                    socket.send(Message::Binary(data)).await.ok();
                }
                Message::Close(_) => break,
                _ => {}
            }
        }
    })
}

// With config + subprotocol negotiation
async fn chat_handler(ws: WebSocketUpgrade) -> impl arvik::IntoResponse {
    ws.protocols(["chat", "json"])
      .max_message_size(64 * 1024)   // 64 KB
      .max_frame_size(16 * 1024)     // 16 KB
      .on_upgrade(|socket| async move {
          let (mut sender, mut receiver) = socket.split();
          while let Some(Ok(msg)) = receiver.next().await {
              sender.send(msg).await.ok();
          }
      })
}

let app = Router::new()
    .route("/ws", get(ws_handler))
    .route("/chat", get(chat_handler));
```

**Features at a glance:**

| Feature | Detail |
|---|---|
| Auto ping/pong | `recv()` replies to Ping transparently |
| Split send/receive | `socket.split()` → `(Sender, Receiver)` |
| `Stream` impl | `Receiver` works with `futures_util` combinators |
| Subprotocol negotiation | `.protocols(["chat", "json"])` |
| Configurable limits | `max_message_size`, `max_frame_size` |
| Typed rejections | `WebSocketUpgradeRejection` with correct HTTP codes |
| RFC 6455 compliant | SHA-1 accept key, full close code enum |

### ✅ Server-Sent Events (SSE)

Full Server-Sent Events support via `arvik-sse`. Send real-time updates to the browser with built-in keep-alive pings and zero-allocation string rendering.

> **SSE is opt-in.** Enable it by adding the `sse` feature to your `Cargo.toml`:
>
> ```toml
> arvik = { version = "0.7", features = ["sse"] }
> ```

```rust
use arvik::{Router, get};
use arvik::sse::{Event, KeepAlive, Sse};
use std::time::Duration;
use tokio_stream::StreamExt as _;

async fn json_stream() -> Sse<impl futures_util::Stream<Item = Result<Event, serde_json::Error>>> {
    let stream = tokio_stream::wrappers::IntervalStream::new(
        tokio::time::interval(Duration::from_millis(500)),
    )
    .enumerate()
    .map(|(i, _): (usize, _)| {
        Event::default()
            .event("metric")
            .id(i.to_string())
            .json_data(&serde_json::json!({ "seq": i }))
    });

    // Automatically send a `: \n\n` comment every 10 seconds to keep proxies alive
    Sse::new(stream).keep_alive(KeepAlive::new().interval(Duration::from_secs(10)))
}

let app = Router::new().route("/stream", get(json_stream));
```

### ✅ TLS and HTTP/2

TLS support is opt-in. Rustls is the primary backend and provides the guaranteed HTTP/2 ALPN path.

```toml
arvik = { version = "0.7", features = ["tls"] }
```

```rust
use arvik::{Router, RustlsConfig, get, serve_tls};

async fn hello() -> &'static str {
    "secure hello"
}

let app = Router::new().route("/", get(hello));
let tls = RustlsConfig::from_pem_file("cert.pem", "key.pem").await?;
serve_tls(app, "0.0.0.0:443", tls).await?;
```

Hot reload uses the `tls-hot-reload` feature and debounces file changes. Failed reloads keep the previous config active, and existing connections continue using the session they already accepted.

```toml
arvik = { version = "0.7", features = ["tls-hot-reload"] }
```

```rust
let tls = RustlsConfig::from_pem_file("cert.pem", "key.pem").await?;
let _watcher = tls.watch_pem_files("cert.pem", "key.pem", std::time::Duration::from_secs(1))?;
```

Native TLS is available for platform TLS stacks. ALPN support is best-effort and platform-dependent; if HTTP/2 ALPN is not available, Arvik falls back cleanly to HTTP/1.1.

```toml
arvik = { version = "0.7", features = ["native-tls"] }
```

HTTP/2 over cleartext is available for internal service-to-service traffic:

```rust
use arvik::{Router, ServerConfig, get, serve_h2c_with_config};

let app = Router::new().route("/", get(hello));
let config = ServerConfig::new()
    .http2_adaptive_window(true)
    .http2_max_concurrent_streams(1_000);

serve_h2c_with_config(app, "0.0.0.0:8080", config).await?;
```

`ServerConfig` is intentionally limited to connection and protocol tuning. File/env config loading, including future `arvik.toml`, belongs to the later `0.7.4` configuration roadmap.

HTTP/2 server push promises are not implemented in Arvik Hyper mode because the browser feature is deprecated and Hyper does not expose a stable push API. Use `Preload` / `PreloadLink` to emit `Link: rel=preload` headers instead.

### ✅ Static Assets

Filesystem static serving is opt-in with `static-files`.

```toml
arvik = { version = "0.7", features = ["static-files"] }
```

```rust
use arvik::{Router, ServeDir, ServeFile};
use std::path::PathBuf;

let assets = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("assets");
let favicon = assets.join("favicon.ico");

let app = Router::new()
    .route_service("/favicon.ico", ServeFile::new(favicon))
    .nest_service(
        "/static",
        ServeDir::new(assets)
            .precompressed_gzip()
            .precompressed_br()
            .directory_listing(true)
            .cache_control("public, max-age=60"),
    );
```

Embedded static assets use `embedded-static` and `rust-embed`.

```toml
arvik = { version = "0.7", features = ["embedded-static"] }
```

```rust
use arvik::{Embed, EmbeddedFileService, Router};

#[derive(Embed)]
#[folder = "assets"]
#[crate_path = "arvik"]
struct Assets;

let app = Router::new().nest_service(
    "/assets",
    EmbeddedFileService::<Assets>::new()
        .precompressed_gzip()
        .precompressed_br()
        .cache_control("public, max-age=31536000, immutable"),
);
```

Static responses include MIME detection, `Last-Modified`, `ETag`, conditional `304`, single byte ranges, optional directory listings, and best-match `.br` / `.gz` precompressed variants.

### ✅ Proc Macros

Proc macros are opt-in with `macros`.

```toml
arvik = { version = "0.7", features = ["macros"] }
```

```rust
use arvik::{Path, Router, State, debug_handler};

#[derive(Clone)]
struct AppState;

#[debug_handler(state = AppState)]
async fn echo(State(_state): State<AppState>, body: String) -> String {
    body
}

#[arvik::get("/users/:id")]
async fn get_user(Path(id): Path<u64>) -> String {
    format!("User #{id}")
}

#[derive(Clone)]
struct Inspect;

#[arvik::handler]
impl Inspect {
    async fn call(&self, req: arvik::Request) -> String {
        req.uri().path().to_string()
    }
}

let app: Router<AppState> = Router::new()
    .routes(arvik::collect_routes![get_user])
    .route("/inspect", arvik::get(Inspect));
```

`collect_routes!` detects duplicate path/method pairs within one collection at compile time. Use an impl-block `#[handler]`; a struct attribute cannot inspect a later inherent `impl`.

---

## Workspace Structure

```
arvik/
├── arvik/              # Facade crate (re-exports everything)
├── arvik-core/         # Core: Request, Response, Body, Handler, IntoResponse, Error
├── arvik-router/       # MethodRouter — HTTP method dispatch
├── arvik-hyper/        # Hyper 1.x server integration
├── arvik-extract/      # Extractors: Path, Query, Json, Form, Multipart
├── arvik-middleware/   # CORS, compression, timeout, auth, rate limits
├── arvik-ws/           # WebSocket support (v0.5.0 ✅)
├── arvik-sse/          # Server-Sent Events (v0.5.1 ✅)
├── arvik-static/       # Static file serving + embedded assets (v0.6.x ✅)
├── arvik-tls/          # TLS via rustls + native-tls (v0.6.x ✅)
├── arvik-macros/       # Proc macros: #[debug_handler], #[route], #[handler] (v0.7.x ✅)
└── arvik-test/         # Testing utilities (coming in v0.7.x)
```

---

## Roadmap

See [ROADMAP.md](ROADMAP.md) for the complete version-by-version plan.

| Version | Focus | Status |
|---------|-------|--------|
| **0.0.x** | Foundation & Core | ✅ Complete |
| **0.1.x** | Routing System | ✅ Complete |
| **0.2.x** | Extractors | ✅ Complete |
| **0.3.x** | Responses & Error Handling | ✅ Complete |
| **0.4.x** | Middleware | ✅ Complete |
| **0.5.0** | WebSocket | ✅ Complete |
| **0.5.1** | Server-Sent Events (SSE) | ✅ Complete |
| **0.5.2** | Multipart Polish | ✅ Complete |
| **0.6.0** | rustls TLS | ✅ Complete |
| **0.6.1** | TLS Hot Reload | ✅ Complete |
| **0.6.2** | native-tls Backend | ✅ Complete |
| **0.6.3** | HTTP/2 Tuning | ✅ Complete |
| **0.6.4** | Static File Serving | ✅ Complete |
| **0.6.5** | Embedded Static Files | ✅ Complete |
| **0.7.0** | `#[debug_handler]` Macro | ✅ Complete |
| **0.7.1** | `#[route]` Macro | ✅ Complete |
| **0.7.2** | `#[handler]` Macro | ✅ Complete |
| 0.7.3+ | Test Client, Config | ⏳ Planned |
| 0.8.x | Observability & Security | ⏳ Planned |
| 0.9.x | Performance Sprint | ⏳ Planned |
| 0.10.x | Stabilization & Docs | ⏳ Planned |

---

## Architecture

See [ARCHITECTURE.md](ARCHITECTURE.md) for the full technical specification including all planned APIs, crate responsibilities, extractor system, middleware design, and performance architecture.

---

## Performance

Arvik aims to unify extreme ergonomics with world-class performance. Here is how Arvik compares against the Rust heavyweights in a simple TCP path routing test (`wrk -t4 -c100 -d10s`), built in `--release` mode and run simultaneously on the same hardware.

| Framework | Version | Requests / sec | Latency (avg) | Underlying Engine |
| --- | --- | --- | --- | --- |
| Actix-Web | v4 | `331,131 req/s` | `483 µs` | Custom HTTP worker model |
| Axum | v0.8.x | `301,439 req/s` | `349 µs` | Tokio / Hyper 1.x |
| **Arvik** | **v0.3.4** | **`307,177 req/s`** | **`333 µs`** | Tokio / Hyper 1.x |

*Tested using `wrk` with 100 concurrent workers across 4 threads for 10 seconds. Arvik achieves performance completely matched with Axum out of the box, powered by its zero-allocation radix trie path routing.*

---

## Contributing

Arvik is being built in public from `0.0.5`. Contributions are welcome at every stage.

See **[CONTRIBUTING.md](CONTRIBUTING.md)** for the full guide — setup, coding standards, commit format, and PR process.

Quick start for contributors:

```bash
git clone https://github.com/AarambhDevHub/arvik.git
cd arvik
cargo check --workspace
cargo clippy --workspace -- -D warnings
cargo test --workspace
```

---

## Community

| Platform | Link | Purpose |
|----------|------|---------|
| 💬 Discord | [Aarambh Dev Hub](https://discord.gg/HDth6PfCnp) | Questions, discussion, dev updates |
| 📺 YouTube | [Aarambh Dev Hub](https://youtube.com/@AarambhDevHub) | Build-in-public video series |
| 🐙 GitHub Discussions | [Discussions](https://github.com/AarambhDevHub/arvik/discussions) | Feature proposals, Q&A |
| 🐛 GitHub Issues | [Issues](https://github.com/AarambhDevHub/arvik/issues) | Bug reports |

---

## Support

If Arvik has been useful to you, consider supporting the project:

[![Buy Me a Coffee](https://img.shields.io/badge/Buy%20Me%20a%20Coffee-support-yellow?logo=buy-me-a-coffee)](https://buymeacoffee.com/aarambhdevhub)
[![GitHub Sponsors](https://img.shields.io/badge/GitHub%20Sponsors-support-ea4aaa?logo=github)](https://github.com/sponsors/aarambh-darshan)
[![Razorpay](https://img.shields.io/badge/Razorpay-donate-02042b?logo=razorpay)](https://razorpay.me/@aarambhdevhub)

---

## Security

Found a vulnerability? Please **do not** open a public issue.
See [SECURITY.md](SECURITY.md) for responsible disclosure instructions.

---

## License

Dual-licensed under [MIT](LICENSE-MIT) or [Apache-2.0](LICENSE-APACHE) at your option.

```
Copyright 2026 Aarambh Dev Hub
```

---

*Arvik — **A**sync **R**ust **V**elocity **I**ntegration **K**it. Built by [Aarambh Dev Hub](https://github.com/AarambhDevHub).* ⚡
