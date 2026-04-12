# ajaya-ws

[![Crates.io](https://img.shields.io/crates/v/ajaya-ws.svg)](https://crates.io/crates/ajaya-ws)
[![Docs.rs](https://docs.rs/ajaya-ws/badge.svg)](https://docs.rs/ajaya-ws)
[![License](https://img.shields.io/badge/license-MIT%2FApache--2.0-blue.svg)](../LICENSE-MIT)

**WebSocket support for the Ajaya web framework.**

This crate will provide WebSocket upgrade handling and bidirectional messaging.

## Planned Features (v0.5.x)

- `WebSocketUpgrade` extractor for HTTP upgrade
- `WebSocket` stream with `.send()` and `.recv()`
- `Message` variants: Text, Binary, Ping, Pong, Close
- Split socket for concurrent send/recv
- Configurable max message/frame size
- Subprotocol negotiation

## Status

**v0.0.5** — Stub. Implementation coming in v0.5.x.

## License

Licensed under either of [Apache License, Version 2.0](../LICENSE-APACHE) or [MIT License](../LICENSE-MIT) at your option.
