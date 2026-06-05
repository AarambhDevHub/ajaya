//! Observability integrations for Arvik.
//!
//! This crate is intentionally feature gated. Enable `metrics`,
//! `opentelemetry`, or `health` through the `arvik` facade.

#[cfg(feature = "health")]
pub mod health;
#[cfg(feature = "logging")]
pub mod logging;
#[cfg(feature = "metrics")]
pub mod metrics;
#[cfg(feature = "opentelemetry")]
pub mod trace;
