use std::time::Duration;

use arvik::{
    CompressionLayer, CorsLayer, RateLimitLayer, RequestIdLayer, Router, SecurityHeadersLayer,
    TraceLayer, get, serve_app,
};

async fn payload() -> &'static str {
    "Arvik benchmark response body repeated enough to cross compression thresholds. \
     Arvik benchmark response body repeated enough to cross compression thresholds."
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let scenario = std::env::var("BENCH_MIDDLEWARE_SCENARIO")
        .unwrap_or_else(|_| "full".to_string())
        .to_ascii_lowercase();

    match scenario.as_str() {
        "none" => serve_app("0.0.0.0:8080", base_app()).await,
        "request_id" => {
            serve_app(
                "0.0.0.0:8080",
                base_app().layer(RequestIdLayer::new()),
            )
            .await
        }
        "headers" => {
            serve_app(
                "0.0.0.0:8080",
                base_app().layer(SecurityHeadersLayer::new()),
            )
            .await
        }
        "tracing" => {
            serve_app(
                "0.0.0.0:8080",
                base_app().layer(TraceLayer::new_for_http()),
            )
            .await
        }
        "cors" => {
            serve_app("0.0.0.0:8080", base_app().layer(CorsLayer::permissive())).await
        }
        "rate_limit" => {
            serve_app(
                "0.0.0.0:8080",
                base_app().layer(RateLimitLayer::new(1_000_000, Duration::from_secs(1)).global()),
            )
            .await
        }
        "compression" => {
            serve_app(
                "0.0.0.0:8080",
                base_app().layer(CompressionLayer::new().min_size(64)),
            )
            .await
        }
        "full" => {
            serve_app(
                "0.0.0.0:8080",
                base_app()
                    .layer(SecurityHeadersLayer::new())
                    .layer(RequestIdLayer::new())
                    .layer(CompressionLayer::new().min_size(64)),
            )
            .await
        }
        other => {
            eprintln!("unknown BENCH_MIDDLEWARE_SCENARIO={other}");
            std::process::exit(2);
        }
    }
}

fn base_app() -> Router {
    Router::new().route("/middleware", get(payload))
}
