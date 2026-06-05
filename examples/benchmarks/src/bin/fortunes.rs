use arvik::{Html, Router, State, get, serve_app};

#[derive(Clone)]
struct FortuneState {
    database_url: Option<String>,
}

async fn fortunes(State(state): State<FortuneState>) -> Html<String> {
    let source = if state.database_url.is_some() {
        "database-url-configured"
    } else {
        "in-memory"
    };
    Html(format!(
        "<!doctype html><html><body><table>\
         <tr><th>id</th><th>message</th></tr>\
         <tr><td>1</td><td>Fortune favors tuned servers.</td></tr>\
         <tr><td>2</td><td>source: {source}</td></tr>\
         </table></body></html>"
    ))
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let state = FortuneState {
        database_url: std::env::var("DATABASE_URL").ok(),
    };
    let app = Router::new()
        .route("/fortunes", get(fortunes))
        .with_state(state);
    serve_app("0.0.0.0:8080", app).await
}
