//! Server implementation using Hyper 1.x and Tokio.

use std::convert::Infallible;
use std::future::Future;
use std::marker::PhantomData;
use std::net::SocketAddr;
use std::pin::Pin;
use std::task::{Context, Poll};

use arvik_core::Body;
use arvik_core::handler::Handler;
use arvik_core::{Request, Response};
use arvik_router::layer::BoxCloneService;
use arvik_router::{MethodRouter, Router};
use hyper::body::Incoming;
use hyper::service::service_fn;
use hyper_util::rt::TokioIo;
use tokio::net::TcpListener;
use tower_service::Service as _;

use crate::ServerConfig;

type BoxError = Box<dyn std::error::Error + Send + Sync>;

/// The Arvik HTTP server.
pub struct Server {
    listener: TcpListener,
    addr: SocketAddr,
    config: ServerConfig,
}

impl Server {
    /// Bind the server to the given address with default runtime/protocol tuning.
    pub async fn bind(addr: &str) -> Result<Self, BoxError> {
        Self::bind_with_config(addr, ServerConfig::default()).await
    }

    /// Bind the server to the given address with explicit runtime/protocol tuning.
    pub async fn bind_with_config(addr: &str, config: ServerConfig) -> Result<Self, BoxError> {
        let listener = TcpListener::bind(addr).await?;
        let addr = listener.local_addr()?;
        tracing::info!("Arvik listening on http://{}", addr);
        Ok(Self {
            listener,
            addr,
            config,
        })
    }

    /// Returns the local address the server is bound to.
    pub fn local_addr(&self) -> SocketAddr {
        self.addr
    }

    /// Returns the server runtime/protocol tuning.
    pub fn config(&self) -> &ServerConfig {
        &self.config
    }

    /// Serve any pre-built Tower [`BoxCloneService`].
    pub async fn serve_service(self, service: BoxCloneService) -> Result<(), BoxError> {
        loop {
            let (stream, peer_addr) = self.listener.accept().await?;
            let io = TokioIo::new(stream);
            let service = service.clone();
            let config = self.config.clone();

            tracing::debug!("Accepted connection from {}", peer_addr);

            tokio::spawn(async move {
                run_connection(io, service, config, peer_addr).await;
            });
        }
    }

    /// Serve a [`Router`] with all configured layers applied.
    pub async fn serve_app(self, router: Router) -> Result<(), BoxError> {
        self.serve_service(router.into_service()).await
    }

    /// Serve a bare async handler (no routing, no layers).
    pub async fn serve<H, T>(self, handler: H) -> Result<(), BoxError>
    where
        H: Handler<T> + Clone + Send + Sync + 'static,
        T: 'static,
    {
        self.serve_service(BoxCloneService::new(HandlerService {
            handler,
            _marker: PhantomData,
        }))
        .await
    }

    /// Serve a [`MethodRouter`] (single path, method dispatch).
    pub async fn serve_method_router(self, router: MethodRouter) -> Result<(), BoxError> {
        self.serve_service(BoxCloneService::new(MethodRouterService { router }))
            .await
    }

    /// Serve a [`Router`] over rustls TLS.
    #[cfg(feature = "tls")]
    pub async fn serve_tls_app(
        self,
        router: Router,
        tls_config: arvik_tls::rustls::RustlsConfig,
    ) -> Result<(), BoxError> {
        self.serve_tls_service(router.into_service(), tls_config)
            .await
    }

    /// Serve any pre-built Tower service over rustls TLS.
    #[cfg(feature = "tls")]
    pub async fn serve_tls_service(
        self,
        service: BoxCloneService,
        tls_config: arvik_tls::rustls::RustlsConfig,
    ) -> Result<(), BoxError> {
        loop {
            let (stream, peer_addr) = self.listener.accept().await?;
            let service = service.clone();
            let tls_config = tls_config.clone();
            let config = self.config.clone();

            tracing::debug!("Accepted TLS connection from {}", peer_addr);

            tokio::spawn(async move {
                match tls_config.accept(stream).await {
                    Ok(tls_stream) => {
                        run_connection(TokioIo::new(tls_stream), service, config, peer_addr).await;
                    }
                    Err(err) => tracing::warn!("TLS handshake failed from {}: {}", peer_addr, err),
                }
            });
        }
    }

    /// Serve a [`Router`] over native-tls.
    #[cfg(feature = "native-tls")]
    pub async fn serve_native_tls_app(
        self,
        router: Router,
        tls_config: arvik_tls::native::NativeTlsConfig,
    ) -> Result<(), BoxError> {
        self.serve_native_tls_service(router.into_service(), tls_config)
            .await
    }

    /// Serve any pre-built Tower service over native-tls.
    #[cfg(feature = "native-tls")]
    pub async fn serve_native_tls_service(
        self,
        service: BoxCloneService,
        tls_config: arvik_tls::native::NativeTlsConfig,
    ) -> Result<(), BoxError> {
        loop {
            let (stream, peer_addr) = self.listener.accept().await?;
            let service = service.clone();
            let tls_config = tls_config.clone();
            let config = self.config.clone();

            tracing::debug!("Accepted native-tls connection from {}", peer_addr);

            tokio::spawn(async move {
                match tls_config.accept(stream).await {
                    Ok(tls_stream) => {
                        run_connection(TokioIo::new(tls_stream), service, config, peer_addr).await;
                    }
                    Err(err) => {
                        tracing::warn!("native-tls handshake failed from {}: {}", peer_addr, err)
                    }
                }
            });
        }
    }
}

async fn run_connection<I>(
    io: I,
    service: BoxCloneService,
    config: ServerConfig,
    peer_addr: SocketAddr,
) where
    I: hyper::rt::Read + hyper::rt::Write + Unpin + Send + 'static,
{
    let hyper_svc = service_fn(move |req: hyper::Request<Incoming>| {
        let mut service = service.clone();
        async move {
            let arvik_req = Request::from_hyper(req);
            let _ = std::future::poll_fn(|cx| service.poll_ready(cx)).await;
            let response = service
                .call(arvik_req)
                .await
                .unwrap_or_else(|infallible| match infallible {});
            Ok::<http::Response<Body>, Infallible>(response)
        }
    });

    let builder = config.auto_builder();
    let result = if config.is_http2_only() {
        builder.serve_connection(io, hyper_svc).await
    } else {
        builder.serve_connection_with_upgrades(io, hyper_svc).await
    };

    if let Err(err) = result {
        tracing::error!("Connection error from {}: {}", peer_addr, err);
    }
}

struct HandlerService<H, T> {
    handler: H,
    _marker: PhantomData<fn() -> T>,
}

impl<H: Clone, T> Clone for HandlerService<H, T> {
    fn clone(&self) -> Self {
        Self {
            handler: self.handler.clone(),
            _marker: PhantomData,
        }
    }
}

impl<H, T> tower_service::Service<Request> for HandlerService<H, T>
where
    H: Handler<T> + Clone + Send + Sync + 'static,
    T: 'static,
{
    type Response = Response;
    type Error = Infallible;
    type Future = Pin<Box<dyn Future<Output = Result<Response, Infallible>> + Send + 'static>>;

    fn poll_ready(&mut self, _cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        Poll::Ready(Ok(()))
    }

    fn call(&mut self, req: Request) -> Self::Future {
        let handler = self.handler.clone();
        Box::pin(async move { Ok(handler.call(req, ()).await) })
    }
}

struct MethodRouterService {
    router: MethodRouter,
}

impl Clone for MethodRouterService {
    fn clone(&self) -> Self {
        Self {
            router: self.router.clone(),
        }
    }
}

impl tower_service::Service<Request> for MethodRouterService {
    type Response = Response;
    type Error = Infallible;
    type Future = Pin<Box<dyn Future<Output = Result<Response, Infallible>> + Send + 'static>>;

    fn poll_ready(&mut self, _cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        Poll::Ready(Ok(()))
    }

    fn call(&mut self, req: Request) -> Self::Future {
        let router = self.router.clone();
        Box::pin(async move { Ok(router.call(req, ()).await) })
    }
}
