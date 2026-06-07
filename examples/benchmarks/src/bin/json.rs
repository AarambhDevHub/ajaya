use arvik::{Json, Router, RuntimeConfig, ServerConfig, get, serve_with_config};
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

fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    RuntimeConfig::benchmark_http1().build()?.block_on(async {
        let app = Router::new().route("/json", get(json));
        serve_with_config(app, "0.0.0.0:8080", ServerConfig::benchmark_http1()).await
    })
}
