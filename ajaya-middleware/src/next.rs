//! ## Next
//!
//! Handle to the remaining middleware / handler chain.
//!
//! Received as the **last** parameter of every `from_fn` middleware function.
//! Call [`Next::run`] to forward the request to the inner layers and the
//! final handler.
//!
//! Choosing **not** to call `run` short-circuits the chain, which is how
//! auth guards, rate limiters, and other gatekeeping middleware work.
//!
//! `Next` is `Clone`, so you can hold it across `await` points freely.
//!
//! ### Examples
//!
//! Pass-through:
//! ```rust,ignore
//! async fn middleware(req: Request, next: Next) -> Response {
//!     next.run(req).await
//! }
//! ```
//!
//! Guard:
//! ```rust,ignore
//! async fn guard(jar: CookieJar, req: Request, next: Next) -> impl IntoResponse {
//!     if jar.get("session").is_some() {
//!         next.run(req).await
//!     } else {
//!         StatusCode::UNAUTHORIZED.into_response()
//!     }
//! }
//! ```
//!
//! Mutate request **and** response:
//! ```rust,ignore
//! async fn wrap(mut req: Request, next: Next) -> Response {
//!     req.extensions_mut().insert(Timestamp::now());
//!     let mut res = next.run(req).await;
//!     res.headers_mut().insert("x-served-by", "ajaya".parse().unwrap());
//!     res
//! }
//! ```

use std::fmt;

use ajaya_router::layer::BoxCloneService;

/// Handle to the remaining middleware / handler chain.
///
/// Received as the **last** parameter of every `from_fn` middleware function.
/// Call [`Next::run`] to forward the request to the inner layers and the
/// final handler.
///
/// Choosing **not** to call `run` short-circuits the chain, which is how
/// auth guards, rate limiters, and other gatekeeping middleware work.
///
/// `Next` is `Clone`, so you can hold it across `await` points freely.
#[derive(Clone)]
pub struct Next {
    inner: BoxCloneService,
}

impl Next {
    /// Create a `Next` from a type-erased inner service.
    ///
    /// This is used internally by the middleware machinery; you typically
    /// never construct `Next` yourself.
    #[doc(hidden)]
    #[inline]
    pub fn new(inner: BoxCloneService) -> Self {
        Self { inner }
    }

    /// Pass the request to the remaining chain and return its response.
    #[inline]
    pub async fn run(self, req: ajaya_core::Request) -> ajaya_core::Response {
        use ajaya_router::layer::oneshot;
        oneshot(self.inner, req).await
    }

    /// Expose the inner [`BoxCloneService`] for power users who need to
    /// compose this with another Tower service manually.
    #[inline]
    pub fn into_service(self) -> BoxCloneService {
        self.inner
    }
}

impl fmt::Debug for Next {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Next").finish_non_exhaustive()
    }
}