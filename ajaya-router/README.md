# ajaya-router

[![Crates.io](https://img.shields.io/crates/v/ajaya-router.svg)](https://crates.io/crates/ajaya-router)
[![Docs.rs](https://docs.rs/ajaya-router/badge.svg)](https://docs.rs/ajaya-router)
[![License](https://img.shields.io/badge/license-MIT%2FApache--2.0-blue.svg)](../LICENSE-MIT)

**Radix trie router system and HTTP method dispatch for the Ajaya web framework.**

## Features

- **Radix Trie Engine**: Zero-allocation route matching via `matchit`
- **Path Routing**: `Router<S>` with `.route()`, `.nest()`, `.merge()`, `.fallback()`
- **Dynamic Path Parameters**: Extract `:id` and catch-all `*wildcard` segments seamlessly via `PathParams`
- **Prefix Nesting**: Mount sub-routers easily for scalable architectures
- **Tower Integration**: Embed raw Tower `Service`s using `.route_service()` and `.nest_service()`
- **Method Routing**: Custom `MethodFilter` bitflag engine for binding `GET`, `POST`, `DELETE`, etc.
- **Fail-Safe Startup**: Strict 405 Method Not Allowed handling and startup cycle checking to prevent conflicting route registration.

## Status

**v0.1.6** — Complete Routing System implemented. Path extraction, Trie-routing, Wildcards, and Tower Service routing functionality is stable.

## License

Licensed under either of [Apache License, Version 2.0](../LICENSE-APACHE) or [MIT License](../LICENSE-MIT) at your option.
