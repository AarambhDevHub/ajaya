//! ## MiddlewareFn trait
//!
//! Trait implemented by every async function that can be used as middleware.
//!
//! You **do not** implement this trait manually. It is automatically implemented
//! for all async functions with signatures:
//!
//! ```text
//! async fn(T1, T2, …, Request, Next) -> impl IntoResponse
//! ```
//!
//! where every `Tₙ` implements [`FromRequestParts<S>`].
//!
//! The type parameter `S` is the router state; `T` is a tuple of the extractor
//! types used to tag the blanket impl and keep inference unambiguous.

use std::future::Future;
use std::pin::Pin;

use ajaya_core::extract::FromRequestParts;
use ajaya_core::{IntoResponse, Request, Response};

/// Trait implemented by every async function that can be used as middleware.
///
/// You **do not** implement this trait manually. It is automatically implemented
/// for all async functions with signatures:
///
/// ```text
/// async fn(T1, T2, …, Request, Next) -> impl IntoResponse
/// ```
///
/// where every `Tₙ` implements [`FromRequestParts<S>`].
///
/// The type parameter `S` is the router state; `T` is a tuple of the extractor
/// types used to tag the blanket impl and keep inference unambiguous.
pub trait MiddlewareFn<S, T>: Clone + Send + Sync + 'static {
    /// The future returned by calling this middleware.
    type Future: Future<Output = Response> + Send + 'static;

    /// Execute the middleware.
    ///
    /// Extractors are run against `req` (and `state`). On success the async
    /// function body is called. On extractor failure the rejection is
    /// converted to a response and the chain is short-circuited.
    fn call(self, req: Request, state: S, next: super::Next) -> Self::Future;
}

macro_rules! impl_middleware_fn {
    // Base case: zero extractors — async fn(Request, Next) -> Res
    ([]) => {
        impl<F, Fut, Res, S> MiddlewareFn<S, ()> for F
        where
            F: Fn(Request, super::Next) -> Fut + Clone + Send + Sync + 'static,
            Fut: Future<Output = Res> + Send + 'static,
            Res: IntoResponse,
            S: Clone + Send + Sync + 'static,
        {
            type Future = Pin<Box<dyn Future<Output = Response> + Send + 'static>>;

            fn call(self, req: Request, _state: S, next: super::Next) -> Self::Future {
                Box::pin(async move { self(req, next).await.into_response() })
            }
        }
    };

    // N extractors: async fn(T1, T2, …, Tn, Request, Next) -> Res
    ([$($ty:ident),+]) => {
        #[allow(non_snake_case, unused_mut, unused_variables)]
        impl<F, Fut, Res, S, $($ty,)+> MiddlewareFn<S, ($($ty,)+)> for F
        where
            F: Fn($($ty,)+ Request, super::Next) -> Fut + Clone + Send + Sync + 'static,
            Fut: Future<Output = Res> + Send + 'static,
            Res: IntoResponse,
            S: Clone + Send + Sync + 'static,
            $( $ty: FromRequestParts<S> + Send, )+
        {
            type Future = Pin<Box<dyn Future<Output = Response> + Send + 'static>>;

            fn call(self, req: Request, state: S, next: super::Next) -> Self::Future {
                Box::pin(async move {
                    // Split request into non-body parts (for extractors) and body.
                    let (mut parts, body) = req.into_request_parts();

                    // Run each extractor in declaration order.
                    // On the first failure return the rejection immediately.
                    $(
                        let $ty = match $ty::from_request_parts(&mut parts, &state).await {
                            Ok(v) => v,
                            Err(rejection) => return rejection.into_response(),
                        };
                    )+

                    // Reconstruct the full request so the handler still gets a body.
                    let req = Request::from_request_parts(parts, body);

                    self($($ty,)+ req, next).await.into_response()
                })
            }
        }
    };
}

// Generate MiddlewareFn impls for 0 through 16 extractors.
impl_middleware_fn!([]);
impl_middleware_fn!([T1]);
impl_middleware_fn!([T1, T2]);
impl_middleware_fn!([T1, T2, T3]);
impl_middleware_fn!([T1, T2, T3, T4]);
impl_middleware_fn!([T1, T2, T3, T4, T5]);
impl_middleware_fn!([T1, T2, T3, T4, T5, T6]);
impl_middleware_fn!([T1, T2, T3, T4, T5, T6, T7]);
impl_middleware_fn!([T1, T2, T3, T4, T5, T6, T7, T8]);
impl_middleware_fn!([T1, T2, T3, T4, T5, T6, T7, T8, T9]);
impl_middleware_fn!([T1, T2, T3, T4, T5, T6, T7, T8, T9, T10]);
impl_middleware_fn!([T1, T2, T3, T4, T5, T6, T7, T8, T9, T10, T11]);
impl_middleware_fn!([T1, T2, T3, T4, T5, T6, T7, T8, T9, T10, T11, T12]);
impl_middleware_fn!([T1, T2, T3, T4, T5, T6, T7, T8, T9, T10, T11, T12, T13]);
impl_middleware_fn!([T1, T2, T3, T4, T5, T6, T7, T8, T9, T10, T11, T12, T13, T14]);
impl_middleware_fn!([
    T1, T2, T3, T4, T5, T6, T7, T8, T9, T10, T11, T12, T13, T14, T15
]);
impl_middleware_fn!([
    T1, T2, T3, T4, T5, T6, T7, T8, T9, T10, T11, T12, T13, T14, T15, T16
]);
