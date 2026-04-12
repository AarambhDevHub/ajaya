# ajaya-sse

[![Crates.io](https://img.shields.io/crates/v/ajaya-sse.svg)](https://crates.io/crates/ajaya-sse)
[![Docs.rs](https://docs.rs/ajaya-sse/badge.svg)](https://docs.rs/ajaya-sse)
[![License](https://img.shields.io/badge/license-MIT%2FApache--2.0-blue.svg)](../LICENSE-MIT)

**Server-Sent Events (SSE) for the Ajaya web framework.**

This crate will provide one-directional event streaming to clients.

## Planned Features (v0.5.x)

- `Sse<S>` response type wrapping any `Stream`
- `Event` builder: `.data()`, `.id()`, `.event()`, `.retry()`
- `KeepAlive` to prevent connection timeouts
- Proper `Content-Type: text/event-stream` headers

## Status

**v0.0.5** — Stub. Implementation coming in v0.5.x.

## License

Licensed under either of [Apache License, Version 2.0](../LICENSE-APACHE) or [MIT License](../LICENSE-MIT) at your option.
