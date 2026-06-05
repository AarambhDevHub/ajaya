use axum::{Json, Router, routing::get};
use serde::Serialize;

#[derive(Serialize)]
struct Message {
    message: &'static str,
}

async fn plaintext() -> &'static str {
    "Hello, World!"
}

async fn json() -> Json<Message> {
    Json(Message {
        message: "Hello, World!",
    })
}

#[tokio::main]
async fn main() {
    let app = Router::new()
        .route("/plaintext", get(plaintext))
        .route("/json", get(json));
    let listener = tokio::net::TcpListener::bind("0.0.0.0:8081")
        .await
        .unwrap();
    axum::serve(listener, app).await.unwrap();
}
