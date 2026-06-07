use arvik::{Html, Router, RuntimeConfig, ServerConfig, State, get, serve_with_config};

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

fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    RuntimeConfig::benchmark_http1().build()?.block_on(async {
        let state = FortuneState {
            database_url: std::env::var("DATABASE_URL").ok(),
        };
        let app = Router::new()
            .route("/fortunes", get(fortunes))
            .with_state(state);
        serve_with_config(app, "0.0.0.0:8080", ServerConfig::benchmark_http1()).await
    })
}
