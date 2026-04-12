//! Server implementation using Hyper 1.x and Tokio.
//!
//! Provides a TCP listener that accepts connections and serves
//! HTTP responses using Hyper's connection builder.

use std::net::SocketAddr;
use std::sync::Arc;

use ajaya_core::Body;
use ajaya_core::handler::Handler;
use ajaya_router::MethodRouter;
use ajaya_router::Router;
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

    /// Start serving HTTP requests using the provided handler.
    ///
    /// Any async function that returns `impl IntoResponse` works.
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
                let service = service_fn(move |req: hyper::Request<Incoming>| {
                    let handler = handler.clone();
                    async move {
                        let ajaya_req = ajaya_core::Request::from_hyper(req);
                        let response = handler.call(ajaya_req, ()).await;
                        Ok::<http::Response<Body>, hyper::Error>(response)
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

    /// Start serving HTTP requests using a [`MethodRouter`].
    ///
    /// The `MethodRouter` dispatches requests based on the HTTP method.
    /// Returns `405 Method Not Allowed` for unregistered methods.
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// use ajaya_router::{get, post};
    ///
    /// async fn hello() -> &'static str { "Hello!" }
    /// async fn create() -> &'static str { "Created!" }
    ///
    /// let router = get(hello).post(create);
    /// let server = Server::bind("0.0.0.0:8080").await?;
    /// server.serve_method_router(router).await?;
    /// ```
    pub async fn serve_method_router(
        self,
        router: MethodRouter,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let router = Arc::new(router);

        loop {
            let (stream, peer_addr) = self.listener.accept().await?;
            let io = TokioIo::new(stream);
            let router = router.clone();

            tracing::debug!("Accepted connection from {}", peer_addr);

            tokio::task::spawn(async move {
                let router = router.clone();
                let service = service_fn(move |req: hyper::Request<Incoming>| {
                    let router = router.clone();
                    async move {
                        let ajaya_req = ajaya_core::Request::from_hyper(req);
                        let response = router.call(ajaya_req, ()).await;
                        Ok::<http::Response<Body>, hyper::Error>(response)
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

    /// Start serving HTTP requests using a [`Router`].
    ///
    /// The `Router` dispatches requests based on path and HTTP method.
    /// Returns `404 Not Found` for unmatched paths and `405 Method Not Allowed`
    /// for unmatched methods.
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// use ajaya_router::{Router, get, post};
    ///
    /// async fn home() -> &'static str { "Home" }
    /// async fn users() -> &'static str { "Users" }
    ///
    /// let app = Router::new()
    ///     .route("/", get(home))
    ///     .route("/users", get(users));
    ///
    /// let server = Server::bind("0.0.0.0:8080").await?;
    /// server.serve_app(app).await?;
    /// ```
    pub async fn serve_app(
        self,
        router: Router,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let router = Arc::new(router);

        loop {
            let (stream, peer_addr) = self.listener.accept().await?;
            let io = TokioIo::new(stream);
            let router = router.clone();

            tracing::debug!("Accepted connection from {}", peer_addr);

            tokio::task::spawn(async move {
                let router = router.clone();
                let service = service_fn(move |req: hyper::Request<Incoming>| {
                    let router = router.clone();
                    async move {
                        let ajaya_req = ajaya_core::Request::from_hyper(req);
                        let response = router.call(ajaya_req, ()).await;
                        Ok::<http::Response<Body>, hyper::Error>(response)
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
