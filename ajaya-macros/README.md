# ajaya-macros

[![Crates.io](https://img.shields.io/crates/v/ajaya-macros.svg)](https://crates.io/crates/ajaya-macros)
[![Docs.rs](https://docs.rs/ajaya-macros/badge.svg)](https://docs.rs/ajaya-macros)
[![License](https://img.shields.io/badge/license-MIT%2FApache--2.0-blue.svg)](../LICENSE-MIT)

**Procedural macros for the Ajaya web framework.**

This crate will provide proc macros for better developer experience and compile-time diagnostics.

## Planned Features (v0.7.x)

- `#[debug_handler]` — Dramatically better compile error messages
- `#[route(GET, "/path")]` — Attribute-based routing
- `#[get("/path")]`, `#[post("/path")]` — Method shorthand macros
- `#[handler]` — Implement `Handler` trait for structs
- `collect_routes![]` — Gather annotated handlers

## Status

**v0.0.1** — Stub. Implementation coming in v0.7.x.

## License

Licensed under either of [Apache License, Version 2.0](../LICENSE-APACHE) or [MIT License](../LICENSE-MIT) at your option.
