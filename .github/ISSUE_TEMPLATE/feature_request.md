---
name: Feature Request
about: Suggest a new feature or API improvement
title: "feat: "
labels: enhancement
assignees: ""
---

## Use Case

What are you trying to accomplish? Describe the problem you are solving, not the solution you want.
A good use case sounds like: "I need to stream server-sent events with per-client backpressure but the current API requires Y workaround."

## Proposed API

Show what you would want to write. Code is clearer than prose.

```rust
// Example of the API you'd like to exist:
let app = Router::new()
    .route("/events", get(events_handler))
    .layer(YourNewLayer::new());
```

## Alternatives Considered

What can you do today instead? Why is it insufficient?

## Affected Crate(s)

Which crate(s) would this change? e.g. `arvik-core`, `arvik-router`, `arvik-middleware`, `arvik-hyper`, `arvik-extract`, `arvik-ws`, `arvik-sse`, `arvik-static`, `arvik-tls`, `arvik-config`, `arvik-observe`

## Additional Context

Links to prior art (axum, actix-web, tower-http…), related issues, or any other context that helps evaluate the request.
