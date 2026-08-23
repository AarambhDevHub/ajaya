//! Split WebSocket halves for concurrent send and receive.
//!
//! Obtained by calling [`WebSocket::split`](crate::WebSocket::split).
//!
//! # Examples
//!
//! ```rust,ignore
//! use arvik::ws::{Message, WebSocket};
//!
//! async fn handle(socket: WebSocket) {
//!     let (mut sender, mut receiver) = socket.split();
//!
//!     // Concurrent tasks
//!     let send_task = tokio::spawn(async move {
//!         sender.send(Message::Text("hello".into())).await.ok();
//!     });
//!
//!     let recv_task = tokio::spawn(async move {
//!         while let Some(Ok(msg)) = receiver.next().await {
//!             println!("Received: {:?}", msg);
//!         }
//!     });
//!
//!     tokio::try_join!(send_task, recv_task).ok();
//! }
//! ```

use futures_util::stream::SplitSink;
use futures_util::stream::SplitStream;
use futures_util::{SinkExt, StreamExt};
use tokio_tungstenite::WebSocketStream;
use tokio_tungstenite::tungstenite;

use crate::message::Message;

pub(crate) type WsStream<S> = WebSocketStream<S>;
pub(crate) type RawSink<S> = SplitSink<WsStream<S>, tungstenite::Message>;
pub(crate) type RawStream<S> = SplitStream<WsStream<S>>;

// ── WebSocket error ───────────────────────────────────────────────────────────

/// Error returned by WebSocket send/receive operations.
pub type WsError = tungstenite::Error;

// ── Sender ───────────────────────────────────────────────────────────────────

/// The sending half of a split [`WebSocket`](crate::WebSocket).
///
/// Obtained via [`WebSocket::split`](crate::WebSocket::split).
/// Multiple `Sender`s cannot be created — `split()` moves the socket.
pub struct Sender<S> {
    pub(crate) inner: RawSink<S>,
}

impl<S> Sender<S>
where
    S: tokio::io::AsyncRead + tokio::io::AsyncWrite + Unpin,
{
    /// Send a message to the remote peer.
    ///
    /// Flushes after each send for lowest latency. For high-throughput
    /// scenarios consider buffering and calling [`flush`](Self::flush) manually.
    #[inline]
    pub async fn send(&mut self, msg: Message) -> Result<(), WsError> {
        self.inner.send(msg.into()).await
    }

    /// Flush any buffered messages.
    #[inline]
    pub async fn flush(&mut self) -> Result<(), WsError> {
        SinkExt::flush(&mut self.inner).await
    }

    /// Send a close frame and flush.
    pub async fn close(&mut self, msg: Option<crate::message::CloseFrame>) -> Result<(), WsError> {
        self.inner.send(Message::Close(msg).into()).await
    }

    /// Send multiple messages in a batch before flushing once.
    ///
    /// More efficient than calling `send` repeatedly for high-throughput paths.
    pub async fn send_batch(
        &mut self,
        msgs: impl IntoIterator<Item = Message>,
    ) -> Result<(), WsError> {
        for msg in msgs {
            SinkExt::feed(&mut self.inner, msg.into()).await?;
        }
        SinkExt::flush(&mut self.inner).await
    }
}

impl<S> std::fmt::Debug for Sender<S> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Sender").finish_non_exhaustive()
    }
}

// ── Receiver ─────────────────────────────────────────────────────────────────

/// The receiving half of a split [`WebSocket`](crate::WebSocket).
///
/// `Ping` frames received here are surfaced as [`Message::Ping`] so that
/// your receive task can forward them to the `Sender` for an explicit pong.
/// Unlike `WebSocket::recv`, the split `Receiver` does **not** auto-pong
/// (there is no access to the sink half to do so). Handle them manually:
///
/// ```rust,ignore
/// while let Some(Ok(msg)) = receiver.next().await {
///     if let Message::Ping(data) = msg {
///         sender.send(Message::Pong(data)).await.ok();
///     }
/// }
/// ```
pub struct Receiver<S> {
    pub(crate) inner: RawStream<S>,
}

impl<S> Receiver<S>
where
    S: tokio::io::AsyncRead + tokio::io::AsyncWrite + Unpin,
{
    /// Receive the next message.
    ///
    /// Returns `None` when the connection is closed.
    /// Returns `Some(Err(e))` on protocol or IO errors.
    ///
    /// # Note
    /// `tungstenite::Error` is a large type (136 bytes). We allow this lint
    /// because it is an external type we cannot shrink.
    #[inline]
    #[allow(clippy::result_large_err)]
    pub async fn next(&mut self) -> Option<Result<Message, WsError>> {
        self.inner.next().await.map(|r| r.map(Message::from))
    }
}

impl<S> std::fmt::Debug for Receiver<S> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Receiver").finish_non_exhaustive()
    }
}

// Implement Stream for Receiver so it works with `while let Some(msg) = receiver.next()`
// AND with tokio_stream / futures_util combinators.
impl<S> futures_util::Stream for Receiver<S>
where
    S: tokio::io::AsyncRead + tokio::io::AsyncWrite + Unpin,
{
    type Item = Result<Message, WsError>;

    // `tungstenite::Error` is 136 bytes (external type — cannot be reduced).
    #[allow(clippy::result_large_err)]
    fn poll_next(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Option<Self::Item>> {
        use std::pin::Pin;
        Pin::new(&mut self.inner)
            .poll_next(cx)
            .map(|opt| opt.map(|r| r.map(Message::from)))
    }
}
