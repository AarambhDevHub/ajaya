//! Unified HTTP body type.
//!
//! Provides a [`Body`] type that wraps a boxed bytes stream,
//! suitable for both requests and responses.
//!
//! # Key Types
//!
//! - [`Body`] — the unified body type used throughout Ajaya
//!
//! # Examples
//!
//! ```rust
//! use ajaya_core::Body;
//! use bytes::Bytes;
//!
//! // Create from bytes
//! let body = Body::from_bytes(Bytes::from("Hello"));
//!
//! // Create empty
//! let body = Body::empty();
//!
//! // Create from string
//! let body = Body::from("Hello, Ajaya!");
//! ```

use std::pin::Pin;
use std::task::{Context, Poll};

use bytes::Bytes;
use http_body::Frame;
use http_body_util::{BodyExt, Empty, Full};

/// Type alias for boxed errors used in body streams.
pub type BoxError = Box<dyn std::error::Error + Send + Sync>;

/// Ajaya's unified HTTP body type.
///
/// Wraps an opaque, type-erased body stream that yields [`Bytes`] frames.
/// This type is used for both request and response bodies throughout
/// the framework.
///
/// `Body` implements [`http_body::Body`] so it integrates seamlessly
/// with Hyper and Tower.
pub struct Body(Pin<Box<dyn http_body::Body<Data = Bytes, Error = BoxError> + Send + 'static>>);

impl Body {
    /// Create a new `Body` from any type implementing `http_body::Body`.
    pub fn new<B>(body: B) -> Self
    where
        B: http_body::Body<Data = Bytes> + Send + Unpin + 'static,
        B::Error: Into<BoxError>,
    {
        Self(Box::pin(MapErrorBody(body)))
    }

    /// Create an empty body (zero bytes).
    pub fn empty() -> Self {
        Self::new(Empty::<Bytes>::new())
    }

    /// Create a body from raw bytes.
    pub fn from_bytes(b: Bytes) -> Self {
        Self::new(Full::new(b))
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
        Pin::new(&mut self.0).poll_frame(cx)
    }

    fn is_end_stream(&self) -> bool {
        self.0.is_end_stream()
    }

    fn size_hint(&self) -> http_body::SizeHint {
        self.0.size_hint()
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
        Self::new(full)
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
