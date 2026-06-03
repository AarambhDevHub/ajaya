#![cfg(feature = "http2")]

use arvik_hyper::{Server, ServerConfig};
use arvik_router::{Router, get};
use bytes::Bytes;
use http::Version;
use http_body_util::{BodyExt as _, Empty};
use hyper_util::rt::{TokioExecutor, TokioIo};
use tokio::net::TcpStream;

async fn hello() -> &'static str {
    "h2c ok"
}

#[tokio::test]
async fn h2c_serves_http2_prior_knowledge_requests() {
    let server = match Server::bind_with_config(
        "127.0.0.1:0",
        ServerConfig::new()
            .http2_only(true)
            .http2_max_concurrent_streams(128),
    )
    .await
    {
        Ok(server) => server,
        Err(err) if is_permission_denied(err.as_ref()) => {
            eprintln!("skipping h2c socket test: local sockets are not permitted");
            return;
        }
        Err(err) => panic!("failed to bind h2c test server: {err}"),
    };
    let addr = server.local_addr();
    let app = Router::new().route("/", get(hello));

    let server_task = tokio::spawn(async move {
        let _ = server.serve_app(app).await;
    });

    let stream = match TcpStream::connect(addr).await {
        Ok(stream) => stream,
        Err(err) if err.kind() == std::io::ErrorKind::PermissionDenied => {
            eprintln!("skipping h2c socket test: local sockets are not permitted");
            server_task.abort();
            return;
        }
        Err(err) => panic!("failed to connect to h2c test server: {err}"),
    };
    let io = TokioIo::new(stream);
    let (mut sender, conn) = hyper::client::conn::http2::handshake(TokioExecutor::new(), io)
        .await
        .unwrap();
    let conn_task = tokio::spawn(conn);

    let req = http::Request::builder()
        .uri("/")
        .body(Empty::<Bytes>::new())
        .unwrap();
    let res = sender.send_request(req).await.unwrap();

    assert_eq!(res.version(), Version::HTTP_2);
    let body = res.into_body().collect().await.unwrap().to_bytes();
    assert_eq!(body, Bytes::from_static(b"h2c ok"));

    conn_task.abort();
    server_task.abort();
}

fn is_permission_denied(err: &(dyn std::error::Error + Send + Sync + 'static)) -> bool {
    err.downcast_ref::<std::io::Error>()
        .is_some_and(|err| err.kind() == std::io::ErrorKind::PermissionDenied)
}
