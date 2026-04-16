//! Typed request extension extractor.
//!
//! Extracts a value from the request's typed extensions map.
//! Extensions are typically inserted by middleware.
//!
//! # Examples
//!
//! ```rust,ignore
//! use ajaya::Extension;
//!
//! #[derive(Clone)]
//! struct CurrentUser { id: u32, name: String }
//!
//! // Middleware inserts CurrentUser into extensions
//! // Handler extracts it:
//! async fn handler(Extension(user): Extension<CurrentUser>) -> String {
//!     format!("Hello, {}!", user.name)
//! }
//! ```

use ajaya_core::extract::FromRequestParts;
use ajaya_core::request_parts::RequestParts;

use crate::rejection::ExtensionRejection;

/// Typed request extension extractor.
///
/// Extracts a clone of `T` from the request extensions.
/// `T` must be `Clone + Send + Sync + 'static`.
#[derive(Debug, Clone)]
pub struct Extension<T>(pub T);

impl<S, T> FromRequestParts<S> for Extension<T>
where
    T: Clone + Send + Sync + 'static,
    S: Send + Sync,
{
    type Rejection = ExtensionRejection;

    async fn from_request_parts(
        parts: &mut RequestParts,
        _state: &S,
    ) -> Result<Self, Self::Rejection> {
        parts
            .extensions()
            .get::<T>()
            .cloned()
            .map(Extension)
            .ok_or_else(|| ExtensionRejection(std::any::type_name::<T>().to_string()))
    }
}
