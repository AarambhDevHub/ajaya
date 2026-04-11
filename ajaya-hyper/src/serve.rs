//! Convenience `serve` function.
//!
//! A one-liner to start the Ajaya server on a given address.

use crate::Server;

/// Start the Ajaya HTTP server on the given address.
///
/// This is a convenience wrapper around [`Server::bind`] and [`Server::serve`].
///
/// # Example
///
/// ```rust,ignore
/// use ajaya_hyper::serve;
///
/// #[tokio::main]
/// async fn main() {
///     serve("0.0.0.0:8080").await.unwrap();
/// }
/// ```
pub async fn serve(addr: &str) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let server = Server::bind(addr).await?;
    server.serve().await
}
