# ajaya-static

[![Crates.io](https://img.shields.io/crates/v/ajaya-static.svg)](https://crates.io/crates/ajaya-static)
[![Docs.rs](https://docs.rs/ajaya-static/badge.svg)](https://docs.rs/ajaya-static)
[![License](https://img.shields.io/badge/license-MIT%2FApache--2.0-blue.svg)](../LICENSE-MIT)

**Static file and directory serving for the Ajaya web framework.**

This crate will provide efficient file serving with caching and compression support.

## Planned Features (v0.6.x)

- `ServeDir` — Serve a directory tree with MIME detection
- `ServeFile` — Serve a single file
- `ETag` and `Last-Modified` headers
- Conditional GET (304 Not Modified)
- Range requests (206 Partial Content)
- Pre-compressed file support (`.gz`, `.br`)
- Directory listing (opt-in)
- Embedded files via `rust-embed` integration

## Status

**v0.0.1** — Stub. Implementation coming in v0.6.x.

## License

Licensed under either of [Apache License, Version 2.0](../LICENSE-APACHE) or [MIT License](../LICENSE-MIT) at your option.
