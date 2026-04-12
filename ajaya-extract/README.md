# ajaya-extract

[![Crates.io](https://img.shields.io/crates/v/ajaya-extract.svg)](https://crates.io/crates/ajaya-extract)
[![Docs.rs](https://docs.rs/ajaya-extract/badge.svg)](https://docs.rs/ajaya-extract)
[![License](https://img.shields.io/badge/license-MIT%2FApache--2.0-blue.svg)](../LICENSE-MIT)

**Request extractors for the Ajaya web framework.**

This crate will provide type-safe extraction of data from HTTP requests.

## Planned Features (v0.2.x)

- `FromRequestParts` and `FromRequest` traits
- `Path<T>` — URL path parameters via serde
- `Query<T>` — Query string parameters
- `Json<T>` — JSON request body
- `Form<T>` — URL-encoded form body
- `State<S>` — Shared application state
- `TypedHeader<T>` — Typed header access
- `Multipart` — File upload handling
- `Option<T>` and `Result<T, E>` wrappers

## Status

**v0.0.5** — Stub. Implementation coming in v0.2.x.

## License

Licensed under either of [Apache License, Version 2.0](../LICENSE-APACHE) or [MIT License](../LICENSE-MIT) at your option.
