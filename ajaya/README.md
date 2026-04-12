# ajaya

[![Crates.io](https://img.shields.io/crates/v/ajaya.svg)](https://crates.io/crates/ajaya)
[![Docs.rs](https://docs.rs/ajaya/badge.svg)](https://docs.rs/ajaya)
[![License](https://img.shields.io/badge/license-MIT%2FApache--2.0-blue.svg)](../LICENSE-MIT)

**The main entry point for the Ajaya web framework.**

This is the facade crate that re-exports everything you need from the Ajaya ecosystem. Add this single dependency to your `Cargo.toml` and you're ready to go.

## Usage

```toml
[dependencies]
ajaya = "0.1.6"
tokio = { version = "1", features = ["full"] }
serde_json = "1.0"
```

```rust
use ajaya::{Router, get, serve_app, Json, PathParams, Request};

async fn hello() -> Json<serde_json::Value> {
    Json(serde_json::json!({
        "message": "Hello from Ajaya!"
    }))
}

async fn user(req: Request) -> String {
    let id = req.extension::<PathParams>().and_then(|p| p.get("id")).unwrap_or("unknown");
    format!("User ID: {}", id)
}

#[tokio::main]
async fn main() {
    let app = Router::new()
        .route("/", get(hello))
        .route("/users/:id", get(user));
        
    serve_app("0.0.0.0:8080", app).await.unwrap();
}
```

## Re-exports

This crate re-exports from:

| Crate | What |
|-------|------|
| `ajaya-core` | `Request`, `Response`, `Body`, `Handler`, `IntoResponse`, `Json`, `Html`, `Error`, `Redirect` |
| `ajaya-hyper` | `Server`, `serve()`, `serve_router()`, `serve_app()` |
| `ajaya-router`| `Router`, `MethodRouter`, `PathParams`, `get()`, `post()`, etc. |

More re-exports will be added as the framework grows.

## License

Licensed under either of [Apache License, Version 2.0](../LICENSE-APACHE) or [MIT License](../LICENSE-MIT) at your option.
