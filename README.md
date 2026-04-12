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

> 🔱 **v0.1.6 — Routing System Complete.** Ajaya now features a lightning-fast matchit radix trie router. Write async handlers, extract path variables, nest routers, compose services, and safely propagate errors. Follow along on [YouTube](https://youtube.com/@AarambhDevHub) or join the [Discord](https://discord.gg/HDth6PfCnp) to track progress.

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
curl http://localhost:8080/
# => {"status":"healthy","framework":"Ajaya","version":"0.1.x"}

curl http://localhost:8080/users/42
# => {"id":"42","name":"User from path param"}

curl http://localhost:8080/not-a-route
# => 404 Not Found
```

---

## Features (v0.1.6)

### ✅ Powerful Routing System

Zero-allocation request matching, dynamic path parameters, and catch-all wildcards.

```rust
use ajaya::{Router, get, serve_app, Json, PathParams, Request};
use http::StatusCode;

async fn user(req: Request) -> Json<serde_json::Value> {
    let id = req.extension::<PathParams>().and_then(|p| p.get("id")).unwrap_or("0");
    Json(serde_json::json!({ "user_id": id }))
}

#[tokio::main]
async fn main() {
    let app = Router::new()
        .route("/", get(|| async { "Home" }))
        .route("/users/:id", get(user))
        .route("/files/*path", get(|| async { "File content" }));

    serve_app("0.0.0.0:8080", app).await.unwrap();
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

### ✅ Method Dispatch

Bind different handlers to specific HTTP methods securely. Unmatched methods automatically return **405 Method Not Allowed** with an accurate `Allow` header.

```rust
let router = get(get_handler)
    .post(create_handler)
    .delete(delete_handler);
```

### ✅ Error Handling

Handlers can return `Result<T, Error>` and use `?` for error propagation. Errors produce secure JSON responses — internal details are never leaked.

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
| **0.1.x** | Routing System | ✅ Complete |
| 0.2.x | Extractors | 🚧 Next Up |
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