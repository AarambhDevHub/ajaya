use arvik::{Json, Query, Router, RuntimeConfig, ServerConfig, State, get, serve_with_config};
use serde::{Deserialize, Serialize};

#[derive(Clone)]
struct BenchDb {
    database_url: Option<String>,
}

#[derive(Deserialize)]
struct Params {
    queries: Option<usize>,
}

#[derive(Serialize)]
struct Row {
    id: usize,
    source: &'static str,
}

async fn multiple_queries(Query(params): Query<Params>, State(db): State<BenchDb>) -> Json<Vec<Row>> {
    let count = params.queries.unwrap_or(20).clamp(1, 500);
    let source = if db.database_url.is_some() {
        "database-url-configured"
    } else {
        "in-memory"
    };
    Json((1..=count).map(|id| Row { id, source }).collect())
}

fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    RuntimeConfig::benchmark_http1().build()?.block_on(async {
        let state = BenchDb {
            database_url: std::env::var("DATABASE_URL").ok(),
        };
        let app = Router::new()
            .route("/queries", get(multiple_queries))
            .with_state(state);
        serve_with_config(app, "0.0.0.0:8080", ServerConfig::benchmark_http1()).await
    })
}
