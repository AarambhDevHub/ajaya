use arvik::{Json, Router, RuntimeConfig, ServerConfig, State, get, serve_with_config};
use serde::Serialize;

#[derive(Clone)]
struct BenchDb {
    database_url: Option<String>,
}

#[derive(Serialize)]
struct Row {
    id: u64,
    source: &'static str,
}

async fn single_query(State(db): State<BenchDb>) -> Json<Row> {
    Json(Row {
        id: 1,
        source: if db.database_url.is_some() {
            "database-url-configured"
        } else {
            "in-memory"
        },
    })
}

fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    RuntimeConfig::benchmark_http1().build()?.block_on(async {
        let state = BenchDb {
            database_url: std::env::var("DATABASE_URL").ok(),
        };
        let app = Router::new()
            .route("/db", get(single_query))
            .with_state(state);
        serve_with_config(app, "0.0.0.0:8080", ServerConfig::benchmark_http1()).await
    })
}
