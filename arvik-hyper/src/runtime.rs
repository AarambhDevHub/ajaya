//! Tokio runtime tuning helpers.

/// Tokio runtime configuration for applications that want an Arvik-provided
/// tuned runtime instead of using `#[tokio::main]`.
#[derive(Debug, Clone, Default)]
pub struct RuntimeConfig {
    worker_threads: Option<usize>,
    max_blocking_threads: Option<usize>,
    thread_name: Option<String>,
    event_interval: Option<u32>,
    global_queue_interval: Option<u32>,
    max_io_events_per_tick: Option<usize>,
}

impl RuntimeConfig {
    /// Create a default runtime configuration.
    pub fn new() -> Self {
        Self::default()
    }

    /// Set the Tokio worker thread count.
    #[must_use]
    pub fn worker_threads(mut self, threads: usize) -> Self {
        self.worker_threads = Some(threads.max(1));
        self
    }

    /// Set Tokio's maximum blocking thread count.
    #[must_use]
    pub fn max_blocking_threads(mut self, threads: usize) -> Self {
        self.max_blocking_threads = Some(threads.max(1));
        self
    }

    /// Set the runtime worker thread name prefix.
    #[must_use]
    pub fn thread_name(mut self, name: impl Into<String>) -> Self {
        self.thread_name = Some(name.into());
        self
    }

    /// Set Tokio's event interval.
    #[must_use]
    pub fn event_interval(mut self, interval: u32) -> Self {
        self.event_interval = Some(interval.max(1));
        self
    }

    /// Set Tokio's global queue interval.
    #[must_use]
    pub fn global_queue_interval(mut self, interval: u32) -> Self {
        self.global_queue_interval = Some(interval.max(1));
        self
    }

    /// Set Tokio's maximum I/O events per tick.
    #[must_use]
    pub fn max_io_events_per_tick(mut self, max: usize) -> Self {
        self.max_io_events_per_tick = Some(max.max(1));
        self
    }

    /// Return the configured worker thread count.
    pub fn worker_threads_value(&self) -> Option<usize> {
        self.worker_threads
    }

    /// Return the configured maximum blocking thread count.
    pub fn max_blocking_threads_value(&self) -> Option<usize> {
        self.max_blocking_threads
    }

    /// Return the configured thread name.
    pub fn thread_name_value(&self) -> Option<&str> {
        self.thread_name.as_deref()
    }

    /// Return the configured event interval.
    pub fn event_interval_value(&self) -> Option<u32> {
        self.event_interval
    }

    /// Return the configured global queue interval.
    pub fn global_queue_interval_value(&self) -> Option<u32> {
        self.global_queue_interval
    }

    /// Return the configured maximum I/O events per tick.
    pub fn max_io_events_per_tick_value(&self) -> Option<usize> {
        self.max_io_events_per_tick
    }

    /// Build a Tokio runtime with the configured tuning.
    pub fn build(&self) -> std::io::Result<tokio::runtime::Runtime> {
        let mut builder = tokio::runtime::Builder::new_multi_thread();
        builder.enable_all();

        if let Some(threads) = self.worker_threads {
            builder.worker_threads(threads);
        }
        if let Some(threads) = self.max_blocking_threads {
            builder.max_blocking_threads(threads);
        }
        if let Some(name) = &self.thread_name {
            builder.thread_name(name);
        }
        if let Some(interval) = self.event_interval {
            builder.event_interval(interval);
        }
        if let Some(interval) = self.global_queue_interval {
            builder.global_queue_interval(interval);
        }
        if let Some(max) = self.max_io_events_per_tick {
            builder.max_io_events_per_tick(max);
        }

        builder.build()
    }

    /// Build a Tokio runtime plus a metrics monitor.
    #[cfg(feature = "runtime-metrics")]
    pub fn build_with_metrics(
        &self,
    ) -> std::io::Result<(tokio::runtime::Runtime, RuntimeMetricsHandle)> {
        let runtime = self.build()?;
        let monitor = tokio_metrics::RuntimeMonitor::new(runtime.handle());
        Ok((runtime, RuntimeMetricsHandle { monitor }))
    }
}

/// Handle exposing Tokio runtime metrics when the `runtime-metrics` feature is enabled.
#[cfg(feature = "runtime-metrics")]
#[derive(Debug)]
pub struct RuntimeMetricsHandle {
    monitor: tokio_metrics::RuntimeMonitor,
}

#[cfg(feature = "runtime-metrics")]
impl RuntimeMetricsHandle {
    /// Return the underlying Tokio metrics monitor.
    pub fn monitor(&self) -> &tokio_metrics::RuntimeMonitor {
        &self.monitor
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stores_runtime_builder_settings() {
        let config = RuntimeConfig::new()
            .worker_threads(2)
            .max_blocking_threads(8)
            .thread_name("arvik-test")
            .event_interval(61)
            .global_queue_interval(31)
            .max_io_events_per_tick(512);

        assert_eq!(config.worker_threads_value(), Some(2));
        assert_eq!(config.max_blocking_threads_value(), Some(8));
        assert_eq!(config.thread_name_value(), Some("arvik-test"));
        assert_eq!(config.event_interval_value(), Some(61));
        assert_eq!(config.global_queue_interval_value(), Some(31));
        assert_eq!(config.max_io_events_per_tick_value(), Some(512));
    }

    #[test]
    fn builds_runtime() {
        let runtime = RuntimeConfig::new().worker_threads(1).build();
        assert!(runtime.is_ok());
    }
}
