//! # Vajra Server
//!
//! The main entry point for the Vajra distributed vector database.
//!
//! Implements the Raft Reactor Pattern:
//! - gRPC handlers send requests to channels
//! - Reactor loop processes events (tick → network → client)
//! - State machine applies committed entries
//!
//! ## Usage
//!
//! ```bash
//! vajra --config /path/to/vajra.toml
//! vajra --node-id 1 --listen 0.0.0.0:50051
//! ```

mod config;
mod http;
mod reactor;
mod state_machine;
mod transport;

use clap::Parser;
use std::path::PathBuf;
use tokio::sync::oneshot;
use tracing::info;
use vajra_common::config::TelemetryConfig;
use vajra_common::telemetry::{init_telemetry, shutdown_telemetry};
use vajra_common::NodeId;

use crate::config::ServerConfig;
use crate::reactor::VajraNode;

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

    /// gRPC listen address (overrides config file)
    #[arg(long)]
    listen: Option<String>,

    /// HTTP REST listen address (overrides config file)
    #[arg(long)]
    http_listen: Option<String>,

    /// Data directory
    #[arg(long)]
    data_dir: Option<PathBuf>,

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

    // Initialize telemetry
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

    // Build server configuration
    let mut config = if let Some(config_path) = &args.config {
        info!(path = %config_path.display(), "Loading configuration from file");
        ServerConfig::from_file(config_path)?
    } else {
        info!("Using default configuration");
        ServerConfig::default()
    };

    // Apply CLI overrides
    if let Some(node_id) = args.node_id {
        config.node_id = NodeId(node_id);
    }
    if let Some(listen) = args.listen {
        config.grpc_addr = listen.parse()?;
    }
    if let Some(http_listen) = args.http_listen {
        config.http_addr = http_listen.parse()?;
    }
    if let Some(data_dir) = args.data_dir {
        config.data_dir = data_dir;
    }

    info!(
        node_id = %config.node_id,
        grpc_addr = %config.grpc_addr,
        http_addr = %config.http_addr,
        data_dir = %config.data_dir.display(),
        dimensions = config.dimensions,
        "Configuration loaded"
    );

    // Create shutdown channel
    let (shutdown_tx, shutdown_rx) = oneshot::channel();

    // Create reactor node
    let node = VajraNode::new(config.clone(), shutdown_rx)?;
    let client_tx = node.client_sender();

    // Spawn HTTP REST API Server
    let http_addr = config.http_addr;
    let http_client_tx = client_tx.clone();
    tokio::spawn(async move {
        crate::http::start_http_server(http_addr, http_client_tx).await;
    });

    info!("Reactor initialized, starting event loop");

    // Spawn shutdown signal handler
    let shutdown_handle = tokio::spawn(async move {
        tokio::signal::ctrl_c().await.ok();
        info!("Shutdown signal received");
        let _ = shutdown_tx.send(());
    });

    // TODO: Start gRPC server in separate task
    // let grpc_handle = tokio::spawn(async move {
    //     start_grpc_server(config.grpc_addr, client_tx).await
    // });

    // Run the reactor (this is the main event loop)
    if let Err(e) = node.run().await {
        tracing::error!(error = %e, "Reactor error");
    }

    // Wait for shutdown handler to complete
    shutdown_handle.await.ok();

    shutdown_telemetry();
    info!("Vajra server stopped");

    Ok(())
}
