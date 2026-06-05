use arvik::{Json, Path, Router, get, serve_app};
use serde::Serialize;

#[derive(Serialize)]
struct User {
    id: u64,
}

async fn user(Path(id): Path<u64>) -> Json<User> {
    Json(User { id })
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let app = Router::new().route("/users/{id}", get(user));
    serve_app("0.0.0.0:8080", app).await
}
