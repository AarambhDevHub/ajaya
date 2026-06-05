use std::time::Duration;

use arvik::{RuntimeConfig, ServerConfig, get, serve_with_config};

async fn plaintext() -> &'static str {
    "Hello, Arvik"
}

fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let runtime = RuntimeConfig::new()
        .worker_threads(
            std::thread::available_parallelism()
                .map(usize::from)
                .unwrap_or(1),
        )
        .max_blocking_threads(512)
        .thread_name("arvik-worker")
        .event_interval(61)
        .global_queue_interval(61)
        .max_io_events_per_tick(1024)
        .build()?;

    runtime.block_on(async {
        let app = arvik::Router::new().route("/", get(plaintext));

        let config = tuned_server_config();
        serve_with_config(app, "0.0.0.0:8080", config).await
    })
}

fn tuned_server_config() -> ServerConfig {
    let config = ServerConfig::http2_high_throughput()
        .tcp_keepalive(Duration::from_secs(60))
        .tcp_keepalive_interval(Duration::from_secs(10))
        .reuse_address(true)
        .backlog(4096)
        .socket_recv_buffer_size(512 * 1024)
        .socket_send_buffer_size(512 * 1024)
        .max_connections(10_000);

    #[cfg(unix)]
    {
        config.reuse_port(true).accept_workers_per_cpu()
    }

    #[cfg(not(unix))]
    {
        config
    }
}
