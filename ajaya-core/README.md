# ajaya-core

[![Crates.io](https://img.shields.io/crates/v/ajaya-core.svg)](https://crates.io/crates/ajaya-core)
[![Docs.rs](https://docs.rs/ajaya-core/badge.svg)](https://docs.rs/ajaya-core)
[![License](https://img.shields.io/badge/license-MIT%2FApache--2.0-blue.svg)](../LICENSE-MIT)

**Core traits and types for the Ajaya web framework.**

This crate provides the foundational abstractions that all other Ajaya crates build upon.

## Types

| Type | Description |
|------|-------------|
| `Request<B>` | HTTP request wrapper around `http::Request` with extensions |
| `Response<B>` | Type alias for `http::Response` with Ajaya's `Body` |
| `Body` | Unified HTTP body type |
| `Error` | Framework error with status code and public message |

## Status

**v0.0.1** — Stub implementations. Full types coming in v0.0.2.

## License

Licensed under either of [Apache License, Version 2.0](../LICENSE-APACHE) or [MIT License](../LICENSE-MIT) at your option.
