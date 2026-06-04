use arvik::{Router, State, debug_handler, get, post, serve_app};

#[derive(Clone)]
struct AppState {
    name: &'static str,
}

async fn home() -> &'static str {
    "POST plain text to http://127.0.0.1:8080/echo"
}

#[debug_handler(state = AppState)]
async fn echo(State(state): State<AppState>, body: String) -> String {
    format!("{}: {body}", state.name)
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    tracing_subscriber::fmt().init();

    let app = Router::new()
        .route("/", get(home))
        .route("/echo", post(echo))
        .with_state(AppState { name: "Arvik" });

    serve_app("0.0.0.0:8080", app).await
}
