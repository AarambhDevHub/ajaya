# ajaya

[![Crates.io](https://img.shields.io/crates/v/ajaya.svg)](https://crates.io/crates/ajaya)
[![Docs.rs](https://docs.rs/ajaya/badge.svg)](https://docs.rs/ajaya)
[![License](https://img.shields.io/badge/license-MIT%2FApache--2.0-blue.svg)](../LICENSE-MIT)

**The main entry point for the Ajaya web framework.**

This is the facade crate that re-exports everything you need from the Ajaya ecosystem. Add this single dependency to your `Cargo.toml` and you're ready to go.

## Usage

```toml
[dependencies]
ajaya = "0.0.5"
```

```rust
use ajaya::{get, serve_router, Json};

async fn hello() -> Json<serde_json::Value> {
    Json(serde_json::json!({
        "message": "Hello from Ajaya!"
    }))
}

#[tokio::main]
async fn main() {
    let router = get(hello);
    serve_router("0.0.0.0:8080", router).await.unwrap();
}
```

## Re-exports

This crate re-exports from:

| Crate | What |
|-------|------|
| `ajaya-core` | `Request`, `Response`, `Body`, `Handler`, `IntoResponse`, `Json`, `Html`, `Error`, `Redirect` |
| `ajaya-hyper` | `Server`, `serve()`, `serve_router()` |
| `ajaya-router`| `MethodRouter`, `get()`, `post()`, `put()`, `delete()`, `patch()`, etc. |

More re-exports will be added as the framework grows.

## License

Licensed under either of [Apache License, Version 2.0](../LICENSE-APACHE) or [MIT License](../LICENSE-MIT) at your option.
