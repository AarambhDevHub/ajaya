//! Connection info extractor.
//!
//! Extracts client connection information (e.g., socket address)
//! from the request extensions. This must be inserted by the
//! server layer during connection accept.
//!
//! # Examples
//!
//! ```rust,ignore
//! use ajaya::ConnectInfo;
//! use std::net::SocketAddr;
//!
//! async fn handler(ConnectInfo(addr): ConnectInfo<SocketAddr>) -> String {
//!     format!("Client IP: {}", addr.ip())
//! }
//! ```

use ajaya_core::extract::FromRequestParts;
use ajaya_core::into_response::IntoResponse;
use ajaya_core::request_parts::RequestParts;
use ajaya_core::response::{Response, ResponseBuilder};

/// Client connection information extractor.
///
/// The type parameter `T` is the connection info type stored
/// in request extensions by the server's accept loop.
/// Typically `std::net::SocketAddr`.
#[derive(Debug, Clone)]
pub struct ConnectInfo<T>(pub T);

/// Rejection for missing connection info.
#[derive(Debug)]
pub struct ConnectInfoRejection;

impl std::fmt::Display for ConnectInfoRejection {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "Missing ConnectInfo. Did you configure the server to insert it?"
        )
    }
}

impl IntoResponse for ConnectInfoRejection {
    fn into_response(self) -> Response {
        ResponseBuilder::new()
            .status(http::StatusCode::INTERNAL_SERVER_ERROR)
            .body(ajaya_core::Body::from(self.to_string()))
    }
}

impl<S, T> FromRequestParts<S> for ConnectInfo<T>
where
    T: Clone + Send + Sync + 'static,
    S: Send + Sync,
{
    type Rejection = ConnectInfoRejection;

    async fn from_request_parts(
        parts: &mut RequestParts,
        _state: &S,
    ) -> Result<Self, Self::Rejection> {
        parts
            .extensions()
            .get::<ConnectInfo<T>>()
            .cloned()
            .ok_or(ConnectInfoRejection)
    }
}
