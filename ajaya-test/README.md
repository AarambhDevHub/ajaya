# ajaya-test

[![Crates.io](https://img.shields.io/crates/v/ajaya-test.svg)](https://crates.io/crates/ajaya-test)
[![Docs.rs](https://docs.rs/ajaya-test/badge.svg)](https://docs.rs/ajaya-test)
[![License](https://img.shields.io/badge/license-MIT%2FApache--2.0-blue.svg)](../LICENSE-MIT)

**In-process testing utilities for the Ajaya web framework.**

This crate will provide a test client for handler testing without spinning up a real server.

## Planned Features (v0.7.x)

- `TestClient::new(app)` — In-memory HTTP client
- Request builders: `.get()`, `.post()`, `.put()`, `.delete()`, `.patch()`
- Request customization: `.header()`, `.json()`, `.form()`, `.body()`, `.query()`
- Response assertions: `.status()`, `.text()`, `.json::<T>()`, `.bytes()`
- WebSocket test: `client.ws("/path")` → `TestWebSocket`
- Cookie jar for stateful sessions across requests

## Status

**v0.0.5** — Stub. Implementation coming in v0.7.x.

## License

Licensed under either of [Apache License, Version 2.0](../LICENSE-APACHE) or [MIT License](../LICENSE-MIT) at your option.
