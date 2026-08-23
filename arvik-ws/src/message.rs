//! WebSocket message types.
//!
//! This module provides the [`Message`] enum that represents all
//! possible WebSocket frame payloads, plus [`CloseFrame`] and
//! [`CloseCode`] for graceful connection termination.
//!
//! # Examples
//!
//! ```rust,ignore
//! use arvik::ws::{Message, CloseFrame, CloseCode};
//!
//! // Receive and respond
//! if let Some(Ok(msg)) = socket.recv().await {
//!     match msg {
//!         Message::Text(text) => {
//!             socket.send(Message::Text(format!("echo: {text}"))).await.ok();
//!         }
//!         Message::Binary(data) => {
//!             socket.send(Message::Binary(data)).await.ok();
//!         }
//!         Message::Close(frame) => {
//!             // frame contains optional code + reason
//!         }
//!         _ => {}
//!     }
//! }
//! ```

use tokio_tungstenite::tungstenite;

// ── CloseCode ────────────────────────────────────────────────────────────────

/// WebSocket close status codes (RFC 6455 §7.4.1).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CloseCode {
    /// Normal closure.
    Normal,
    /// Endpoint is going away (e.g., server shutting down).
    Away,
    /// Protocol error.
    Protocol,
    /// Unsupported data type.
    Unsupported,
    /// No status code was received.
    Status,
    /// Abnormal closure (no close frame).
    Abnormal,
    /// Non-UTF-8 data received.
    Invalid,
    /// Policy violation.
    Policy,
    /// Message too large.
    Size,
    /// Missing extension.
    Extension,
    /// Internal server error during request.
    Error,
    /// Server restarting — try reconnecting.
    Restart,
    /// Try again later.
    Again,
    /// Custom / vendor-defined close code.
    Other(u16),
}

impl From<CloseCode> for u16 {
    fn from(code: CloseCode) -> Self {
        match code {
            CloseCode::Normal => 1000,
            CloseCode::Away => 1001,
            CloseCode::Protocol => 1002,
            CloseCode::Unsupported => 1003,
            CloseCode::Status => 1005,
            CloseCode::Abnormal => 1006,
            CloseCode::Invalid => 1007,
            CloseCode::Policy => 1008,
            CloseCode::Size => 1009,
            CloseCode::Extension => 1010,
            CloseCode::Error => 1011,
            CloseCode::Restart => 1012,
            CloseCode::Again => 1013,
            CloseCode::Other(n) => n,
        }
    }
}

impl From<u16> for CloseCode {
    fn from(n: u16) -> Self {
        match n {
            1000 => CloseCode::Normal,
            1001 => CloseCode::Away,
            1002 => CloseCode::Protocol,
            1003 => CloseCode::Unsupported,
            1005 => CloseCode::Status,
            1006 => CloseCode::Abnormal,
            1007 => CloseCode::Invalid,
            1008 => CloseCode::Policy,
            1009 => CloseCode::Size,
            1010 => CloseCode::Extension,
            1011 => CloseCode::Error,
            1012 => CloseCode::Restart,
            1013 => CloseCode::Again,
            n => CloseCode::Other(n),
        }
    }
}

impl From<tungstenite::protocol::frame::coding::CloseCode> for CloseCode {
    fn from(c: tungstenite::protocol::frame::coding::CloseCode) -> Self {
        u16::from(c).into()
    }
}

impl From<CloseCode> for tungstenite::protocol::frame::coding::CloseCode {
    fn from(c: CloseCode) -> Self {
        u16::from(c).into()
    }
}

// ── CloseFrame ───────────────────────────────────────────────────────────────

/// A WebSocket close frame containing an optional code and reason string.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CloseFrame {
    /// The close status code.
    pub code: CloseCode,
    /// A human-readable close reason (may be empty).
    pub reason: String,
}

impl CloseFrame {
    /// Create a close frame with a code and reason.
    pub fn new(code: CloseCode, reason: impl Into<String>) -> Self {
        Self {
            code,
            reason: reason.into(),
        }
    }

    /// Normal close with no reason.
    pub fn normal() -> Self {
        Self::new(CloseCode::Normal, "")
    }
}

impl From<tungstenite::protocol::CloseFrame<'_>> for CloseFrame {
    fn from(f: tungstenite::protocol::CloseFrame<'_>) -> Self {
        Self {
            code: f.code.into(),
            reason: f.reason.into_owned(),
        }
    }
}

impl<'a> From<&'a CloseFrame> for tungstenite::protocol::CloseFrame<'a> {
    fn from(f: &'a CloseFrame) -> Self {
        tungstenite::protocol::CloseFrame {
            code: f.code.into(),
            reason: std::borrow::Cow::Borrowed(&f.reason),
        }
    }
}

// ── Message ──────────────────────────────────────────────────────────────────

/// A WebSocket message.
///
/// # Examples
///
/// ```rust,ignore
/// use arvik::ws::Message;
///
/// let text = Message::Text("hello".into());
/// let binary = Message::Binary(vec![0, 1, 2, 3]);
/// let ping = Message::Ping(vec![]);
/// let close = Message::Close(None); // no frame → use default close
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Message {
    /// A UTF-8 text message.
    Text(String),
    /// A binary message.
    Binary(Vec<u8>),
    /// A ping frame (server should auto-pong — see `WebSocket::recv`).
    Ping(Vec<u8>),
    /// A pong frame (response to ping).
    Pong(Vec<u8>),
    /// A close frame.
    Close(Option<CloseFrame>),
}

impl Message {
    /// Returns `true` if this is a text message.
    pub fn is_text(&self) -> bool {
        matches!(self, Self::Text(_))
    }

    /// Returns `true` if this is a binary message.
    pub fn is_binary(&self) -> bool {
        matches!(self, Self::Binary(_))
    }

    /// Returns `true` if this is a close frame.
    pub fn is_close(&self) -> bool {
        matches!(self, Self::Close(_))
    }

    /// Returns `true` if this is a ping.
    pub fn is_ping(&self) -> bool {
        matches!(self, Self::Ping(_))
    }

    /// Returns `true` if this is a pong.
    pub fn is_pong(&self) -> bool {
        matches!(self, Self::Pong(_))
    }

    /// Returns the text content if this is a [`Message::Text`].
    pub fn as_text(&self) -> Option<&str> {
        match self {
            Self::Text(t) => Some(t.as_str()),
            _ => None,
        }
    }

    /// Returns the binary content if this is a [`Message::Binary`].
    pub fn as_bytes(&self) -> Option<&[u8]> {
        match self {
            Self::Binary(b) => Some(b.as_slice()),
            _ => None,
        }
    }

    /// Total payload length in bytes.
    pub fn len(&self) -> usize {
        match self {
            Self::Text(t) => t.len(),
            Self::Binary(b) | Self::Ping(b) | Self::Pong(b) => b.len(),
            Self::Close(_) => 0,
        }
    }

    /// Returns `true` if the payload is empty.
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

impl From<tungstenite::Message> for Message {
    fn from(msg: tungstenite::Message) -> Self {
        match msg {
            tungstenite::Message::Text(t) => Self::Text(t),
            tungstenite::Message::Binary(b) => Self::Binary(b),
            tungstenite::Message::Ping(d) => Self::Ping(d),
            tungstenite::Message::Pong(d) => Self::Pong(d),
            tungstenite::Message::Close(frame) => Self::Close(frame.map(|f| CloseFrame {
                code: f.code.into(),
                reason: f.reason.into_owned(),
            })),
            tungstenite::Message::Frame(_) => unreachable!("raw frames are internal"),
        }
    }
}

impl From<Message> for tungstenite::Message {
    fn from(msg: Message) -> Self {
        match msg {
            Message::Text(t) => tungstenite::Message::Text(t),
            Message::Binary(b) => tungstenite::Message::Binary(b),
            Message::Ping(d) => tungstenite::Message::Ping(d),
            Message::Pong(d) => tungstenite::Message::Pong(d),
            Message::Close(frame) => {
                tungstenite::Message::Close(frame.map(|f| tungstenite::protocol::CloseFrame {
                    code: f.code.into(),
                    reason: std::borrow::Cow::Owned(f.reason),
                }))
            }
        }
    }
}

impl From<String> for Message {
    fn from(s: String) -> Self {
        Self::Text(s)
    }
}

impl From<&str> for Message {
    fn from(s: &str) -> Self {
        Self::Text(s.to_owned())
    }
}

impl From<Vec<u8>> for Message {
    fn from(b: Vec<u8>) -> Self {
        Self::Binary(b)
    }
}

impl From<bytes::Bytes> for Message {
    fn from(b: bytes::Bytes) -> Self {
        Self::Binary(b.to_vec())
    }
}
