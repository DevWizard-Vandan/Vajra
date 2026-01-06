//! Telemetry and observability infrastructure for Vajra.
//!
//! This module provides the observability stack:
//! - Structured JSON logging via `tracing-subscriber`
//! - Distributed tracing via `tracing-opentelemetry` and `opentelemetry-otlp`
//! - Prometheus metrics via `metrics-exporter-prometheus`
//!
//! # Example
//!
//! ```no_run
//! use vajra_common::telemetry::{TelemetryConfig, init_telemetry, shutdown_telemetry};
//!
//! #[tokio::main]
//! async fn main() {
//!     let config = TelemetryConfig::default();
//!     init_telemetry(&config).expect("Failed to initialize telemetry");
//!     
//!     tracing::info!("Vajra starting up");
//!     
//!     // ... application code ...
//!     
//!     shutdown_telemetry();
//! }
//! ```

pub use crate::config::TelemetryConfig;
use crate::VajraError;
use std::sync::OnceLock;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt, EnvFilter, Layer};

/// Global telemetry state for cleanup
static TELEMETRY_INITIALIZED: OnceLock<bool> = OnceLock::new();

/// Initialize the telemetry stack.
///
/// This sets up:
/// 1. Structured JSON logs to stdout (or pretty logs if `json_logs` is false)
/// 2. OpenTelemetry tracing exporter (if `otlp_endpoint` is configured)
/// 3. Prometheus metrics exporter
///
/// # Errors
///
/// Returns an error if:
/// - Telemetry has already been initialized
/// - Failed to create the OTLP exporter
/// - Failed to set up the tracing subscriber
///
/// # Panics
///
/// This function will panic if called more than once.
pub fn init_telemetry(config: &TelemetryConfig) -> Result<(), VajraError> {
    // Prevent double initialization
    if TELEMETRY_INITIALIZED.get().is_some() {
        return Err(VajraError::Configuration {
            message: "Telemetry already initialized".into(),
        });
    }

    // Build the env filter from config
    let env_filter =
        EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new(&config.log_level));

    // Create the logging layer
    let fmt_layer = if config.json_logs {
        tracing_subscriber::fmt::layer()
            .json()
            .with_target(true)
            .with_thread_ids(true)
            .with_file(true)
            .with_line_number(true)
            .boxed()
    } else {
        tracing_subscriber::fmt::layer()
            .with_target(true)
            .with_thread_ids(true)
            .boxed()
    };

    // Build the subscriber
    let subscriber = tracing_subscriber::registry()
        .with(env_filter)
        .with(fmt_layer);

    // Initialize OpenTelemetry if endpoint is configured
    if let Some(ref endpoint) = config.otlp_endpoint {
        init_opentelemetry(config, endpoint)?;

        // Add the OpenTelemetry layer
        let telemetry_layer = tracing_opentelemetry::layer();
        subscriber.with(telemetry_layer).init();
    } else {
        subscriber.init();
    }

    // Initialize Prometheus metrics
    init_prometheus_metrics()?;

    let _ = TELEMETRY_INITIALIZED.set(true);

    tracing::info!(
        service = %config.service_name,
        log_level = %config.log_level,
        json_logs = config.json_logs,
        otlp_enabled = config.otlp_endpoint.is_some(),
        "Telemetry initialized"
    );

    Ok(())
}

/// Initialize OpenTelemetry with OTLP exporter.
fn init_opentelemetry(config: &TelemetryConfig, endpoint: &str) -> Result<(), VajraError> {
    use opentelemetry_otlp::WithExportConfig;
    use opentelemetry_sdk::trace::Config;

    use opentelemetry_sdk::Resource;

    let resource = Resource::new(vec![opentelemetry::KeyValue::new(
        "service.name",
        config.service_name.clone(),
    )]);

    let tracer_provider = opentelemetry_otlp::new_pipeline()
        .tracing()
        .with_exporter(
            opentelemetry_otlp::new_exporter()
                .tonic()
                .with_endpoint(endpoint),
        )
        .with_trace_config(Config::default().with_resource(resource))
        .install_batch(opentelemetry_sdk::runtime::Tokio)
        .map_err(|e| VajraError::Configuration {
            message: format!("Failed to initialize OpenTelemetry: {e}"),
        })?;

    // The pipeline install_batch returns a Tracer, and also registers the global provider.
    // We don't need to explicitly create another tracer here.
    let _ = tracer_provider;

    Ok(())
}

/// Initialize Prometheus metrics exporter.
fn init_prometheus_metrics() -> Result<(), VajraError> {
    // Register the Prometheus exporter
    let builder = metrics_exporter_prometheus::PrometheusBuilder::new();

    builder.install().map_err(|e| VajraError::Configuration {
        message: format!("Failed to initialize Prometheus metrics: {e}"),
    })?;

    // Register standard metrics
    register_standard_metrics();

    Ok(())
}

/// Register standard Vajra metrics.
fn register_standard_metrics() {
    use metrics::{describe_counter, describe_gauge, describe_histogram, Unit};

    // Engine metrics
    describe_gauge!(
        "vajra_vectors_total",
        Unit::Count,
        "Total vectors in the index"
    );
    describe_gauge!(
        "vajra_vectors_deleted",
        Unit::Count,
        "Soft-deleted vectors pending compaction"
    );
    describe_histogram!(
        "vajra_search_duration_seconds",
        Unit::Seconds,
        "Search operation latency"
    );
    describe_histogram!(
        "vajra_insert_duration_seconds",
        Unit::Seconds,
        "Insert operation latency"
    );

    // Raft metrics
    describe_gauge!("vajra_raft_term", Unit::Count, "Current Raft term");
    describe_gauge!(
        "vajra_raft_commit_index",
        Unit::Count,
        "Committed log index"
    );
    describe_gauge!("vajra_raft_applied_index", Unit::Count, "Applied log index");
    describe_gauge!(
        "vajra_raft_is_leader",
        Unit::Count,
        "1 if this node is the leader, 0 otherwise"
    );
    describe_counter!(
        "vajra_raft_elections_total",
        Unit::Count,
        "Total elections started"
    );
    describe_histogram!(
        "vajra_raft_append_entries_duration_seconds",
        Unit::Seconds,
        "AppendEntries RPC latency"
    );

    // WAL metrics
    describe_gauge!(
        "vajra_wal_size_bytes",
        Unit::Bytes,
        "Total WAL size on disk"
    );
    describe_counter!(
        "vajra_wal_writes_total",
        Unit::Count,
        "Total entries written to WAL"
    );
    describe_counter!(
        "vajra_wal_fsync_total",
        Unit::Count,
        "Total fsync operations"
    );

    // Network metrics
    describe_counter!(
        "vajra_grpc_requests_total",
        Unit::Count,
        "Total gRPC requests received"
    );
    describe_histogram!(
        "vajra_grpc_request_duration_seconds",
        Unit::Seconds,
        "gRPC request latency"
    );
    describe_counter!("vajra_grpc_errors_total", Unit::Count, "Total gRPC errors");
}

/// Shutdown the telemetry stack gracefully.
///
/// This flushes any pending traces and metrics before shutdown.
pub fn shutdown_telemetry() {
    tracing::info!("Shutting down telemetry");

    // Shutdown OpenTelemetry
    opentelemetry::global::shutdown_tracer_provider();

    tracing::info!("Telemetry shutdown complete");
}

/// Create a new trace span with a trace ID.
///
/// This creates a span compatible with W3C Trace Context format.
#[macro_export]
macro_rules! trace_span {
    ($name:expr) => {
        tracing::info_span!($name)
    };
    ($name:expr, $($field:tt)*) => {
        tracing::info_span!($name, $($field)*)
    };
}

/// Record a metric increment.
#[inline]
pub fn increment_counter(name: &'static str, value: u64, labels: &[(&'static str, String)]) {
    let labels: Vec<metrics::Label> = labels
        .iter()
        .map(|(k, v)| metrics::Label::new(*k, v.clone()))
        .collect();
    metrics::counter!(name, labels).increment(value);
}

/// Record a gauge value.
#[inline]
pub fn set_gauge(name: &'static str, value: f64, labels: &[(&'static str, String)]) {
    let labels: Vec<metrics::Label> = labels
        .iter()
        .map(|(k, v)| metrics::Label::new(*k, v.clone()))
        .collect();
    metrics::gauge!(name, labels).set(value);
}

/// Record a histogram observation.
#[inline]
pub fn record_histogram(name: &'static str, value: f64, labels: &[(&'static str, String)]) {
    let labels: Vec<metrics::Label> = labels
        .iter()
        .map(|(k, v)| metrics::Label::new(*k, v.clone()))
        .collect();
    metrics::histogram!(name, labels).record(value);
}

/// A guard that records the duration of an operation when dropped.
pub struct TimingGuard {
    name: &'static str,
    start: std::time::Instant,
    labels: Vec<(&'static str, String)>,
}

impl TimingGuard {
    /// Create a new timing guard.
    #[must_use]
    pub fn new(name: &'static str, labels: Vec<(&'static str, String)>) -> Self {
        Self {
            name,
            start: std::time::Instant::now(),
            labels,
        }
    }
}

impl Drop for TimingGuard {
    fn drop(&mut self) {
        let duration = self.start.elapsed();
        record_histogram(self.name, duration.as_secs_f64(), &self.labels);
    }
}

/// Start timing an operation.
///
/// Returns a guard that records the duration when dropped.
#[must_use]
pub fn start_timer(name: &'static str) -> TimingGuard {
    TimingGuard::new(name, vec![])
}

/// Start timing an operation with labels.
///
/// Returns a guard that records the duration when dropped.
#[must_use]
pub fn start_timer_with_labels(
    name: &'static str,
    labels: Vec<(&'static str, String)>,
) -> TimingGuard {
    TimingGuard::new(name, labels)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_telemetry_config_default() {
        let config = TelemetryConfig::default();
        assert_eq!(config.service_name, "vajra");
        assert!(config.otlp_endpoint.is_none());
        assert!(config.json_logs);
    }

    #[test]
    fn test_timing_guard() {
        let guard = start_timer("test_metric");
        std::thread::sleep(std::time::Duration::from_millis(10));
        drop(guard);
        // Metric should have been recorded (no assertion, just verifying no panic)
    }
}
