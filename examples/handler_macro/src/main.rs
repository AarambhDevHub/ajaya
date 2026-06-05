use arvik::{Request, Router, get, serve_app};

#[derive(Clone)]
struct RequestInfo {
    label: &'static str,
}

#[arvik::handler]
impl RequestInfo {
    async fn call(&self, req: Request) -> String {
        format!("{} {} {}", self.label, req.method(), req.uri().path())
    }
}

async fn home() -> &'static str {
    "Open http://127.0.0.1:8080/inspect"
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    tracing_subscriber::fmt().init();

    let app: Router<()> = Router::new()
        .route("/", get(home))
        .route("/inspect", get(RequestInfo { label: "request" }));

    serve_app("0.0.0.0:8080", app).await
}
