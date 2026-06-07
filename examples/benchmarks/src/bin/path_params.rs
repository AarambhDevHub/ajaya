use arvik::{Json, Path, Router, RuntimeConfig, ServerConfig, get, serve_with_config};
use serde::Serialize;

#[derive(Serialize)]
struct User {
    id: u64,
}

async fn user(Path(id): Path<u64>) -> Json<User> {
    Json(User { id })
}

fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    RuntimeConfig::benchmark_http1().build()?.block_on(async {
        let app = Router::new().route("/users/{id}", get(user));
        serve_with_config(app, "0.0.0.0:8080", ServerConfig::benchmark_http1()).await
    })
}
