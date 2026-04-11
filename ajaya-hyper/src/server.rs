//! Server implementation using Hyper 1.x and Tokio.
//!
//! Provides a TCP listener that accepts connections and serves
//! HTTP responses using Hyper's connection builder.

use std::net::SocketAddr;

use bytes::Bytes;
use http_body_util::Full;
use hyper::body::Incoming;
use hyper::service::service_fn;
use hyper_util::rt::{TokioExecutor, TokioIo};
use hyper_util::server::conn::auto::Builder;
use tokio::net::TcpListener;

/// The Ajaya HTTP server.
///
/// Wraps a Tokio TCP listener and Hyper connection builder
/// to accept and serve HTTP connections.
///
/// # Example
///
/// ```rust,ignore
/// use ajaya_hyper::Server;
///
/// #[tokio::main]
/// async fn main() {
///     let server = Server::bind("0.0.0.0:8080").await.unwrap();
///     server.serve().await.unwrap();
/// }
/// ```
pub struct Server {
    listener: TcpListener,
    addr: SocketAddr,
}

impl Server {
    /// Bind the server to the given address.
    ///
    /// Returns a `Server` ready to accept connections.
    pub async fn bind(addr: &str) -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        let listener = TcpListener::bind(addr).await?;
        let addr = listener.local_addr()?;
        tracing::info!("🔱 Ajaya listening on http://{}", addr);
        Ok(Self { listener, addr })
    }

    /// Returns the local address the server is bound to.
    pub fn local_addr(&self) -> SocketAddr {
        self.addr
    }

    /// Start serving HTTP requests.
    ///
    /// This runs an infinite accept loop, spawning a new Tokio task
    /// for each incoming connection. Currently responds with
    /// "Hello from Ajaya" to every request.
    pub async fn serve(self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        loop {
            let (stream, peer_addr) = self.listener.accept().await?;
            let io = TokioIo::new(stream);

            tracing::debug!("Accepted connection from {}", peer_addr);

            tokio::task::spawn(async move {
                let service = service_fn(move |_req: hyper::Request<Incoming>| {
                    let peer = peer_addr;
                    async move {
                        tracing::debug!("Request from {}", peer);
                        let response = hyper::Response::builder()
                            .status(200)
                            .header("content-type", "text/plain; charset=utf-8")
                            .header("server", "Ajaya/0.0.1")
                            .body(Full::new(Bytes::from("Hello from Ajaya")))
                            .unwrap();
                        Ok::<_, hyper::Error>(response)
                    }
                });

                if let Err(err) = Builder::new(TokioExecutor::new())
                    .serve_connection(io, service)
                    .await
                {
                    tracing::error!("Connection error: {}", err);
                }
            });
        }
    }
}
