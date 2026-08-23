//! Server-Sent Event builder.
//!
//! Construct events with chained builder methods and send them through a
//! `Stream` wrapped by [`Sse`](crate::Sse).
//!
//! # Wire format (SSE spec)
//!
//! ```text
//! id: 42
//! event: update
//! data: {"temperature": 24.5}
//! retry: 5000
//!
//! ```
//!
//! Multi-line `data` values are split across multiple `data:` lines:
//!
//! ```text
//! data: line one
//! data: line two
//!
//! ```
//!
//! Comment-only events (used for keep-alive) look like:
//!
//! ```text
//! : ping
//!
//! ```

use bytes::{Bytes, BytesMut};
use serde::Serialize;

/// A single Server-Sent Event.
///
/// All fields are optional. An event with only `data` set is the most common
/// form; `event` lets clients filter with `EventSource.addEventListener`.
///
/// # Examples
///
/// ```rust,ignore
/// use arvik::sse::Event;
/// use std::time::Duration;
///
/// // Simple data event
/// let e = Event::default().data("hello");
///
/// // Named event with ID and retry hint
/// let e = Event::default()
///     .id("42")
///     .event("temperature")
///     .data(r#"{"celsius": 24.5}"#)
///     .retry(Duration::from_secs(5));
///
/// // Keep-alive comment (no id/event/data)
/// let ping = Event::default().comment("ping");
/// ```
#[must_use]
#[derive(Debug, Clone, Default)]
pub struct Event {
    comment: Option<String>,
    id: Option<String>,
    event: Option<String>,
    data: Option<String>,
    retry: Option<u64>, // milliseconds
}

impl Event {
    /// Set the `data` field.
    ///
    /// Multi-line values (containing `\n`) are automatically split into
    /// multiple `data:` lines, as required by the SSE spec.
    ///
    /// `\r\n` and lone `\r` are normalized to `\n` (SSE spec §9.2).
    pub fn data(mut self, data: impl Into<String>) -> Self {
        let s = data.into().replace("\r\n", "\n").replace('\r', "\n");
        self.data = Some(s);
        self
    }

    /// Set the `id` field.
    ///
    /// The browser uses this to send `Last-Event-ID` on reconnect so you
    /// can resume from where the stream left off.
    ///
    /// `U+0000 NULL` bytes are stripped, and the value is truncated at the
    /// first CR/LF: an id containing `\n\n` would terminate the event framing
    /// and silently drop every field serialized after it — an injection vector
    /// whenever ids derive from untrusted input (e.g. an echoed
    /// `Last-Event-ID` header).
    pub fn id(mut self, id: impl Into<String>) -> Self {
        let s: String = id
            .into()
            .chars()
            .take_while(|&c| c != '\n' && c != '\r')
            .filter(|&c| c != '\0')
            .collect();
        self.id = Some(s);
        self
    }

    /// Set the `event` field (event type / name).
    ///
    /// Clients can filter: `source.addEventListener("update", handler)`.
    /// Defaults to `"message"` if omitted.
    ///
    /// Truncates at the first `\n` or `\r` — multi-line event names are
    /// invalid per the SSE spec and silently ignored by browsers.
    pub fn event(mut self, event: impl Into<String>) -> Self {
        let s = event.into();
        let s = s.split(['\n', '\r']).next().unwrap_or("").to_owned();
        self.event = Some(s);
        self
    }

    /// Set the `retry` reconnection interval.
    ///
    /// Tells the client how long to wait before reconnecting after the
    /// connection drops. The browser default is usually 3 seconds.
    pub fn retry(mut self, duration: std::time::Duration) -> Self {
        self.retry = Some(duration.as_millis() as u64);
        self
    }

    /// Add a comment line (prefixed with `:`).
    ///
    /// Comments are ignored by the browser but keep the TCP connection alive
    /// through proxies and load balancers. Used internally by [`KeepAlive`].
    ///
    /// `\r\n` and lone `\r` are normalized to `\n`.
    ///
    /// [`KeepAlive`]: crate::KeepAlive
    pub fn comment(mut self, comment: impl Into<String>) -> Self {
        let s = comment.into().replace("\r\n", "\n").replace('\r', "\n");
        self.comment = Some(s);
        self
    }

    /// Serialize any `T: Serialize` directly as the `data` field.
    ///
    /// Avoids the boilerplate of `serde_json::to_string(&val).unwrap()` in
    /// handler code. Returns an error if serialization fails.
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// #[derive(Serialize)]
    /// struct Tick { seq: u64, value: f64 }
    ///
    /// let event = Event::default().event("tick").json_data(&Tick { seq: 1, value: 3.14 })?;
    /// ```
    pub fn json_data<T: Serialize>(mut self, value: &T) -> Result<Self, serde_json::Error> {
        let s = serde_json::to_string(value)?;
        self.data = Some(s);
        Ok(self)
    }

    /// Serialize this event into its SSE wire representation.
    ///
    /// Pre-computes the byte length to minimise `BytesMut` reallocations.
    /// Allocation happens once per event; the returned `Bytes` is then
    /// passed through the body pipeline without further copying.
    pub fn serialize(&self) -> Bytes {
        let mut buf = BytesMut::with_capacity(self.estimate_wire_len());

        // Comment lines: ": <text>\n"
        if let Some(comment) = &self.comment {
            for line in comment.split('\n') {
                buf.extend_from_slice(b": ");
                buf.extend_from_slice(line.as_bytes());
                buf.extend_from_slice(b"\n");
            }
        }

        // id: <value>\n
        if let Some(id) = &self.id {
            buf.extend_from_slice(b"id: ");
            buf.extend_from_slice(id.as_bytes());
            buf.extend_from_slice(b"\n");
        }

        // event: <type>\n
        if let Some(event) = &self.event {
            buf.extend_from_slice(b"event: ");
            buf.extend_from_slice(event.as_bytes());
            buf.extend_from_slice(b"\n");
        }

        // data: <line>\n  (one per newline in the value)
        match &self.data {
            Some(data) if data.is_empty() => {
                // Empty string still dispatches an event
                buf.extend_from_slice(b"data: \n");
            }
            Some(data) => {
                for line in data.split('\n') {
                    buf.extend_from_slice(b"data: ");
                    buf.extend_from_slice(line.as_bytes());
                    buf.extend_from_slice(b"\n");
                }
            }
            None => {}
        }

        // retry: <ms>\n  — use itoa for zero-allocation integer formatting
        if let Some(ms) = self.retry {
            buf.extend_from_slice(b"retry: ");
            let mut tmp = itoa::Buffer::new();
            buf.extend_from_slice(tmp.format(ms).as_bytes());
            buf.extend_from_slice(b"\n");
        }

        // Terminating blank line — signals end of event to the browser
        buf.extend_from_slice(b"\n");
        buf.freeze()
    }

    /// Estimate the wire byte length so `BytesMut` can be pre-allocated.
    fn estimate_wire_len(&self) -> usize {
        let mut n = 1_usize; // final "\n"

        if let Some(c) = &self.comment {
            // ": " (2) + content + "\n" per line
            n += c.split('\n').count() * 3 + c.len();
        }
        if let Some(id) = &self.id {
            n += 4 + id.len() + 1; // "id: " + value + "\n"
        }
        if let Some(ev) = &self.event {
            n += 7 + ev.len() + 1; // "event: " + value + "\n"
        }
        if let Some(data) = &self.data {
            let lines = if data.is_empty() {
                1
            } else {
                data.split('\n').count()
            };
            // "data: " (6) + content_line + "\n" (1) per line.
            // data.len() includes the \n separators which are consumed by
            // splitting, so subtract them to avoid over-counting.
            n += lines * 7 + data.len() - (lines - 1);
        }
        if self.retry.is_some() {
            n += 7 + 20 + 1; // "retry: " + up to 20 digits (u64::MAX) + "\n"
        }

        n
    }
}
