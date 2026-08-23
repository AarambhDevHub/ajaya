//! Extractor traits for the Arvik web framework.
//!
//! This module defines the two core traits that all extractors implement:
//!
//! - [`FromRequestParts`] — Extractors that only need headers, URI, method,
//!   and extensions (no body consumption). Multiple of these can be used
//!   per handler.
//!
//! - [`FromRequest`] — Extractors that consume the request body. Only **one**
//!   `FromRequest` extractor is allowed per handler (it must be the last parameter).
//!
//! # How Handlers Use Extractors
//!
//! When you write a handler function:
//!
//! ```rust,ignore
//! async fn handler(
//!     method: Method,           // FromRequestParts (1st)
//!     Path(id): Path<u32>,      // FromRequestParts (2nd)
//!     Json(body): Json<Payload> // FromRequest (last — consumes body)
//! ) -> impl IntoResponse {
//!     // ...
//! }
//! ```
//!
//! The framework generates code that:
//! 1. Splits the request into `(RequestParts, Body)`
//! 2. Extracts `method` and `Path(id)` from `&mut RequestParts`
//! 3. Reconstructs the request and extracts `Json(body)` via `FromRequest`
//! 4. Calls your handler function with the extracted values

use std::convert::Infallible;

use crate::into_response::IntoResponse;
use crate::request::Request;
use crate::request_parts::RequestParts;

/// Marker type for extractors that go through the full request.
pub struct ViaRequest;

/// Marker type for extractors that go through request parts only.
pub struct ViaParts;

/// Extracts a value from the non-body parts of a request.
///
/// Implementors of this trait can be used as handler parameters
/// in any position. Multiple `FromRequestParts` extractors can
/// be used in a single handler.
///
/// # Example
///
/// ```rust,ignore
/// use arvik_core::extract::FromRequestParts;
/// use arvik_core::request_parts::RequestParts;
///
/// pub struct MyExtractor(String);
///
/// impl<S: Send + Sync> FromRequestParts<S> for MyExtractor {
///     type Rejection = std::convert::Infallible;
///
///     async fn from_request_parts(
///         parts: &mut RequestParts,
///         _state: &S,
///     ) -> Result<Self, Self::Rejection> {
///         Ok(MyExtractor(parts.uri().to_string()))
///     }
/// }
/// ```
pub trait FromRequestParts<S>: Sized {
    /// The rejection type returned when extraction fails.
    ///
    /// Must implement [`IntoResponse`] so it can be converted
    /// to an HTTP error response automatically.
    type Rejection: IntoResponse;

    /// Extract this type from the request parts and state.
    fn from_request_parts(
        parts: &mut RequestParts,
        state: &S,
    ) -> impl Future<Output = Result<Self, Self::Rejection>> + Send;
}

/// Extracts a value from the full request (may consume the body).
///
/// Only **one** `FromRequest` extractor is allowed per handler,
/// and it must be the **last** parameter.
///
/// The type parameter `M` is a marker to prevent conflicting impls.
///
/// # Example
///
/// ```rust,ignore
/// use arvik_core::extract::FromRequest;
/// use arvik_core::request::Request;
///
/// pub struct MyBody(Vec<u8>);
///
/// impl<S: Send + Sync> FromRequest<S> for MyBody {
///     type Rejection = arvik_core::Error;
///
///     async fn from_request(
///         req: Request,
///         _state: &S,
///     ) -> Result<Self, Self::Rejection> {
///         let bytes = req.into_body().to_bytes().await
///             .map_err(|e| arvik_core::Error::new(e))?;
///         Ok(MyBody(bytes.to_vec()))
///     }
/// }
/// ```
pub trait FromRequest<S, M = ViaRequest>: Sized {
    /// The rejection type returned when extraction fails.
    type Rejection: IntoResponse;

    /// Extract this type from the full request and state.
    fn from_request(
        req: Request,
        state: &S,
    ) -> impl Future<Output = Result<Self, Self::Rejection>> + Send;
}

// ---------------------------------------------------------------------------
// Blanket: FromRequestParts<S> → FromRequest<S, ViaParts>
// ---------------------------------------------------------------------------

/// Every `FromRequestParts` extractor can also be used as a
/// `FromRequest` extractor. The body is discarded.
impl<S, T> FromRequest<S, ViaParts> for T
where
    T: FromRequestParts<S>,
    S: Send + Sync,
{
    type Rejection = T::Rejection;

    async fn from_request(req: Request, state: &S) -> Result<Self, Self::Rejection> {
        let (mut parts, _body) = req.into_request_parts();
        T::from_request_parts(&mut parts, state).await
    }
}

// ---------------------------------------------------------------------------
// Optional extractors: Option<T> — never rejects
// ---------------------------------------------------------------------------

/// An `Option<T>` extractor never rejects. Returns `None` if the
/// inner extractor fails.
impl<S, T> FromRequestParts<S> for Option<T>
where
    T: FromRequestParts<S>,
    S: Send + Sync,
{
    type Rejection = Infallible;

    async fn from_request_parts(
        parts: &mut RequestParts,
        state: &S,
    ) -> Result<Self, Self::Rejection> {
        Ok(T::from_request_parts(parts, state).await.ok())
    }
}

// ---------------------------------------------------------------------------
// Result extractors: Result<T, T::Rejection> — gives you the rejection
// ---------------------------------------------------------------------------

/// A `Result<T, T::Rejection>` extractor never rejects. Returns `Err(rejection)`
/// if the inner extractor fails, allowing the handler to inspect the error.
impl<S, T> FromRequestParts<S> for Result<T, T::Rejection>
where
    T: FromRequestParts<S>,
    S: Send + Sync,
{
    type Rejection = Infallible;

    async fn from_request_parts(
        parts: &mut RequestParts,
        state: &S,
    ) -> Result<Self, Self::Rejection> {
        Ok(T::from_request_parts(parts, state).await)
    }
}

// ---------------------------------------------------------------------------
// Use `Future` trait from std
// ---------------------------------------------------------------------------
use std::future::Future;

// ---------------------------------------------------------------------------
// Standard type impls: http::Method, http::Uri, http::Version, http::HeaderMap
// These must live in arvik-core because of Rust's orphan rule.
// ---------------------------------------------------------------------------

/// Extract the HTTP method from the request (infallible).
impl<S: Send + Sync> FromRequestParts<S> for http::Method {
    type Rejection = Infallible;

    async fn from_request_parts(
        parts: &mut RequestParts,
        _state: &S,
    ) -> Result<Self, Self::Rejection> {
        Ok(parts.method().clone())
    }
}

/// Extract the request URI (infallible).
impl<S: Send + Sync> FromRequestParts<S> for http::Uri {
    type Rejection = Infallible;

    async fn from_request_parts(
        parts: &mut RequestParts,
        _state: &S,
    ) -> Result<Self, Self::Rejection> {
        Ok(parts.uri().clone())
    }
}

/// Extract the HTTP version (infallible).
impl<S: Send + Sync> FromRequestParts<S> for http::Version {
    type Rejection = Infallible;

    async fn from_request_parts(
        parts: &mut RequestParts,
        _state: &S,
    ) -> Result<Self, Self::Rejection> {
        Ok(parts.version())
    }
}

/// Extract a clone of the full header map (infallible).
impl<S: Send + Sync> FromRequestParts<S> for http::HeaderMap {
    type Rejection = Infallible;

    async fn from_request_parts(
        parts: &mut RequestParts,
        _state: &S,
    ) -> Result<Self, Self::Rejection> {
        Ok(parts.headers().clone())
    }
}

// ---------------------------------------------------------------------------
// Body-consuming standard type impls
// ---------------------------------------------------------------------------

/// Extract the raw request body as [`crate::Body`] (infallible).
impl<S: Send + Sync> FromRequest<S> for crate::Body {
    type Rejection = Infallible;

    async fn from_request(req: Request, _state: &S) -> Result<Self, Self::Rejection> {
        Ok(req.into_body())
    }
}

/// Extract the entire [`Request`] as-is (infallible escape hatch).
impl<S: Send + Sync> FromRequest<S> for Request {
    type Rejection = Infallible;

    async fn from_request(req: Request, _state: &S) -> Result<Self, Self::Rejection> {
        Ok(req)
    }
}

/// Extract the raw body as [`bytes::Bytes`].
///
/// The body is buffered under the request's [`crate::body_limit::DefaultBodyLimit`]
/// (2 MiB by default); larger bodies are rejected with `413`.
impl<S: Send + Sync> FromRequest<S> for bytes::Bytes {
    type Rejection = crate::Error;

    async fn from_request(req: Request, _state: &S) -> Result<Self, Self::Rejection> {
        let limit = crate::body_limit::resolve_limit(req.extensions());
        req.into_body()
            .to_bytes_limited(limit)
            .await
            .map_err(body_limit_rejection)
    }
}

/// Extract the raw body as a UTF-8 [`String`].
///
/// The body is buffered under the request's [`crate::body_limit::DefaultBodyLimit`]
/// (2 MiB by default); larger bodies are rejected with `413`.
impl<S: Send + Sync> FromRequest<S> for String {
    type Rejection = crate::Error;

    async fn from_request(req: Request, _state: &S) -> Result<Self, Self::Rejection> {
        let limit = crate::body_limit::resolve_limit(req.extensions());
        let bytes = req
            .into_body()
            .to_bytes_limited(limit)
            .await
            .map_err(body_limit_rejection)?;
        String::from_utf8(bytes.to_vec())
            .map_err(|e| crate::Error::new(e).with_status(http::StatusCode::BAD_REQUEST))
    }
}

/// Map a limited-body read failure onto an [`crate::Error`] response:
/// `413 Payload Too Large` when the limit was hit, `500` on read errors.
fn body_limit_rejection(e: crate::body_limit::BodyLimitError) -> crate::Error {
    if e.is_payload_too_large() {
        return crate::Error::new(e).with_status(http::StatusCode::PAYLOAD_TOO_LARGE);
    }
    crate::Error::new(e)
}

#[cfg(test)]
mod body_limit_tests {
    use super::*;
    use crate::body::Body;
    use crate::body_limit::{DEFAULT_BODY_LIMIT, DefaultBodyLimit};
    use http::StatusCode;

    fn post_request(body: Body) -> Request {
        Request::new(
            http::Request::builder()
                .method(http::Method::POST)
                .uri("/")
                .body(body)
                .unwrap(),
        )
    }

    #[tokio::test]
    async fn bytes_extractor_enforces_default_body_limit() {
        let req = post_request(Body::from_bytes(vec![0u8; DEFAULT_BODY_LIMIT + 1].into()));
        let err = <bytes::Bytes as FromRequest<()>>::from_request(req, &())
            .await
            .unwrap_err();
        assert_eq!(err.into_response().status(), StatusCode::PAYLOAD_TOO_LARGE);
    }

    #[tokio::test]
    async fn bytes_extractor_accepts_body_at_limit() {
        let req = post_request(Body::from_bytes(vec![0u8; DEFAULT_BODY_LIMIT].into()));
        let bytes = <bytes::Bytes as FromRequest<()>>::from_request(req, &())
            .await
            .unwrap();
        assert_eq!(bytes.len(), DEFAULT_BODY_LIMIT);
    }

    #[tokio::test]
    async fn default_body_limit_extension_overrides_the_default() {
        let mut req = post_request(Body::from_bytes(vec![0u8; 16].into()));
        req.extensions_mut().insert(DefaultBodyLimit::max(8));
        let err = <bytes::Bytes as FromRequest<()>>::from_request(req, &())
            .await
            .unwrap_err();
        assert_eq!(err.into_response().status(), StatusCode::PAYLOAD_TOO_LARGE);

        let mut req = post_request(Body::from_bytes(vec![0u8; 16].into()));
        req.extensions_mut().insert(DefaultBodyLimit::disabled());
        let bytes = <bytes::Bytes as FromRequest<()>>::from_request(req, &())
            .await
            .unwrap();
        assert_eq!(bytes.len(), 16);
    }

    #[tokio::test]
    async fn string_extractor_rejects_oversized_body() {
        let req = post_request(Body::from_bytes(vec![b'a'; DEFAULT_BODY_LIMIT + 1].into()));
        let err = <String as FromRequest<()>>::from_request(req, &())
            .await
            .unwrap_err();
        assert_eq!(err.into_response().status(), StatusCode::PAYLOAD_TOO_LARGE);
    }
}
