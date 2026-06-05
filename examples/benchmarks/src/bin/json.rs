use arvik::{Json, Router, get, serve_app};
use serde::Serialize;

#[derive(Serialize)]
struct Message {
    message: &'static str,
}

async fn json() -> Json<Message> {
    Json(Message {
        message: "Hello, World!",
    })
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let app = Router::new().route("/json", get(json));
    serve_app("0.0.0.0:8080", app).await
}
