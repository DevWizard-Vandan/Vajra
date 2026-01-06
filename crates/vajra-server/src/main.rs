//! # Vajra Server
//!
//! The main binary entry point for the Vajra distributed vector database.
//!
//! This binary orchestrates all components:
//! - WAL recovery and state machine rebuild
//! - Raft node initialization
//! - gRPC server startup
//! - Metrics HTTP endpoint
//!
//! ## Usage
//!
//! ```bash
//! # Start with default configuration
//! vajra
//!
//! # Start with custom config file
//! vajra --config /path/to/vajra.toml
//!
//! # Start with specific node ID
//! vajra --node-id 1 --listen 0.0.0.0:50051
//! ```

use clap::Parser;
use std::path::PathBuf;
use tracing::info;
use vajra_common::config::{TelemetryConfig, VajraConfig};
use vajra_common::telemetry::{init_telemetry, shutdown_telemetry};

/// Vajra - Distributed Vector Database
#[derive(Parser, Debug)]
#[command(name = "vajra")]
#[command(author, version, about, long_about = None)]
struct Args {
    /// Path to configuration file
    #[arg(short, long, value_name = "FILE")]
    config: Option<PathBuf>,

    /// Node ID (overrides config file)
    #[arg(long)]
    node_id: Option<u64>,

    /// Listen address (overrides config file)
    #[arg(long)]
    listen: Option<String>,

    /// Metrics listen address (overrides config file)
    #[arg(long)]
    metrics_listen: Option<String>,

    /// Log level (trace, debug, info, warn, error)
    #[arg(long, default_value = "info")]
    log_level: String,

    /// Output logs as JSON
    #[arg(long, default_value = "true")]
    json_logs: bool,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let args = Args::parse();

    // Initialize telemetry first
    let telemetry_config = TelemetryConfig {
        service_name: "vajra".into(),
        otlp_endpoint: None,
        log_level: args.log_level.clone(),
        json_logs: args.json_logs,
    };

    init_telemetry(&telemetry_config)?;

    info!(
        version = env!("CARGO_PKG_VERSION"),
        "Starting Vajra distributed vector database"
    );

    // Load configuration
    let config = if let Some(config_path) = &args.config {
        info!(path = %config_path.display(), "Loading configuration from file");
        VajraConfig::from_file(config_path)?
    } else {
        info!("Using default configuration");
        VajraConfig::default()
    };

    // Validate configuration
    config.validate()?;

    info!(
        node_id = %config.node.id,
        dimensions = config.engine.dimensions,
        max_vectors = config.engine.max_vectors,
        "Configuration validated"
    );

    // TODO: Phase 2+ - Implement actual server startup
    // 1. Open/recover WAL
    // 2. Rebuild state machine from WAL
    // 3. Initialize Raft node
    // 4. Start gRPC server
    // 5. Start metrics HTTP server

    info!("Vajra server initialized (Phase 0 - foundation only)");
    info!("Press Ctrl+C to shutdown");

    // Wait for shutdown signal
    tokio::signal::ctrl_c().await?;

    info!("Shutdown signal received");
    shutdown_telemetry();

    info!("Vajra server stopped");
    Ok(())
}
