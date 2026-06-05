use arvik::{Path, Router, serve_app};

#[arvik::get("/")]
async fn home() -> &'static str {
    "Open http://127.0.0.1:8080/users/42"
}

#[arvik::get("/users/{id}")]
async fn get_user(Path(id): Path<u64>) -> String {
    format!("User #{id}")
}

#[arvik::post("/users")]
async fn create_user(body: String) -> String {
    format!("Created user from payload: {body}")
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    tracing_subscriber::fmt().init();

    let app: Router<()> =
        Router::new().routes(arvik::collect_routes![home, get_user, create_user]);

    serve_app("0.0.0.0:8080", app).await
}
