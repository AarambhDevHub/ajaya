use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;

use arvik_hyper::{Server, ShutdownConfig};
use arvik_router::{Router, get};
use tokio::io::{AsyncReadExt, AsyncWriteExt};

async fn bind_test_server() -> Option<Server> {
    match Server::bind("127.0.0.1:0").await {
        Ok(server) => Some(server),
        Err(err) if is_permission_denied(err.as_ref()) => {
            eprintln!("skipping graceful shutdown socket test: local sockets are not permitted");
            None
        }
        Err(err) => panic!("failed to bind test server: {err}"),
    }
}

async fn raw_get(addr: std::net::SocketAddr, path: &str) -> String {
    let mut stream = tokio::net::TcpStream::connect(addr).await.unwrap();
    let request = format!("GET {path} HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n\r\n");
    stream.write_all(request.as_bytes()).await.unwrap();

    let mut response = String::new();
    stream.read_to_string(&mut response).await.unwrap();
    response
}

#[tokio::test]
async fn graceful_shutdown_stops_accepting_after_signal() {
    let app = Router::new().route("/", get(|| async { "ok" }));
    let Some(server) = bind_test_server().await else {
        return;
    };
    let addr = server.local_addr();
    let (shutdown_tx, shutdown_rx) = tokio::sync::oneshot::channel();

    let task = tokio::spawn(async move {
        server
            .serve_app_with_graceful_shutdown(
                app,
                async {
                    let _ = shutdown_rx.await;
                },
                ShutdownConfig::default().drain_timeout(Duration::from_secs(2)),
            )
            .await
    });

    let response = raw_get(addr, "/").await;
    assert!(response.contains("200 OK"));
    assert!(response.ends_with("ok"));

    shutdown_tx.send(()).unwrap();
    task.await.unwrap().unwrap();

    assert!(tokio::net::TcpStream::connect(addr).await.is_err());
}

#[tokio::test]
async fn graceful_shutdown_waits_for_in_flight_request() {
    let entered = Arc::new(tokio::sync::Notify::new());
    let entered_route = Arc::clone(&entered);
    let app = Router::new().route(
        "/slow",
        get(move || {
            let entered = Arc::clone(&entered_route);
            async move {
                entered.notify_one();
                tokio::time::sleep(Duration::from_millis(100)).await;
                "slow"
            }
        }),
    );
    let Some(server) = bind_test_server().await else {
        return;
    };
    let addr = server.local_addr();
    let (shutdown_tx, shutdown_rx) = tokio::sync::oneshot::channel();

    let task = tokio::spawn(async move {
        server
            .serve_app_with_graceful_shutdown(
                app,
                async {
                    let _ = shutdown_rx.await;
                },
                ShutdownConfig::default().drain_timeout(Duration::from_secs(2)),
            )
            .await
    });

    let request = tokio::spawn(raw_get(addr, "/slow"));
    entered.notified().await;
    shutdown_tx.send(()).unwrap();

    let response = request.await.unwrap();
    assert!(response.contains("200 OK"));
    assert!(response.ends_with("slow"));
    task.await.unwrap().unwrap();
}

#[tokio::test]
async fn connection_hooks_fire() {
    let connected = Arc::new(AtomicUsize::new(0));
    let disconnected = Arc::new(AtomicUsize::new(0));
    let app = Router::new().route("/", get(|| async { "ok" }));
    let Some(server) = bind_test_server().await else {
        return;
    };
    let addr = server.local_addr();
    let (shutdown_tx, shutdown_rx) = tokio::sync::oneshot::channel();

    let shutdown = ShutdownConfig::default()
        .drain_timeout(Duration::from_secs(2))
        .on_connected({
            let connected = Arc::clone(&connected);
            move |_| {
                connected.fetch_add(1, Ordering::SeqCst);
            }
        })
        .on_disconnected({
            let disconnected = Arc::clone(&disconnected);
            move |_| {
                disconnected.fetch_add(1, Ordering::SeqCst);
            }
        });

    let task = tokio::spawn(async move {
        server
            .serve_app_with_graceful_shutdown(
                app,
                async {
                    let _ = shutdown_rx.await;
                },
                shutdown,
            )
            .await
    });

    let _ = raw_get(addr, "/").await;
    shutdown_tx.send(()).unwrap();
    task.await.unwrap().unwrap();

    assert_eq!(connected.load(Ordering::SeqCst), 1);
    assert_eq!(disconnected.load(Ordering::SeqCst), 1);
}

fn is_permission_denied(err: &(dyn std::error::Error + Send + Sync + 'static)) -> bool {
    if err
        .downcast_ref::<std::io::Error>()
        .is_some_and(|err| err.kind() == std::io::ErrorKind::PermissionDenied)
    {
        return true;
    }

    let mut source = err.source();
    while let Some(err) = source {
        if err
            .downcast_ref::<std::io::Error>()
            .is_some_and(|err| err.kind() == std::io::ErrorKind::PermissionDenied)
        {
            return true;
        }
        source = err.source();
    }

    false
}
