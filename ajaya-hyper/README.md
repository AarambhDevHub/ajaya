# ajaya-hyper

[![Crates.io](https://img.shields.io/crates/v/ajaya-hyper.svg)](https://crates.io/crates/ajaya-hyper)
[![Docs.rs](https://docs.rs/ajaya-hyper/badge.svg)](https://docs.rs/ajaya-hyper)
[![License](https://img.shields.io/badge/license-MIT%2FApache--2.0-blue.svg)](../LICENSE-MIT)

**Hyper 1.x server integration for the Ajaya web framework.**

This crate provides the TCP listener and HTTP connection handling powered by Tokio and Hyper.

## Usage

```rust
use ajaya_hyper::serve_app;
use ajaya_router::{Router, get};

#[tokio::main]
async fn main() {
    let app = Router::new().route("/", get(|| async { "Hello World" }));
    serve_app("0.0.0.0:8080", app).await.unwrap();
}
```

## API

| Item | Description |
|------|-------------|
| `Server::bind(addr)` | Bind a TCP listener to the given address |
| `Server::serve(handler)` | Start serving single handler on all connections |
| `Server::serve_method_router(router)` | Start serving HTTP method-matched routing |
| `Server::serve_app(router)` | Start serving full path-based routing |
| `serve_app(addr, router)` | Convenience one-liner for path routing |

## Features

- Automatic conversion from raw hyper `Incoming` payloads into Ajaya `Request<Body>`.
- Hyper 1.x with `hyper-util` auto connection builder
- Per-connection Tokio task spawning
- HTTP/1.1 and HTTP/2 support via ALPN
- Tracing integration for connection logging

## License

Licensed under either of [Apache License, Version 2.0](../LICENSE-APACHE) or [MIT License](../LICENSE-MIT) at your option.
