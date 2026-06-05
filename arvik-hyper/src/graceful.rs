//! Graceful shutdown and connection lifecycle types.

use std::fmt;
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;

/// Information about an accepted connection.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ConnectionInfo {
    /// The local listener address.
    pub local_addr: SocketAddr,
    /// The peer address reported by the TCP listener.
    pub peer_addr: SocketAddr,
}

type ConnectionHook = Arc<dyn Fn(ConnectionInfo) + Send + Sync + 'static>;

/// Graceful shutdown and connection lifecycle configuration.
#[derive(Clone)]
pub struct ShutdownConfig {
    drain_timeout: Duration,
    on_connected: Option<ConnectionHook>,
    on_disconnected: Option<ConnectionHook>,
}

impl Default for ShutdownConfig {
    fn default() -> Self {
        Self {
            drain_timeout: Duration::from_secs(30),
            on_connected: None,
            on_disconnected: None,
        }
    }
}

impl ShutdownConfig {
    /// Create a default shutdown config.
    pub fn new() -> Self {
        Self::default()
    }

    /// Set how long the server waits for accepted connections to complete
    /// after the shutdown signal fires.
    #[must_use]
    pub fn drain_timeout(mut self, timeout: Duration) -> Self {
        self.drain_timeout = timeout;
        self
    }

    /// Run a hook after a connection is accepted and admitted.
    #[must_use]
    pub fn on_connected<F>(mut self, hook: F) -> Self
    where
        F: Fn(ConnectionInfo) + Send + Sync + 'static,
    {
        self.on_connected = Some(Arc::new(hook));
        self
    }

    /// Run a hook after a connection task finishes.
    #[must_use]
    pub fn on_disconnected<F>(mut self, hook: F) -> Self
    where
        F: Fn(ConnectionInfo) + Send + Sync + 'static,
    {
        self.on_disconnected = Some(Arc::new(hook));
        self
    }

    /// Return the configured drain timeout.
    pub fn drain_timeout_value(&self) -> Duration {
        self.drain_timeout
    }

    pub(crate) fn call_connected(&self, info: ConnectionInfo) {
        if let Some(hook) = &self.on_connected {
            hook(info);
        }
    }

    pub(crate) fn call_disconnected(&self, info: ConnectionInfo) {
        if let Some(hook) = &self.on_disconnected {
            hook(info);
        }
    }
}

impl fmt::Debug for ShutdownConfig {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ShutdownConfig")
            .field("drain_timeout", &self.drain_timeout)
            .field(
                "on_connected",
                &self.on_connected.as_ref().map(|_| "<hook>"),
            )
            .field(
                "on_disconnected",
                &self.on_disconnected.as_ref().map(|_| "<hook>"),
            )
            .finish()
    }
}

/// Wait for the default platform shutdown signal.
pub async fn default_shutdown_signal() {
    #[cfg(unix)]
    {
        use tokio::signal::unix::{SignalKind, signal};

        match signal(SignalKind::terminate()) {
            Ok(mut terminate) => {
                tokio::select! {
                    _ = tokio::signal::ctrl_c() => {}
                    _ = terminate.recv() => {}
                }
            }
            Err(_) => {
                let _ = tokio::signal::ctrl_c().await;
            }
        }
    }

    #[cfg(not(unix))]
    {
        let _ = tokio::signal::ctrl_c().await;
    }
}
