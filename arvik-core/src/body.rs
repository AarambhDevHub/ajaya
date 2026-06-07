//! Unified HTTP body type.
//!
//! Provides a [`Body`] type that wraps a boxed bytes stream,
//! suitable for both requests and responses.
//!
//! # Key Types
//!
//! - [`Body`] — the unified body type used throughout Arvik
//!
//! # Examples
//!
//! ```rust
//! use arvik_core::Body;
//! use bytes::Bytes;
//!
//! // Create from bytes
//! let body = Body::from_bytes(Bytes::from("Hello"));
//!
//! // Create empty
//! let body = Body::empty();
//!
//! // Create from string
//! let body = Body::from("Hello, Arvik!");
//! ```

use std::collections::VecDeque;
use std::convert::Infallible;
use std::pin::Pin;
use std::task::{Context, Poll};

use bytes::Bytes;
use http_body::Frame;
use http_body_util::{BodyExt, Empty, Full};

/// Type alias for boxed errors used in body streams.
pub type BoxError = Box<dyn std::error::Error + Send + Sync>;

/// Arvik's unified HTTP body type.
///
/// Wraps an opaque, type-erased body stream that yields [`Bytes`] frames.
/// This type is used for both request and response bodies throughout
/// the framework.
///
/// `Body` implements [`http_body::Body`] so it integrates seamlessly
/// with Hyper and Tower.
pub struct Body {
    kind: BodyKind,
}

enum BodyKind {
    Empty(Empty<Bytes>),
    Full(Full<Bytes>),
    Chunks(ChunksBody),
    Boxed(Pin<Box<dyn http_body::Body<Data = Bytes, Error = BoxError> + Send + 'static>>),
}

impl Body {
    /// Create a new `Body` from any type implementing `http_body::Body`.
    pub fn new<B>(body: B) -> Self
    where
        B: http_body::Body<Data = Bytes> + Send + Unpin + 'static,
        B::Error: Into<BoxError>,
    {
        Self {
            kind: BodyKind::Boxed(Box::pin(MapErrorBody(body))),
        }
    }

    /// Create an empty body (zero bytes).
    pub fn empty() -> Self {
        Self {
            kind: BodyKind::Empty(Empty::<Bytes>::new()),
        }
    }

    /// Create a body from raw bytes.
    pub fn from_bytes(b: Bytes) -> Self {
        Self {
            kind: BodyKind::Full(Full::new(b)),
        }
    }

    /// Create a body from static bytes without copying.
    pub fn from_static(bytes: &'static [u8]) -> Self {
        Self::from_bytes(Bytes::from_static(bytes))
    }

    /// Create a body from multiple byte chunks without concatenating them.
    pub fn from_chunks<I, B>(chunks: I) -> Self
    where
        I: IntoIterator<Item = B>,
        B: Into<Bytes>,
    {
        let chunks: VecDeque<Bytes> = chunks
            .into_iter()
            .map(Into::into)
            .filter(|chunk| !chunk.is_empty())
            .collect();

        if chunks.is_empty() {
            return Self::empty();
        }

        let remaining = chunks.iter().map(Bytes::len).sum();
        Self {
            kind: BodyKind::Chunks(ChunksBody { chunks, remaining }),
        }
    }

    /// Return true when the body's size hint proves it is empty.
    pub fn is_empty_hint(&self) -> bool {
        let hint = http_body::Body::size_hint(self);
        hint.lower() == 0 && hint.upper() == Some(0)
    }

    /// Return the exact body size when the body size hint proves one.
    pub fn exact_size_hint(&self) -> Option<u64> {
        let hint = http_body::Body::size_hint(self);
        hint.upper().filter(|upper| *upper == hint.lower())
    }

    /// Collect the entire body into [`Bytes`].
    ///
    /// This consumes the body stream and buffers all data in memory.
    pub async fn to_bytes(self) -> Result<Bytes, BoxError> {
        let collected = BodyExt::collect(self).await?;
        Ok(collected.to_bytes())
    }

    /// Collect the entire body into a UTF-8 [`String`].
    ///
    /// Returns an error if the body is not valid UTF-8 or if
    /// reading the stream fails.
    pub async fn to_string(self) -> Result<String, BoxError> {
        let bytes = self.to_bytes().await?;
        String::from_utf8(bytes.to_vec()).map_err(|e| Box::new(e) as BoxError)
    }

    /// Create a `Body` from a `Stream<Item = Result<Bytes, E>>`.
    ///
    /// Useful for streaming large responses without buffering them in memory.
    /// The stream must be `Unpin`; wrap non-Unpin streams with `Box::pin`.
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// use bytes::Bytes;
    /// use futures_util::stream;
    ///
    /// let chunks = stream::iter(vec![
    ///     Ok::<_, std::io::Error>(Bytes::from("Hello ")),
    ///     Ok(Bytes::from("world!")),
    /// ]);
    /// let body = Body::from_stream(chunks);
    /// ```
    pub fn from_stream<S, E>(stream: S) -> Self
    where
        S: futures_util::Stream<Item = Result<Bytes, E>> + Send + Unpin + 'static,
        E: Into<BoxError>,
    {
        Self::new(StreamBodyInner { stream })
    }
}

impl http_body::Body for Body {
    type Data = Bytes;
    type Error = BoxError;

    fn poll_frame(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<Option<Result<Frame<Self::Data>, Self::Error>>> {
        match &mut self.kind {
            BodyKind::Empty(body) => poll_infallible(Pin::new(body).poll_frame(cx)),
            BodyKind::Full(body) => poll_infallible(Pin::new(body).poll_frame(cx)),
            BodyKind::Chunks(body) => Pin::new(body).poll_frame(cx),
            BodyKind::Boxed(body) => body.as_mut().poll_frame(cx),
        }
    }

    fn is_end_stream(&self) -> bool {
        match &self.kind {
            BodyKind::Empty(body) => body.is_end_stream(),
            BodyKind::Full(body) => body.is_end_stream(),
            BodyKind::Chunks(body) => body.is_end_stream(),
            BodyKind::Boxed(body) => body.is_end_stream(),
        }
    }

    fn size_hint(&self) -> http_body::SizeHint {
        match &self.kind {
            BodyKind::Empty(body) => body.size_hint(),
            BodyKind::Full(body) => body.size_hint(),
            BodyKind::Chunks(body) => body.size_hint(),
            BodyKind::Boxed(body) => body.size_hint(),
        }
    }
}

impl Default for Body {
    fn default() -> Self {
        Self::empty()
    }
}

impl std::fmt::Debug for Body {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Body").finish()
    }
}

/// Internal adapter: wraps a stream as an `http_body::Body`.
struct StreamBodyInner<S> {
    stream: S,
}

struct ChunksBody {
    chunks: VecDeque<Bytes>,
    remaining: usize,
}

impl http_body::Body for ChunksBody {
    type Data = Bytes;
    type Error = BoxError;

    fn poll_frame(
        mut self: Pin<&mut Self>,
        _cx: &mut Context<'_>,
    ) -> Poll<Option<Result<Frame<Self::Data>, Self::Error>>> {
        let Some(chunk) = self.chunks.pop_front() else {
            return Poll::Ready(None);
        };
        self.remaining = self.remaining.saturating_sub(chunk.len());
        Poll::Ready(Some(Ok(Frame::data(chunk))))
    }

    fn is_end_stream(&self) -> bool {
        self.chunks.is_empty()
    }

    fn size_hint(&self) -> http_body::SizeHint {
        let mut hint = http_body::SizeHint::new();
        hint.set_exact(self.remaining as u64);
        hint
    }
}

impl<S, E> http_body::Body for StreamBodyInner<S>
where
    S: futures_util::Stream<Item = Result<Bytes, E>> + Unpin,
    E: Into<BoxError>,
{
    type Data = Bytes;
    type Error = BoxError;

    fn poll_frame(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Option<Result<http_body::Frame<Self::Data>, Self::Error>>> {
        match std::pin::Pin::new(&mut self.stream).poll_next(cx) {
            std::task::Poll::Ready(Some(Ok(bytes))) => {
                std::task::Poll::Ready(Some(Ok(http_body::Frame::data(bytes))))
            }
            std::task::Poll::Ready(Some(Err(e))) => std::task::Poll::Ready(Some(Err(e.into()))),
            std::task::Poll::Ready(None) => std::task::Poll::Ready(None),
            std::task::Poll::Pending => std::task::Poll::Pending,
        }
    }
}

// --- From conversions ---

impl From<()> for Body {
    fn from((): ()) -> Self {
        Self::empty()
    }
}

impl From<String> for Body {
    fn from(s: String) -> Self {
        Self::from_bytes(Bytes::from(s))
    }
}

impl From<&'static str> for Body {
    fn from(s: &'static str) -> Self {
        Self::from_bytes(Bytes::from(s))
    }
}

impl From<Bytes> for Body {
    fn from(b: Bytes) -> Self {
        Self::from_bytes(b)
    }
}

impl From<Vec<u8>> for Body {
    fn from(v: Vec<u8>) -> Self {
        Self::from_bytes(Bytes::from(v))
    }
}

impl From<Full<Bytes>> for Body {
    fn from(full: Full<Bytes>) -> Self {
        Self {
            kind: BodyKind::Full(full),
        }
    }
}

fn poll_infallible(
    poll: Poll<Option<Result<Frame<Bytes>, Infallible>>>,
) -> Poll<Option<Result<Frame<Bytes>, BoxError>>> {
    match poll {
        Poll::Ready(Some(Ok(frame))) => Poll::Ready(Some(Ok(frame))),
        Poll::Ready(Some(Err(err))) => match err {},
        Poll::Ready(None) => Poll::Ready(None),
        Poll::Pending => Poll::Pending,
    }
}

// --- Internal helper to map body error types ---

/// Wrapper that maps any body's error type to [`BoxError`].
struct MapErrorBody<B>(B);

impl<B> http_body::Body for MapErrorBody<B>
where
    B: http_body::Body<Data = Bytes> + Unpin,
    B::Error: Into<BoxError>,
{
    type Data = Bytes;
    type Error = BoxError;

    fn poll_frame(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<Option<Result<Frame<Self::Data>, Self::Error>>> {
        // Safety: MapErrorBody is Unpin because B is Unpin
        let inner = Pin::new(&mut self.get_mut().0);
        match inner.poll_frame(cx) {
            Poll::Ready(Some(Ok(frame))) => Poll::Ready(Some(Ok(frame))),
            Poll::Ready(Some(Err(e))) => Poll::Ready(Some(Err(e.into()))),
            Poll::Ready(None) => Poll::Ready(None),
            Poll::Pending => Poll::Pending,
        }
    }

    fn is_end_stream(&self) -> bool {
        self.0.is_end_stream()
    }

    fn size_hint(&self) -> http_body::SizeHint {
        self.0.size_hint()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn empty_body_has_empty_hint() {
        let body = Body::empty();
        assert!(body.is_empty_hint());
        assert_eq!(body.to_bytes().await.unwrap(), Bytes::new());
    }

    #[tokio::test]
    async fn static_body_is_zero_copy_bytes() {
        let body = Body::from_static(b"hello");
        assert!(!body.is_empty_hint());
        assert_eq!(body.exact_size_hint(), Some(5));
        assert_eq!(body.to_bytes().await.unwrap(), Bytes::from_static(b"hello"));
    }

    #[tokio::test]
    async fn chunked_body_preserves_chunk_contents() {
        let body = Body::from_chunks([
            Bytes::from_static(b"hello"),
            Bytes::from_static(b" "),
            Bytes::from_static(b"world"),
        ]);

        let hint = http_body::Body::size_hint(&body);
        assert_eq!(hint.lower(), 11);
        assert_eq!(hint.upper(), Some(11));
        assert_eq!(
            body.to_bytes().await.unwrap(),
            Bytes::from_static(b"hello world")
        );
    }
}
