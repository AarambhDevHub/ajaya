//! Server implementation using Hyper 1.x and Tokio.
//!
//! Provides a TCP listener that accepts connections and serves
//! HTTP responses using Hyper's connection builder.

use std::net::SocketAddr;
use std::sync::Arc;

use ajaya_core::Body;
use ajaya_core::handler::Handler;
use ajaya_router::layer::BoxCloneService;
use ajaya_router::{MethodRouter, Router};
use hyper::body::Incoming;
use hyper::service::service_fn;
use hyper_util::rt::{TokioExecutor, TokioIo};
use hyper_util::server::conn::auto::Builder;
use tokio::net::TcpListener;
use tower_service::Service as _;

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
/// async fn hello() -> &'static str { "Hello!" }
///
/// #[tokio::main]
/// async fn main() {
///     let server = Server::bind("0.0.0.0:8080").await.unwrap();
///     server.serve(hello).await.unwrap();
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

    // ── Serve methods ────────────────────────────────────────────────────────

    /// Serve any pre-built Tower [`BoxCloneService`].
    ///
    /// This is the lowest-level serve method. Use it when you've composed
    /// middleware manually via `router.into_service()` and want full control.
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// let svc = app
    ///     .layer(CorsLayer::permissive())
    ///     .with_state(state)
    ///     .into_service();
    ///
    /// Server::bind("0.0.0.0:8080").await?.serve_service(svc).await?;
    /// ```
    pub async fn serve_service(
        self,
        service: BoxCloneService,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        loop {
            let (stream, peer_addr) = self.listener.accept().await?;
            let io = TokioIo::new(stream);
            let svc = service.clone();

            tracing::debug!("Accepted connection from {}", peer_addr);

            tokio::task::spawn(async move {
                // let svc = svc;
                let hyper_svc = service_fn(move |req: hyper::Request<Incoming>| {
                    let mut s = svc.clone();
                    async move {
                        let ajaya_req = ajaya_core::Request::from_hyper(req);
                        // poll_ready is always Poll::Ready for our services
                        let _ = std::future::poll_fn(|cx| s.poll_ready(cx)).await;
                        let response = s
                            .call(ajaya_req)
                            .await
                            .unwrap_or_else(|infallible| match infallible {});
                        Ok::<http::Response<Body>, hyper::Error>(response)
                    }
                });

                if let Err(err) = Builder::new(TokioExecutor::new())
                    .serve_connection(io, hyper_svc)
                    .await
                {
                    tracing::error!("Connection error: {}", err);
                }
            });
        }
    }

    /// Serve a [`Router`] with all configured layers applied.
    ///
    /// Calls [`Router::into_service`] internally. This is the recommended
    /// entry point for most applications.
    pub async fn serve_app(
        self,
        router: Router,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let svc = router.into_service();
        self.serve_service(svc).await
    }

    /// Serve a bare async handler (no routing, no layers).
    ///
    /// Useful for simple single-handler servers or testing.
    pub async fn serve<H, T>(
        self,
        handler: H,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>>
    where
        H: Handler<T> + Clone + Send + Sync + 'static,
        T: 'static,
    {
        loop {
            let (stream, peer_addr) = self.listener.accept().await?;
            let io = TokioIo::new(stream);
            let handler = handler.clone();

            tracing::debug!("Accepted connection from {}", peer_addr);

            tokio::task::spawn(async move {
                let handler = handler.clone();
                let hyper_svc = service_fn(move |req: hyper::Request<Incoming>| {
                    let handler = handler.clone();
                    async move {
                        let ajaya_req = ajaya_core::Request::from_hyper(req);
                        let response = handler.call(ajaya_req, ()).await;
                        Ok::<http::Response<Body>, hyper::Error>(response)
                    }
                });

                if let Err(err) = Builder::new(TokioExecutor::new())
                    .serve_connection(io, hyper_svc)
                    .await
                {
                    tracing::error!("Connection error: {}", err);
                }
            });
        }
    }

    /// Serve a [`MethodRouter`] (single path, method dispatch).
    pub async fn serve_method_router(
        self,
        router: MethodRouter,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let router = Arc::new(router);

        loop {
            let (stream, peer_addr) = self.listener.accept().await?;
            let io = TokioIo::new(stream);
            let router = Arc::clone(&router);

            tracing::debug!("Accepted connection from {}", peer_addr);

            tokio::task::spawn(async move {
                let router = Arc::clone(&router);
                let hyper_svc = service_fn(move |req: hyper::Request<Incoming>| {
                    let router = Arc::clone(&router);
                    async move {
                        let ajaya_req = ajaya_core::Request::from_hyper(req);
                        let response = router.call(ajaya_req, ()).await;
                        Ok::<http::Response<Body>, hyper::Error>(response)
                    }
                });

                if let Err(err) = Builder::new(TokioExecutor::new())
                    .serve_connection(io, hyper_svc)
                    .await
                {
                    tracing::error!("Connection error: {}", err);
                }
            });
        }
    }
}
