# Ajaya (अजय) — The Unconquerable Rust Web Framework

<div align="center">

**🔱 Built on Tokio + Hyper. Engineered for maximum performance.**

[![Rust](https://img.shields.io/badge/rust-1.85%2B-orange.svg)](https://www.rust-lang.org)
[![License](https://img.shields.io/badge/license-MIT%2FApache--2.0-blue.svg)](LICENSE-MIT)
[![Version](https://img.shields.io/badge/version-0.0.5-green.svg)](CHANGELOG.md)
[![Discord](https://img.shields.io/discord/placeholder?label=discord&logo=discord&logoColor=white)](https://discord.gg/HDth6PfCnp)

</div>

---

## What is Ajaya?

**Ajaya** (अजय, *"The Unconquerable"*) is a high-performance Rust web framework built from the ground up on **Tokio** and **Hyper 1.x**. It aims to unify the best features of Axum and Actix-web under one ergonomic, blazing-fast API.

> 🔱 **v0.0.5 — Foundation Complete.** Core types, handler trait, method dispatch, and error handling are implemented. Write async handlers, dispatch by HTTP method, return JSON responses, and propagate errors with `?`. Follow along on [YouTube](https://youtube.com/@AarambhDevHub) or join the [Discord](https://discord.gg/HDth6PfCnp) to track progress.

---

## Quick Start

```bash
# Clone the repo
git clone https://github.com/AarambhDevHub/ajaya.git
cd ajaya

# Run the server
cargo run -p ajaya
```

Then in another terminal:

```bash
curl http://localhost:8080
# => {"status":"healthy","framework":"Ajaya","version":"0.0.5"}

curl -X POST http://localhost:8080
# => {"status":"created","id":42}

curl -X DELETE http://localhost:8080
# => 405 Method Not Allowed
```

---

## Features (v0.0.5)

### ✅ Handler System

Any async function that returns `impl IntoResponse` works as a handler:

```rust
use ajaya::{get, serve_router, Json, Error};
use http::StatusCode;

// JSON response
async fn health() -> Result<Json<serde_json::Value>, Error> {
    Ok(Json(serde_json::json!({
        "status": "healthy",
        "version": "0.0.5"
    })))
}

// Status code + body
async fn create() -> (StatusCode, Json<serde_json::Value>) {
    (StatusCode::CREATED, Json(serde_json::json!({ "id": 42 })))
}

#[tokio::main]
async fn main() {
    let router = get(health).post(create);
    serve_router("0.0.0.0:8080", router).await.unwrap();
}
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

### ✅ Method Dispatch

```rust
use ajaya::{get, delete, patch};

let router = get(get_handler)
    .post(create_handler)
    .put(update_handler)
    .delete(delete_handler)
    .patch(patch_handler);
```

Unmatched methods return **405 Method Not Allowed** with an `Allow` header.

### ✅ Error Handling

Handlers can return `Result<T, Error>` and use `?` for error propagation.
Errors produce JSON responses — internal details are never leaked:

```json
{"error": "Not Found", "code": 404}
```

---

## Workspace Structure

```
ajaya/
├── ajaya/              # Facade crate (re-exports everything)
├── ajaya-core/         # Core: Request, Response, Body, Handler, IntoResponse, Error
├── ajaya-router/       # MethodRouter — HTTP method dispatch
├── ajaya-hyper/        # Hyper 1.x server integration
├── ajaya-extract/      # Extractors: Path, Query, Json, Form (coming in v0.2.x)
├── ajaya-middleware/   # CORS, compression, timeout, etc. (coming in v0.4.x)
├── ajaya-ws/           # WebSocket support (coming in v0.5.x)
├── ajaya-sse/          # Server-Sent Events (coming in v0.5.x)
├── ajaya-static/       # Static file serving (coming in v0.6.x)
├── ajaya-tls/          # TLS via rustls (coming in v0.6.x)
├── ajaya-macros/       # Proc macros: #[handler], #[route] (coming in v0.7.x)
└── ajaya-test/         # Testing utilities (coming in v0.7.x)
```

---

## Roadmap

See [ROADMAP.md](ROADMAP.md) for the complete version-by-version plan.

| Version | Focus | Status |
|---------|-------|--------|
| **0.0.x** | Foundation & Core | ✅ Complete |
| 0.1.x | Routing System | 🚧 Next Up |
| 0.2.x | Extractors | ⏳ Planned |
| 0.3.x | Responses & Error Handling | ⏳ Planned |
| 0.4.x | Middleware | ⏳ Planned |
| 0.5.x | WebSocket, SSE, Multipart | ⏳ Planned |
| 0.6.x | TLS, HTTP/2, Static Files | ⏳ Planned |
| 0.7.x | Macros, Testing, Config | ⏳ Planned |
| 0.8.x | Observability & Security | ⏳ Planned |
| 0.9.x | Performance Sprint | ⏳ Planned |
| 0.10.x | Stabilization & Docs | ⏳ Planned |

---

## Architecture

See [ARCHITECTURE.md](ARCHITECTURE.md) for the full technical specification including all planned APIs, crate responsibilities, extractor system, middleware design, and performance architecture.

---

## Performance Targets

| Benchmark | Target | Actix-web |
|-----------|--------|-----------|
| Plaintext | 800K req/sec | 600K req/sec |
| JSON | 500K req/sec | 380K req/sec |
| Single Query | 200K req/sec | 150K req/sec |

Benchmarks will be tracked in `examples/benchmarks/` starting from `v0.9.x`.

---

## Contributing

Ajaya is being built in public from `0.0.5`. Contributions are welcome at every stage.

See **[CONTRIBUTING.md](CONTRIBUTING.md)** for the full guide — setup, coding standards, commit format, and PR process.

Quick start for contributors:

```bash
git clone https://github.com/AarambhDevHub/ajaya.git
cd ajaya
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
| 🐙 GitHub Discussions | [Discussions](https://github.com/AarambhDevHub/ajaya/discussions) | Feature proposals, Q&A |
| 🐛 GitHub Issues | [Issues](https://github.com/AarambhDevHub/ajaya/issues) | Bug reports |

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

*Ajaya (अजय) — Unconquerable. Built by [Aarambh Dev Hub](https://github.com/AarambhDevHub).* 🔱