# ajaya-router

[![Crates.io](https://img.shields.io/crates/v/ajaya-router.svg)](https://crates.io/crates/ajaya-router)
[![Docs.rs](https://docs.rs/ajaya-router/badge.svg)](https://docs.rs/ajaya-router)
[![License](https://img.shields.io/badge/license-MIT%2FApache--2.0-blue.svg)](../LICENSE-MIT)

**Radix trie router system and HTTP method dispatch for the Ajaya web framework.**

## Features

- Custom `MethodFilter` bitflag engine
- `MethodRouter` for binding different handlers to specific HTTP methods (`GET`, `POST`, `DELETE`, etc.)
- Strict 405 Method Not Allowed handling utilizing the valid `Allow` HTTP header.
- Dynamic dispatch of type-erased `Handler`s.

## Planned Features (v0.1.x)

- `Router<S>` with `.route()`, `.nest()`, `.merge()`, `.fallback()`
- Radix trie with zero heap allocation on lookup
- Path parameters (`:id`) and wildcards (`*path`)
- Route conflict detection at startup
- Nested router composition

## Status

**v0.0.5** — Method dispatch implemented (`MethodRouter`). Path routing (radix trie) coming in v0.1.x.

## License

Licensed under either of [Apache License, Version 2.0](../LICENSE-APACHE) or [MIT License](../LICENSE-MIT) at your option.
