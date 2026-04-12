# ajaya-hyper

[![Crates.io](https://img.shields.io/crates/v/ajaya-hyper.svg)](https://crates.io/crates/ajaya-hyper)
[![Docs.rs](https://docs.rs/ajaya-hyper/badge.svg)](https://docs.rs/ajaya-hyper)
[![License](https://img.shields.io/badge/license-MIT%2FApache--2.0-blue.svg)](../LICENSE-MIT)

**Hyper 1.x server integration for the Ajaya web framework.**

This crate provides the TCP listener and HTTP connection handling powered by Tokio and Hyper.

## Usage

```rust
use ajaya_hyper::serve_router;
use ajaya_router::get;

#[tokio::main]
async fn main() {
    let router = get(|| async { "Hello World" });
    serve_router("0.0.0.0:8080", router).await.unwrap();
}
```

## API

| Item | Description |
|------|-------------|
| `Server::bind(addr)` | Bind a TCP listener to the given address |
| `Server::serve(handler)` | Start serving single handler on all connections |
| `Server::serve_method_router(router)` | Start serving HTTP method-matched routing |
| `serve_router(addr, router)` | Convenience one-liner for routing |

## Features

- Automatic conversion from raw hyper `Incoming` payloads into Ajaya `Request<Body>`.
- Hyper 1.x with `hyper-util` auto connection builder
- Per-connection Tokio task spawning
- HTTP/1.1 and HTTP/2 support via ALPN
- Tracing integration for connection logging

## License

Licensed under either of [Apache License, Version 2.0](../LICENSE-APACHE) or [MIT License](../LICENSE-MIT) at your option.
