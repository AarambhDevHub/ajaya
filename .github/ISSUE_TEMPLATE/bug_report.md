---
name: Bug Report
about: Something is broken or behaving unexpectedly
title: "fix: "
labels: bug
assignees: ""
---

## Description

A clear, one-paragraph description of the bug.

## Minimal Reproduction

```rust
// The smallest possible code that demonstrates the bug.
// Remove everything unrelated.
// This must compile (or explain why it doesn't).
use arvik::{Router, get, serve_app};

#[tokio::main]
async fn main() {
    let app = Router::new()
        .route("/", get(|| async { "hello" }));
    // reproduce the bug here
    serve_app("127.0.0.1:8080", app).await.unwrap();
}
```

## Expected Behavior

What you expected to happen.

## Actual Behavior

What actually happened. Include the full error message, panic output, or unexpected value.

## Environment

| | |
|---|---|
| Arvik version | `0.9.x` |
| Rust version | `rustc --version` output |
| OS | e.g. Pop OS 22.04 / macOS 14 / Windows 11 |
| Active features | e.g. `default`, `tls`, `ws`, `config`, `macros` |

## Additional Context

Anything else that might be relevant — related issues, workarounds you've tried, etc.
