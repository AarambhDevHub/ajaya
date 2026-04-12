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
| `Body` | Unified HTTP body type with `http_body::Body` support |
| `Error` | Framework error producing JSON responses securely |
| `ResponseBuilder` | Ergonomic fluent API for building typed responses |
| `Handler<T,S>` | Trait defining an async request handler |
| `IntoResponse` | Conversion trait turning handlers' return types into HTTP Responses |

## Status

**v0.1.6** — Core implementations are integrated and stable against the v0.1.x routing system.

## License

Licensed under either of [Apache License, Version 2.0](../LICENSE-APACHE) or [MIT License](../LICENSE-MIT) at your option.
