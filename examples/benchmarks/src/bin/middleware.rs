use std::time::Duration;

use arvik::{
    CompressionLayer, CorsLayer, RateLimitLayer, RequestIdLayer, Router, SecurityHeadersLayer,
    TraceLayer, RuntimeConfig, ServerConfig, get, serve_with_config,
};

async fn payload() -> &'static str {
    "Arvik benchmark response body repeated enough to cross compression thresholds. \
     Arvik benchmark response body repeated enough to cross compression thresholds."
}

fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    RuntimeConfig::benchmark_http1().build()?.block_on(async {
    let scenario = std::env::var("BENCH_MIDDLEWARE_SCENARIO")
        .unwrap_or_else(|_| "full".to_string())
        .to_ascii_lowercase();

    match scenario.as_str() {
        "none" => serve_benchmark(base_app()).await,
        "request_id" => {
            serve_benchmark(base_app().layer(RequestIdLayer::new()))
            .await
        }
        "headers" => {
            serve_benchmark(base_app().layer(SecurityHeadersLayer::new()))
            .await
        }
        "tracing" => {
            serve_benchmark(base_app().layer(TraceLayer::new_for_http()))
            .await
        }
        "cors" => {
            serve_benchmark(base_app().layer(CorsLayer::permissive())).await
        }
        "rate_limit" => {
            serve_benchmark(
                base_app().layer(RateLimitLayer::new(1_000_000, Duration::from_secs(1)).global()),
            )
            .await
        }
        "compression" => {
            serve_benchmark(base_app().layer(CompressionLayer::new().min_size(64)))
            .await
        }
        "full" => {
            serve_benchmark(full_app()).await
        }
        other => {
            eprintln!("unknown BENCH_MIDDLEWARE_SCENARIO={other}");
            std::process::exit(2);
        }
    }
    })
}

fn base_app() -> Router {
    Router::new().route("/middleware", get(payload))
}

fn full_app() -> Router {
    base_app()
        .layer(CompressionLayer::new().min_size(64))
        .layer(SecurityHeadersLayer::new())
        .layer(TraceLayer::new_for_http())
        .layer(RequestIdLayer::new())
        .layer(RateLimitLayer::new(1_000_000, Duration::from_secs(1)).global())
        .layer(CorsLayer::permissive())
}

async fn serve_benchmark(
    app: Router,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    serve_with_config(app, "0.0.0.0:8080", ServerConfig::benchmark_http1()).await
}
