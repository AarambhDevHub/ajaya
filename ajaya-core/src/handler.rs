//! Handler trait and implementations.
//!
//! The [`Handler`] trait is the core abstraction for request handling
//! in Ajaya. Any async function that takes extractors and returns
//! an [`IntoResponse`] type can be used as a handler.
//!
//! # Supported Handler Signatures (v0.0.3)
//!
//! ```rust,ignore
//! // Zero-argument handler
//! async fn hello() -> &'static str { "Hello!" }
//!
//! // Request-argument handler
//! async fn echo(req: Request) -> String {
//!     format!("You requested: {}", req.uri())
//! }
//! ```
//!
//! More extractor-based signatures will be supported in v0.2.x.

use std::future::Future;
use std::pin::Pin;

use crate::into_response::IntoResponse;
use crate::request::Request;
use crate::response::Response;

/// A boxed future that produces a [`Response`].
///
/// Used for type-erased handler storage in routers.
pub type BoxFuture<'a, T> = Pin<Box<dyn Future<Output = T> + Send + 'a>>;

/// The core handler trait.
///
/// Types implementing `Handler` can process HTTP requests and
/// produce responses. The type parameter `T` is a marker for
/// the handler's argument types (used for blanket impls).
/// `S` is the application state type.
///
/// # Implementors
///
/// You typically don't implement this trait directly. Instead,
/// write an async function and the blanket implementations
/// will do the rest:
///
/// ```rust,ignore
/// async fn my_handler() -> &'static str {
///     "Hello from Ajaya!"
/// }
/// ```
pub trait Handler<T, S = ()>: Clone + Send + 'static {
    /// The future returned by this handler.
    type Future: Future<Output = Response> + Send + 'static;

    /// Call this handler with the given request and state.
    fn call(self, req: Request, state: S) -> Self::Future;
}

// --- Blanket impl: async fn() -> impl IntoResponse ---

impl<F, Fut, Res, S> Handler<(), S> for F
where
    F: FnOnce() -> Fut + Clone + Send + 'static,
    Fut: Future<Output = Res> + Send + 'static,
    Res: IntoResponse,
    S: Send + 'static,
{
    type Future = Pin<Box<dyn Future<Output = Response> + Send + 'static>>;

    fn call(self, _req: Request, _state: S) -> Self::Future {
        Box::pin(async move { self().await.into_response() })
    }
}

// --- Blanket impl: async fn(Request) -> impl IntoResponse ---

/// Marker type for handlers that take a full [`Request`].
pub struct RequestMarker;

impl<F, Fut, Res, S> Handler<(RequestMarker,), S> for F
where
    F: FnOnce(Request) -> Fut + Clone + Send + 'static,
    Fut: Future<Output = Res> + Send + 'static,
    Res: IntoResponse,
    S: Send + 'static,
{
    type Future = Pin<Box<dyn Future<Output = Response> + Send + 'static>>;

    fn call(self, req: Request, _state: S) -> Self::Future {
        Box::pin(async move { self(req).await.into_response() })
    }
}

// --- Type-erased handler for dynamic dispatch ---

/// Trait object interface for type-erased handlers.
///
/// This allows storing handlers of different types in the same
/// collection (e.g., inside `MethodRouter`).
pub trait ErasedHandler<S>: Send + Sync {
    /// Clone this handler into a new box.
    fn clone_box(&self) -> Box<dyn ErasedHandler<S>>;

    /// Call this handler, returning a boxed future.
    fn call(self: Box<Self>, req: Request, state: S) -> BoxFuture<'static, Response>;
}

impl<H, T, S> ErasedHandler<S> for ErasedHandlerWrapper<H, T, S>
where
    H: Handler<T, S> + Clone + Send + Sync + 'static,
    T: 'static,
    S: Clone + Send + 'static,
{
    fn clone_box(&self) -> Box<dyn ErasedHandler<S>> {
        Box::new(self.clone())
    }

    fn call(self: Box<Self>, req: Request, state: S) -> BoxFuture<'static, Response> {
        let fut = self.handler.call(req, state);
        Box::pin(fut)
    }
}

/// Wrapper that pairs a concrete handler with its type marker.
pub struct ErasedHandlerWrapper<H, T, S> {
    handler: H,
    _marker: std::marker::PhantomData<fn(T, S)>,
}

impl<H: Clone, T, S> Clone for ErasedHandlerWrapper<H, T, S> {
    fn clone(&self) -> Self {
        Self {
            handler: self.handler.clone(),
            _marker: std::marker::PhantomData,
        }
    }
}

/// Create a type-erased handler box from a concrete handler.
pub fn into_erased<H, T, S>(handler: H) -> Box<dyn ErasedHandler<S>>
where
    H: Handler<T, S> + Clone + Send + Sync + 'static,
    T: 'static,
    S: Clone + Send + 'static,
{
    Box::new(ErasedHandlerWrapper {
        handler,
        _marker: std::marker::PhantomData,
    })
}
