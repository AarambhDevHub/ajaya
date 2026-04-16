//! Handler trait and implementations.
//!
//! The [`Handler`] trait is the core abstraction for request handling
//! in Ajaya. Any async function that takes extractors and returns
//! an [`IntoResponse`] type can be used as a handler.
//!
//! # Supported Handler Signatures
//!
//! ```rust,ignore
//! // Zero-argument handler
//! async fn hello() -> &'static str { "Hello!" }
//!
//! // Request-argument handler
//! async fn echo(req: Request) -> String {
//!     format!("You requested: {}", req.uri())
//! }
//!
//! // Extractor-based handlers (up to 16 extractors)
//! async fn handler(
//!     method: Method,
//!     Path(id): Path<u32>,
//!     Json(body): Json<Payload>,
//! ) -> impl IntoResponse {
//!     // ...
//! }
//! ```

use std::future::Future;
use std::pin::Pin;

use crate::extract::{FromRequest, FromRequestParts};
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

// ---------------------------------------------------------------------------
// Blanket impl: async fn() -> impl IntoResponse (zero extractors)
// ---------------------------------------------------------------------------

impl<F, Fut, Res, S> Handler<((),), S> for F
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

// ---------------------------------------------------------------------------
// Macro-generated blanket impls for 1..16 extractors
// ---------------------------------------------------------------------------
//
// For a handler `async fn(T1, T2, ..., Tn) -> R`:
//   - T1 through T(n-1) must implement `FromRequestParts<S>`
//   - Tn (the last parameter) must implement `FromRequest<S>`
//     (which includes all `FromRequestParts` types via blanket impl)
//
// This means body-consuming extractors (Json, Form, etc.) must be the
// last parameter, while parts-only extractors (Method, Path, etc.)
// can appear in any order before it.

macro_rules! impl_handler {
    (
        [$($ty:ident),*], $last:ident
    ) => {
        #[allow(non_snake_case, unused_mut, unused_variables)]
        impl<F, Fut, Res, S, M, $($ty,)* $last> Handler<(M, $($ty,)* $last,), S> for F
        where
            F: FnOnce($($ty,)* $last) -> Fut + Clone + Send + 'static,
            Fut: Future<Output = Res> + Send + 'static,
            Res: IntoResponse,
            S: Clone + Send + Sync + 'static,
            $( $ty: FromRequestParts<S> + Send, )*
            $last: FromRequest<S, M> + Send,
        {
            type Future = Pin<Box<dyn Future<Output = Response> + Send + 'static>>;

            fn call(self, req: Request, state: S) -> Self::Future {
                Box::pin(async move {
                    let (mut parts, body) = req.into_request_parts();

                    $(
                        let $ty = match $ty::from_request_parts(&mut parts, &state).await {
                            Ok(value) => value,
                            Err(rejection) => return rejection.into_response(),
                        };
                    )*

                    let req = Request::from_request_parts(parts, body);

                    let $last = match $last::from_request(req, &state).await {
                        Ok(value) => value,
                        Err(rejection) => return rejection.into_response(),
                    };

                    self($($ty,)* $last).await.into_response()
                })
            }
        }
    };
}

// Generate implementations for 1 to 16 extractors.
// The last type is always extracted via FromRequest (body consumer).
// All preceding types are extracted via FromRequestParts.
impl_handler!([], T1);
impl_handler!([T1], T2);
impl_handler!([T1, T2], T3);
impl_handler!([T1, T2, T3], T4);
impl_handler!([T1, T2, T3, T4], T5);
impl_handler!([T1, T2, T3, T4, T5], T6);
impl_handler!([T1, T2, T3, T4, T5, T6], T7);
impl_handler!([T1, T2, T3, T4, T5, T6, T7], T8);
impl_handler!([T1, T2, T3, T4, T5, T6, T7, T8], T9);
impl_handler!([T1, T2, T3, T4, T5, T6, T7, T8, T9], T10);
impl_handler!([T1, T2, T3, T4, T5, T6, T7, T8, T9, T10], T11);
impl_handler!([T1, T2, T3, T4, T5, T6, T7, T8, T9, T10, T11], T12);
impl_handler!([T1, T2, T3, T4, T5, T6, T7, T8, T9, T10, T11, T12], T13);
impl_handler!(
    [T1, T2, T3, T4, T5, T6, T7, T8, T9, T10, T11, T12, T13],
    T14
);
impl_handler!(
    [T1, T2, T3, T4, T5, T6, T7, T8, T9, T10, T11, T12, T13, T14],
    T15
);
impl_handler!(
    [
        T1, T2, T3, T4, T5, T6, T7, T8, T9, T10, T11, T12, T13, T14, T15
    ],
    T16
);

// ---------------------------------------------------------------------------
// Type-erased handler for dynamic dispatch
// ---------------------------------------------------------------------------

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
