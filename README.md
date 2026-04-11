# Ajaya (अजय) — The Unconquerable Rust Web Framework

<div align="center">

**🔱 Built on Tokio + Hyper. Engineered for maximum performance.**

[![Rust](https://img.shields.io/badge/rust-1.85%2B-orange.svg)](https://www.rust-lang.org)
[![License](https://img.shields.io/badge/license-MIT%2FApache--2.0-blue.svg)](LICENSE-MIT)
[![Version](https://img.shields.io/badge/version-0.0.1-green.svg)](CHANGELOG.md)
[![Discord](https://img.shields.io/discord/placeholder?label=discord&logo=discord&logoColor=white)](https://discord.gg/)

</div>

---

## What is Ajaya?

**Ajaya** (अजय, *"The Unconquerable"*) is a high-performance Rust web framework built from the ground up on **Tokio** and **Hyper 1.x**. It aims to unify the best features of Axum and Actix-web under one ergonomic, blazing-fast API.

> ⚠️ **v0.0.1 — Workspace Bootstrap.** This is the very first milestone. The framework compiles and runs a basic HTTP server. All advanced features are coming in subsequent versions — follow along on [YouTube](https://youtube.com/@AarambhDevHub) or join the [Discord](https://discord.gg/) to track progress.

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
# => Hello from Ajaya
```

---

## Workspace Structure

```
ajaya/
├── ajaya/              # Facade crate (re-exports everything)
├── ajaya-core/         # Core traits: Request, Response, Body, Error
├── ajaya-router/       # Radix trie router (coming in v0.1.x)
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
| **0.0.x** | Foundation & Core | 🚧 In Progress |
| 0.1.x | Routing System | ⏳ Planned |
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

Ajaya is being built in public from `0.0.1`. Contributions are welcome at every stage.

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
| 💬 Discord | [Aarambh Dev Hub](https://discord.gg/) | Questions, discussion, dev updates |
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