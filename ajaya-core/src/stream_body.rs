//! Streaming response body.
//!
//! [`StreamBody`] wraps any `Stream<Item = Result<Bytes, E>>` and
//! implements [`IntoResponse`], enabling zero-copy streaming without
//! buffering the full payload in memory.
//!
//! # Examples
//!
//! ```rust,ignore
//! use ajaya::StreamBody;
//! use bytes::Bytes;
//! use futures_util::stream;
//!
//! async fn large_file() -> StreamBody<impl Stream<Item = Result<Bytes, std::io::Error>>> {
//!     let chunks = tokio_util::io::ReaderStream::new(tokio::fs::File::open("big.bin").await.unwrap());
//!     StreamBody::new(chunks)
//! }
//! ```

use std::pin::Pin;
use std::task::{Context, Poll};

use bytes::Bytes;
use futures_util::Stream;
use http_body::Frame;

use crate::Body;
use crate::body::BoxError;
use crate::into_response::IntoResponse;
use crate::response::{Response, ResponseBuilder};

// ---------------------------------------------------------------------------
// StreamBody — public API
// ---------------------------------------------------------------------------

/// A response body backed by a [`Stream`]`<Item = Result<Bytes, E>>`.
///
/// Use this when you need to stream large responses without buffering
/// them in memory first. Transfer-Encoding is handled automatically
/// by Hyper.
///
/// The stream type `S` must be `Unpin`. For non-Unpin streams use
/// `Box::pin(stream)` before constructing.
pub struct StreamBody<S>(S);

impl<S> StreamBody<S> {
    /// Wrap a stream as a response body.
    pub fn new(stream: S) -> Self {
        Self(stream)
    }

    /// Unwrap the inner stream.
    pub fn into_inner(self) -> S {
        self.0
    }
}

impl<S, E> http_body::Body for StreamBody<S>
where
    S: Stream<Item = Result<Bytes, E>> + Unpin,
    E: Into<BoxError>,
{
    type Data = Bytes;
    type Error = BoxError;

    fn poll_frame(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<Option<Result<Frame<Self::Data>, Self::Error>>> {
        match Pin::new(&mut self.0).poll_next(cx) {
            Poll::Ready(Some(Ok(bytes))) => Poll::Ready(Some(Ok(Frame::data(bytes)))),
            Poll::Ready(Some(Err(e))) => Poll::Ready(Some(Err(e.into()))),
            Poll::Ready(None) => Poll::Ready(None),
            Poll::Pending => Poll::Pending,
        }
    }
}

impl<S, E> IntoResponse for StreamBody<S>
where
    S: Stream<Item = Result<Bytes, E>> + Send + Unpin + 'static,
    E: Into<BoxError>,
{
    fn into_response(self) -> Response {
        ResponseBuilder::new().body(Body::new(self))
    }
}

impl<S: std::fmt::Debug> std::fmt::Debug for StreamBody<S> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_tuple("StreamBody").field(&self.0).finish()
    }
}
