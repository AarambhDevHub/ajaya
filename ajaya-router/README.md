# ajaya-router

[![Crates.io](https://img.shields.io/crates/v/ajaya-router.svg)](https://crates.io/crates/ajaya-router)
[![Docs.rs](https://docs.rs/ajaya-router/badge.svg)](https://docs.rs/ajaya-router)
[![License](https://img.shields.io/badge/license-MIT%2FApache--2.0-blue.svg)](../LICENSE-MIT)

**Radix trie router for the Ajaya web framework.**

This crate will provide zero-allocation route matching with path parameters and wildcards.

## Planned Features (v0.1.x)

- `Router<S>` with `.route()`, `.nest()`, `.merge()`, `.fallback()`
- `MethodRouter` for HTTP method dispatch (`get()`, `post()`, etc.)
- Radix trie with zero heap allocation on lookup
- Path parameters (`:id`) and wildcards (`*path`)
- Route conflict detection at startup
- Nested router composition

## Status

**v0.0.1** — Stub. Implementation coming in v0.1.x.

## License

Licensed under either of [Apache License, Version 2.0](../LICENSE-APACHE) or [MIT License](../LICENSE-MIT) at your option.
