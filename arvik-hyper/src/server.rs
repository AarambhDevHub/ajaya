//! Server implementation using Hyper 1.x and Tokio.

use std::convert::Infallible;
use std::future::{Future, pending};
use std::io;
use std::marker::PhantomData;
use std::net::SocketAddr;
use std::pin::Pin;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::task::{Context, Poll};

use arvik_core::Body;
use arvik_core::handler::Handler;
use arvik_core::{Request, Response};
use arvik_router::layer::BoxCloneService;
use arvik_router::{MethodRouter, Router};
use hyper::body::Incoming;
use hyper::service::service_fn;
use hyper_util::rt::TokioIo;
use socket2::{Domain, Protocol, Socket, TcpKeepalive, Type};
use tokio::net::{TcpListener, TcpStream, lookup_host};
use tokio::sync::mpsc;
use tokio::task::JoinSet;
use tower_service::Service as _;

use crate::{ConnectionInfo, ServerConfig, ShutdownConfig};

type BoxError = Box<dyn std::error::Error + Send + Sync>;

/// The Arvik HTTP server.
pub struct Server {
    listeners: Vec<TcpListener>,
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
        let listeners = bind_listeners(addr, &config).await?;
        let addr = listeners
            .first()
            .expect("bind_listeners always returns at least one listener")
            .local_addr()?;
        tracing::info!("Arvik listening on http://{}", addr);
        Ok(Self {
            listeners,
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
        self.serve_service_with_graceful_shutdown(
            service,
            pending::<()>(),
            ShutdownConfig::default(),
        )
        .await
    }

    /// Serve any pre-built Tower [`BoxCloneService`] until `signal` resolves.
    pub async fn serve_service_with_graceful_shutdown<F>(
        self,
        service: BoxCloneService,
        signal: F,
        shutdown_config: ShutdownConfig,
    ) -> Result<(), BoxError>
    where
        F: Future<Output = ()> + Send,
    {
        self.accept_plain(service, signal, shutdown_config).await
    }

    /// Serve a [`Router`] with all configured layers applied.
    pub async fn serve_app(self, router: Router) -> Result<(), BoxError> {
        self.serve_service(router.into_service()).await
    }

    /// Serve a [`Router`] until `signal` resolves.
    pub async fn serve_app_with_graceful_shutdown<F>(
        self,
        router: Router,
        signal: F,
        shutdown_config: ShutdownConfig,
    ) -> Result<(), BoxError>
    where
        F: Future<Output = ()> + Send,
    {
        self.serve_service_with_graceful_shutdown(router.into_service(), signal, shutdown_config)
            .await
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

    /// Serve a bare async handler until `signal` resolves.
    pub async fn serve_with_graceful_shutdown<H, T, F>(
        self,
        handler: H,
        signal: F,
        shutdown_config: ShutdownConfig,
    ) -> Result<(), BoxError>
    where
        H: Handler<T> + Clone + Send + Sync + 'static,
        T: 'static,
        F: Future<Output = ()> + Send,
    {
        self.serve_service_with_graceful_shutdown(
            BoxCloneService::new(HandlerService {
                handler,
                _marker: PhantomData,
            }),
            signal,
            shutdown_config,
        )
        .await
    }

    /// Serve a [`MethodRouter`] (single path, method dispatch).
    pub async fn serve_method_router(self, router: MethodRouter) -> Result<(), BoxError> {
        self.serve_service(BoxCloneService::new(MethodRouterService { router }))
            .await
    }

    /// Serve a [`MethodRouter`] until `signal` resolves.
    pub async fn serve_method_router_with_graceful_shutdown<F>(
        self,
        router: MethodRouter,
        signal: F,
        shutdown_config: ShutdownConfig,
    ) -> Result<(), BoxError>
    where
        F: Future<Output = ()> + Send,
    {
        self.serve_service_with_graceful_shutdown(
            BoxCloneService::new(MethodRouterService { router }),
            signal,
            shutdown_config,
        )
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

    /// Serve a [`Router`] over rustls TLS until `signal` resolves.
    #[cfg(feature = "tls")]
    pub async fn serve_tls_app_with_graceful_shutdown<F>(
        self,
        router: Router,
        tls_config: arvik_tls::rustls::RustlsConfig,
        signal: F,
        shutdown_config: ShutdownConfig,
    ) -> Result<(), BoxError>
    where
        F: Future<Output = ()> + Send,
    {
        self.serve_tls_service_with_graceful_shutdown(
            router.into_service(),
            tls_config,
            signal,
            shutdown_config,
        )
        .await
    }

    /// Serve any pre-built Tower service over rustls TLS.
    #[cfg(feature = "tls")]
    pub async fn serve_tls_service(
        self,
        service: BoxCloneService,
        tls_config: arvik_tls::rustls::RustlsConfig,
    ) -> Result<(), BoxError> {
        self.serve_tls_service_with_graceful_shutdown(
            service,
            tls_config,
            pending::<()>(),
            ShutdownConfig::default(),
        )
        .await
    }

    /// Serve any pre-built Tower service over rustls TLS until `signal` resolves.
    #[cfg(feature = "tls")]
    pub async fn serve_tls_service_with_graceful_shutdown<F>(
        self,
        service: BoxCloneService,
        tls_config: arvik_tls::rustls::RustlsConfig,
        signal: F,
        shutdown_config: ShutdownConfig,
    ) -> Result<(), BoxError>
    where
        F: Future<Output = ()> + Send,
    {
        self.accept_tls(service, tls_config, signal, shutdown_config)
            .await
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

    /// Serve a [`Router`] over native-tls until `signal` resolves.
    #[cfg(feature = "native-tls")]
    pub async fn serve_native_tls_app_with_graceful_shutdown<F>(
        self,
        router: Router,
        tls_config: arvik_tls::native::NativeTlsConfig,
        signal: F,
        shutdown_config: ShutdownConfig,
    ) -> Result<(), BoxError>
    where
        F: Future<Output = ()> + Send,
    {
        self.serve_native_tls_service_with_graceful_shutdown(
            router.into_service(),
            tls_config,
            signal,
            shutdown_config,
        )
        .await
    }

    /// Serve any pre-built Tower service over native-tls.
    #[cfg(feature = "native-tls")]
    pub async fn serve_native_tls_service(
        self,
        service: BoxCloneService,
        tls_config: arvik_tls::native::NativeTlsConfig,
    ) -> Result<(), BoxError> {
        self.serve_native_tls_service_with_graceful_shutdown(
            service,
            tls_config,
            pending::<()>(),
            ShutdownConfig::default(),
        )
        .await
    }

    /// Serve any pre-built Tower service over native-tls until `signal` resolves.
    #[cfg(feature = "native-tls")]
    pub async fn serve_native_tls_service_with_graceful_shutdown<F>(
        self,
        service: BoxCloneService,
        tls_config: arvik_tls::native::NativeTlsConfig,
        signal: F,
        shutdown_config: ShutdownConfig,
    ) -> Result<(), BoxError>
    where
        F: Future<Output = ()> + Send,
    {
        self.accept_native_tls(service, tls_config, signal, shutdown_config)
            .await
    }

    async fn accept_plain<F>(
        self,
        service: BoxCloneService,
        signal: F,
        shutdown_config: ShutdownConfig,
    ) -> Result<(), BoxError>
    where
        F: Future<Output = ()> + Send,
    {
        let mut signal = Box::pin(signal);
        let mut connections = JoinSet::new();
        let mut accept_workers = spawn_accept_workers(self.listeners);
        let mut accepted = accept_workers.receiver;
        let active = Arc::new(AtomicUsize::new(0));

        loop {
            tokio::select! {
                biased;

                _ = &mut signal => {
                    tracing::info!("Shutdown signal received; stopping listener on {}", self.addr);
                    break;
                }
                joined = connections.join_next(), if !connections.is_empty() => {
                    log_join_result(joined);
                }
                next = accepted.recv() => {
                    let Some(next) = next else {
                        break;
                    };
                    let (stream, peer_addr) = next?;
                    apply_stream_options(&stream, &self.config)?;
                    let info = ConnectionInfo { local_addr: self.addr, peer_addr };
                    if !try_admit_connection(&self.config, &active, info) {
                        drop(stream);
                        continue;
                    }

                    let io = TokioIo::new(stream);
                    let service = service.clone();
                    let config = self.config.clone();
                    let shutdown_config = shutdown_config.clone();
                    let active = Arc::clone(&active);

                    tracing::debug!("Accepted connection from {}", peer_addr);
                    shutdown_config.call_connected(info);

                    connections.spawn(async move {
                        let _guard = ConnectionGuard::new(active, shutdown_config, info);
                        run_connection(io, service, config, peer_addr).await;
                    });
                }
            }
        }

        accept_workers.tasks.abort_all();
        while let Some(joined) = accept_workers.tasks.join_next().await {
            log_join_result(Some(joined));
        }
        drain_connections(connections, shutdown_config).await;
        Ok(())
    }

    #[cfg(feature = "tls")]
    async fn accept_tls<F>(
        self,
        service: BoxCloneService,
        tls_config: arvik_tls::rustls::RustlsConfig,
        signal: F,
        shutdown_config: ShutdownConfig,
    ) -> Result<(), BoxError>
    where
        F: Future<Output = ()> + Send,
    {
        let mut signal = Box::pin(signal);
        let mut connections = JoinSet::new();
        let mut accept_workers = spawn_accept_workers(self.listeners);
        let mut accepted = accept_workers.receiver;
        let active = Arc::new(AtomicUsize::new(0));

        loop {
            tokio::select! {
                biased;

                _ = &mut signal => {
                    tracing::info!("Shutdown signal received; stopping TLS listener on {}", self.addr);
                    break;
                }
                joined = connections.join_next(), if !connections.is_empty() => {
                    log_join_result(joined);
                }
                next = accepted.recv() => {
                    let Some(next) = next else {
                        break;
                    };
                    let (stream, peer_addr) = next?;
                    apply_stream_options(&stream, &self.config)?;
                    let info = ConnectionInfo { local_addr: self.addr, peer_addr };
                    if !try_admit_connection(&self.config, &active, info) {
                        drop(stream);
                        continue;
                    }

                    let service = service.clone();
                    let tls_config = tls_config.clone();
                    let config = self.config.clone();
                    let shutdown_config = shutdown_config.clone();
                    let active = Arc::clone(&active);

                    tracing::debug!("Accepted TLS connection from {}", peer_addr);
                    shutdown_config.call_connected(info);

                    connections.spawn(async move {
                        let _guard = ConnectionGuard::new(active, shutdown_config, info);
                        match tls_config.accept(stream).await {
                            Ok(tls_stream) => {
                                run_connection(TokioIo::new(tls_stream), service, config, peer_addr).await;
                            }
                            Err(err) => tracing::warn!("TLS handshake failed from {}: {}", peer_addr, err),
                        }
                    });
                }
            }
        }

        accept_workers.tasks.abort_all();
        while let Some(joined) = accept_workers.tasks.join_next().await {
            log_join_result(Some(joined));
        }
        drain_connections(connections, shutdown_config).await;
        Ok(())
    }

    #[cfg(feature = "native-tls")]
    async fn accept_native_tls<F>(
        self,
        service: BoxCloneService,
        tls_config: arvik_tls::native::NativeTlsConfig,
        signal: F,
        shutdown_config: ShutdownConfig,
    ) -> Result<(), BoxError>
    where
        F: Future<Output = ()> + Send,
    {
        let mut signal = Box::pin(signal);
        let mut connections = JoinSet::new();
        let mut accept_workers = spawn_accept_workers(self.listeners);
        let mut accepted = accept_workers.receiver;
        let active = Arc::new(AtomicUsize::new(0));

        loop {
            tokio::select! {
                biased;

                _ = &mut signal => {
                    tracing::info!("Shutdown signal received; stopping native-tls listener on {}", self.addr);
                    break;
                }
                joined = connections.join_next(), if !connections.is_empty() => {
                    log_join_result(joined);
                }
                next = accepted.recv() => {
                    let Some(next) = next else {
                        break;
                    };
                    let (stream, peer_addr) = next?;
                    apply_stream_options(&stream, &self.config)?;
                    let info = ConnectionInfo { local_addr: self.addr, peer_addr };
                    if !try_admit_connection(&self.config, &active, info) {
                        drop(stream);
                        continue;
                    }

                    let service = service.clone();
                    let tls_config = tls_config.clone();
                    let config = self.config.clone();
                    let shutdown_config = shutdown_config.clone();
                    let active = Arc::clone(&active);

                    tracing::debug!("Accepted native-tls connection from {}", peer_addr);
                    shutdown_config.call_connected(info);

                    connections.spawn(async move {
                        let _guard = ConnectionGuard::new(active, shutdown_config, info);
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

        accept_workers.tasks.abort_all();
        while let Some(joined) = accept_workers.tasks.join_next().await {
            log_join_result(Some(joined));
        }
        drain_connections(connections, shutdown_config).await;
        Ok(())
    }
}

async fn bind_listeners(addr: &str, config: &ServerConfig) -> Result<Vec<TcpListener>, BoxError> {
    if config.accept_workers_count() > 1 && !config.reuse_port_enabled() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "accept_workers greater than 1 requires reuse_port(true)",
        )
        .into());
    }

    if !config.needs_tuned_listener() {
        return Ok(vec![TcpListener::bind(addr).await?]);
    }

    let resolved = lookup_host(addr).await?.next().ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            "address resolved to no sockets",
        )
    })?;

    if config.accept_workers_count() > 1 && resolved.port() == 0 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "accept_workers greater than 1 requires an explicit non-zero port",
        )
        .into());
    }

    let mut listeners = Vec::with_capacity(config.accept_workers_count());
    for _ in 0..config.accept_workers_count() {
        listeners.push(bind_socket(resolved, config)?);
    }
    Ok(listeners)
}

fn bind_socket(addr: SocketAddr, config: &ServerConfig) -> io::Result<TcpListener> {
    let socket = Socket::new(Domain::for_address(addr), Type::STREAM, Some(Protocol::TCP))?;
    socket.set_nonblocking(true)?;

    if config.reuse_address_enabled() {
        socket.set_reuse_address(true)?;
    }

    if config.reuse_port_enabled() {
        set_reuse_port(&socket)?;
    }

    if let Some(size) = config.socket_recv_buffer_size_value() {
        socket.set_recv_buffer_size(size)?;
    }
    if let Some(size) = config.socket_send_buffer_size_value() {
        socket.set_send_buffer_size(size)?;
    }

    socket.bind(&addr.into())?;
    socket.listen(config.backlog_size().unwrap_or(1024))?;

    let std_listener: std::net::TcpListener = socket.into();
    TcpListener::from_std(std_listener)
}

#[cfg(unix)]
fn set_reuse_port(socket: &Socket) -> io::Result<()> {
    socket.set_reuse_port(true)
}

#[cfg(not(unix))]
fn set_reuse_port(_socket: &Socket) -> io::Result<()> {
    Err(io::Error::new(
        io::ErrorKind::Unsupported,
        "SO_REUSEPORT is not supported on this platform",
    ))
}

struct AcceptWorkers {
    receiver: mpsc::Receiver<io::Result<(TcpStream, SocketAddr)>>,
    tasks: JoinSet<()>,
}

fn spawn_accept_workers(listeners: Vec<TcpListener>) -> AcceptWorkers {
    let (sender, receiver) = mpsc::channel(1024);
    let mut tasks = JoinSet::new();

    for listener in listeners {
        let sender = sender.clone();
        tasks.spawn(async move {
            loop {
                let accepted = listener.accept().await;
                if sender.send(accepted).await.is_err() {
                    break;
                }
            }
        });
    }

    drop(sender);
    AcceptWorkers { receiver, tasks }
}

fn apply_stream_options(stream: &TcpStream, config: &ServerConfig) -> io::Result<()> {
    if let Some(enabled) = config.tcp_nodelay_setting() {
        stream.set_nodelay(enabled)?;
    }

    if config.tcp_keepalive_duration().is_some()
        || config.tcp_keepalive_interval_duration().is_some()
        || config.tcp_keepalive_retries_count().is_some()
    {
        let mut keepalive = TcpKeepalive::new();
        if let Some(duration) = config.tcp_keepalive_duration() {
            keepalive = keepalive.with_time(duration);
        }
        if let Some(interval) = config.tcp_keepalive_interval_duration() {
            keepalive = keepalive.with_interval(interval);
        }
        if let Some(retries) = config.tcp_keepalive_retries_count() {
            keepalive = with_keepalive_retries(keepalive, retries)?;
        }
        socket2::SockRef::from(stream).set_tcp_keepalive(&keepalive)?;
    }

    Ok(())
}

#[cfg(any(target_os = "linux", target_os = "android"))]
fn with_keepalive_retries(keepalive: TcpKeepalive, retries: u32) -> io::Result<TcpKeepalive> {
    Ok(keepalive.with_retries(retries))
}

#[cfg(not(any(target_os = "linux", target_os = "android")))]
fn with_keepalive_retries(_keepalive: TcpKeepalive, _retries: u32) -> io::Result<TcpKeepalive> {
    Err(io::Error::new(
        io::ErrorKind::Unsupported,
        "TCP keepalive retry tuning is not supported on this platform",
    ))
}

fn try_admit_connection(config: &ServerConfig, active: &AtomicUsize, info: ConnectionInfo) -> bool {
    if let Some(max) = config.max_connections_limit() {
        let mut current = active.load(Ordering::Acquire);
        loop {
            if current >= max {
                tracing::warn!(
                    peer_addr = %info.peer_addr,
                    max_connections = max,
                    "Connection admission limit reached"
                );
                return false;
            }

            match active.compare_exchange_weak(
                current,
                current + 1,
                Ordering::AcqRel,
                Ordering::Acquire,
            ) {
                Ok(_) => return true,
                Err(next) => current = next,
            }
        }
    }

    active.fetch_add(1, Ordering::AcqRel);
    true
}

struct ConnectionGuard {
    active: Arc<AtomicUsize>,
    shutdown_config: ShutdownConfig,
    info: ConnectionInfo,
}

impl ConnectionGuard {
    fn new(
        active: Arc<AtomicUsize>,
        shutdown_config: ShutdownConfig,
        info: ConnectionInfo,
    ) -> Self {
        Self {
            active,
            shutdown_config,
            info,
        }
    }
}

impl Drop for ConnectionGuard {
    fn drop(&mut self) {
        self.active.fetch_sub(1, Ordering::AcqRel);
        self.shutdown_config.call_disconnected(self.info);
    }
}

async fn drain_connections(mut connections: JoinSet<()>, shutdown_config: ShutdownConfig) {
    if connections.is_empty() {
        return;
    }

    let timeout = shutdown_config.drain_timeout_value();
    let sleep = tokio::time::sleep(timeout);
    tokio::pin!(sleep);

    loop {
        tokio::select! {
            _ = &mut sleep => {
                let remaining = connections.len();
                if remaining > 0 {
                    tracing::warn!(
                        remaining_connections = remaining,
                        ?timeout,
                        "Graceful shutdown drain timeout elapsed; aborting remaining connections"
                    );
                    connections.abort_all();
                    while let Some(joined) = connections.join_next().await {
                        log_join_result(Some(joined));
                    }
                }
                break;
            }
            joined = connections.join_next() => {
                match joined {
                    Some(result) => log_join_result(Some(result)),
                    None => break,
                }
            }
        }
    }
}

fn log_join_result(result: Option<Result<(), tokio::task::JoinError>>) {
    if let Some(Err(err)) = result {
        if err.is_cancelled() {
            tracing::debug!("Connection task cancelled during shutdown");
        } else {
            tracing::error!("Connection task failed: {}", err);
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

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn default_bind_path_works() {
        let Some(server) = bind_or_skip(Server::bind("127.0.0.1:0").await) else {
            return;
        };
        assert_ne!(server.local_addr().port(), 0);
        assert_eq!(server.config().accept_workers_count(), 1);
    }

    #[tokio::test]
    async fn tuned_bind_path_works() {
        let config = ServerConfig::new()
            .reuse_address(true)
            .backlog(128)
            .socket_recv_buffer_size(64 * 1024)
            .socket_send_buffer_size(64 * 1024)
            .tcp_nodelay(true);

        let Some(server) = bind_or_skip(Server::bind_with_config("127.0.0.1:0", config).await)
        else {
            return;
        };
        assert_ne!(server.local_addr().port(), 0);
    }

    #[tokio::test]
    async fn accept_workers_requires_reuse_port() {
        let err =
            match Server::bind_with_config("127.0.0.1:0", ServerConfig::new().accept_workers(2))
                .await
            {
                Ok(_) => panic!("bind should reject accept_workers without reuse_port"),
                Err(err) => err,
            };

        assert!(err.to_string().contains("requires reuse_port"));
    }

    #[tokio::test]
    async fn accepted_stream_options_are_applied() {
        let listener = match TcpListener::bind("127.0.0.1:0").await {
            Ok(listener) => listener,
            Err(err) if err.kind() == io::ErrorKind::PermissionDenied => return,
            Err(err) => panic!("bind failed: {err}"),
        };
        let addr = listener.local_addr().unwrap();
        let client = TcpStream::connect(addr);
        let accepted = listener.accept();
        let (_client, accepted) = tokio::join!(client, accepted);
        let (server, _) = accepted.unwrap();

        apply_stream_options(&server, &ServerConfig::new().tcp_nodelay(true)).unwrap();
        assert!(server.nodelay().unwrap());
    }

    fn bind_or_skip(result: Result<Server, BoxError>) -> Option<Server> {
        match result {
            Ok(server) => Some(server),
            Err(err)
                if err
                    .downcast_ref::<io::Error>()
                    .is_some_and(|err| err.kind() == io::ErrorKind::PermissionDenied) =>
            {
                None
            }
            Err(err) => panic!("bind failed: {err}"),
        }
    }
}
