# ajaya-tls

[![Crates.io](https://img.shields.io/crates/v/ajaya-tls.svg)](https://crates.io/crates/ajaya-tls)
[![Docs.rs](https://docs.rs/ajaya-tls/badge.svg)](https://docs.rs/ajaya-tls)
[![License](https://img.shields.io/badge/license-MIT%2FApache--2.0-blue.svg)](../LICENSE-MIT)

**TLS / HTTPS support for the Ajaya web framework.**

This crate will provide TLS termination using rustls and native-tls backends.

## Planned Features (v0.6.x)

- `RustlsConfig` — rustls-based TLS (no OpenSSL dependency)
- `NativeTlsConfig` — native-tls backend (OpenSSL/SChannel/SecureTransport)
- PEM file and in-memory certificate loading
- Self-signed certificate generation for development
- TLS certificate hot-reload without restart
- ALPN negotiation for HTTP/2

## Status

**v0.0.5** — Stub. Implementation coming in v0.6.x.

## License

Licensed under either of [Apache License, Version 2.0](../LICENSE-APACHE) or [MIT License](../LICENSE-MIT) at your option.
