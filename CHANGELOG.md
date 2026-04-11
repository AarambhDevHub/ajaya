# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

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

[Unreleased]: https://github.com/AarambhDevHub/ajaya/compare/v0.0.1...HEAD
[0.0.1]: https://github.com/AarambhDevHub/ajaya/releases/tag/v0.0.1
