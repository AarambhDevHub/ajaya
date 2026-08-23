//! # arvik-ws
//!
//! WebSocket support for the Arvik web framework.
//!
//! ## Quick start
//!
//! ```rust,ignore
//! use arvik::ws::{WebSocket, WebSocketUpgrade, Message};
//! use arvik::{Router, get};
//!
//! async fn ws_echo(ws: WebSocketUpgrade) -> impl arvik::IntoResponse {
//!     ws.on_upgrade(|mut socket| async move {
//!         while let Some(Ok(msg)) = socket.recv().await {
//!             match msg {
//!                 Message::Text(_) | Message::Binary(_) => {
//!                     socket.send(msg).await.ok();
//!                 }
//!                 _ => break,
//!             }
//!         }
//!     })
//! }
//!
//! let app = Router::new().route("/ws", get(ws_echo));
//! ```
//!
//! ## Concurrent send + receive
//!
//! ```rust,ignore
//! use arvik::ws::{WebSocket, Message};
//!
//! async fn handle(socket: WebSocket) {
//!     let (mut sender, mut receiver) = socket.split();
//!
//!     let s = tokio::spawn(async move {
//!         sender.send(Message::Text("hello".into())).await.ok();
//!     });
//!
//!     while let Some(Ok(msg)) = receiver.next().await {
//!         println!("got: {msg:?}");
//!     }
//!     s.await.ok();
//! }
//! ```
//!
//! ## Configuration
//!
//! ```rust,ignore
//! ws.protocols(["chat", "json"])        // subprotocol negotiation
//!   .max_message_size(64 * 1024)        // reject oversized messages
//!   .max_frame_size(16 * 1024)
//!   .on_upgrade(handler)
//! ```

// `tungstenite::Error` is this crate's public error contract, returned by
// value from every socket operation. It exceeds clippy's result_large_err
// threshold on newer toolchains; boxing it would allocate on each call and
// break the API. The large-variant cases are rare runtime failures.
#![allow(clippy::result_large_err)]

pub mod message;
pub mod rejection;
pub mod socket;
pub mod split;
pub mod upgrade;

// ── Public API re-exports ─────────────────────────────────────────────────────

pub use message::{CloseCode, CloseFrame, Message};
pub use rejection::WebSocketUpgradeRejection;
pub use socket::WebSocket;
pub use split::{Receiver, Sender, WsError};
pub use upgrade::{WebSocketConfig, WebSocketUpgrade};
