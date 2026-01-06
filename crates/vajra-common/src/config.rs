//! Configuration management for Vajra.
//!
//! This module defines all configuration structures used throughout the system:
//!
//! - [`VajraConfig`] - Top-level configuration aggregating all components
//! - [`NodeConfig`] - Node identification and cluster membership
//! - [`EngineConfig`] - HNSW index parameters
//! - [`RaftConfig`] - Consensus protocol timing
//! - [`StorageConfig`] - WAL and snapshot settings
//! - [`NetworkConfig`] - gRPC and transport settings

use crate::types::{DistanceMetric, NodeId};
use serde::{Deserialize, Serialize};
use std::net::SocketAddr;
use std::path::PathBuf;
use std::time::Duration;

/// Top-level configuration for a Vajra node.
///
/// This struct aggregates all component configurations and can be
/// loaded from a TOML or JSON file.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VajraConfig {
    /// Node identification and cluster membership
    pub node: NodeConfig,
    /// Vector index configuration
    pub engine: EngineConfig,
    /// Raft consensus configuration
    pub raft: RaftConfig,
    /// Storage and persistence configuration
    pub storage: StorageConfig,
    /// Network and transport configuration
    pub network: NetworkConfig,
}

impl Default for VajraConfig {
    fn default() -> Self {
        Self {
            node: NodeConfig::default(),
            engine: EngineConfig::default(),
            raft: RaftConfig::default(),
            storage: StorageConfig::default(),
            network: NetworkConfig::default(),
        }
    }
}

impl VajraConfig {
    /// Load configuration from a TOML file.
    ///
    /// # Errors
    /// Returns an error if the file cannot be read or parsed.
    pub fn from_file(path: &std::path::Path) -> crate::Result<Self> {
        let content = std::fs::read_to_string(path)
            .map_err(|e| crate::VajraError::io("reading config file", e))?;
        toml::from_str(&content).map_err(|e| crate::VajraError::Configuration {
            message: format!("failed to parse config: {e}"),
        })
    }

    /// Validate the configuration for consistency.
    ///
    /// # Errors
    /// Returns an error if the configuration is invalid.
    pub fn validate(&self) -> crate::Result<()> {
        // Validate engine config
        if self.engine.dimensions == 0 {
            return Err(crate::VajraError::Configuration {
                message: "dimensions must be greater than 0".into(),
            });
        }

        if self.engine.max_vectors == 0 {
            return Err(crate::VajraError::Configuration {
                message: "max_vectors must be greater than 0".into(),
            });
        }

        // Validate HNSW config
        if self.engine.hnsw.m == 0 {
            return Err(crate::VajraError::Configuration {
                message: "HNSW M must be greater than 0".into(),
            });
        }

        // Validate Raft config
        if self.raft.election_timeout_min_ms >= self.raft.election_timeout_max_ms {
            return Err(crate::VajraError::Configuration {
                message: "election_timeout_min_ms must be less than election_timeout_max_ms".into(),
            });
        }

        if self.raft.heartbeat_interval_ms >= self.raft.election_timeout_min_ms {
            return Err(crate::VajraError::Configuration {
                message: "heartbeat_interval_ms must be less than election_timeout_min_ms".into(),
            });
        }

        Ok(())
    }
}

/// Node identification and cluster membership configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NodeConfig {
    /// Unique identifier for this node
    pub id: NodeId,
    /// Human-readable name for this node
    pub name: String,
    /// Addresses of all nodes in the cluster (including self)
    pub cluster_nodes: Vec<ClusterNode>,
}

impl Default for NodeConfig {
    fn default() -> Self {
        Self {
            id: NodeId(1),
            name: "vajra-node-1".into(),
            cluster_nodes: vec![ClusterNode {
                id: NodeId(1),
                address: "127.0.0.1:50051".parse().unwrap(),
            }],
        }
    }
}

/// A node in the cluster.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClusterNode {
    /// Node identifier
    pub id: NodeId,
    /// Node address for Raft communication
    pub address: SocketAddr,
}

/// Vector index configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EngineConfig {
    /// Dimension of vectors in the index
    pub dimensions: usize,
    /// Maximum number of vectors the index can hold
    pub max_vectors: usize,
    /// HNSW algorithm parameters
    pub hnsw: HnswConfig,
    /// Distance metric for similarity calculations
    pub distance_metric: DistanceMetric,
}

impl Default for EngineConfig {
    fn default() -> Self {
        Self {
            dimensions: 128,
            max_vectors: 1_000_000,
            hnsw: HnswConfig::default(),
            distance_metric: DistanceMetric::Euclidean,
        }
    }
}

/// HNSW (Hierarchical Navigable Small World) algorithm parameters.
///
/// These parameters control the trade-off between index build time,
/// search speed, and recall quality.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HnswConfig {
    /// Maximum number of connections per node (except layer 0).
    /// Higher values increase recall but slow down search and increase memory.
    /// Typical range: 12-48, default: 16
    pub m: usize,

    /// Maximum number of connections per node at layer 0.
    /// Usually set to 2*M for better connectivity at the base layer.
    /// Default: 2 * M
    pub m_max0: usize,

    /// Number of candidates to consider during index construction.
    /// Higher values improve recall but increase build time.
    /// Typical range: 100-500, default: 200
    pub ef_construction: usize,

    /// Default number of candidates to consider during search.
    /// Higher values improve recall but slow down search.
    /// Can be overridden per-query. Typical range: 10-200, default: 50
    pub ef_search: usize,

    /// Level generation multiplier.
    /// Controls the probability of a node being promoted to higher layers.
    /// Default: 1.0 / ln(M)
    pub ml: f64,
}

impl Default for HnswConfig {
    fn default() -> Self {
        let m = 16;
        Self {
            m,
            m_max0: m * 2,
            ef_construction: 200,
            ef_search: 50,
            ml: 1.0 / (m as f64).ln(),
        }
    }
}

impl HnswConfig {
    /// Create a configuration optimized for high recall.
    #[must_use]
    pub fn high_recall() -> Self {
        let m = 32;
        Self {
            m,
            m_max0: m * 2,
            ef_construction: 400,
            ef_search: 100,
            ml: 1.0 / (m as f64).ln(),
        }
    }

    /// Create a configuration optimized for fast search.
    #[must_use]
    pub fn fast_search() -> Self {
        let m = 12;
        Self {
            m,
            m_max0: m * 2,
            ef_construction: 100,
            ef_search: 20,
            ml: 1.0 / (m as f64).ln(),
        }
    }
}

/// Raft consensus protocol configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RaftConfig {
    /// Minimum election timeout in milliseconds.
    /// After this time without hearing from a leader, a follower starts an election.
    /// Default: 150ms
    pub election_timeout_min_ms: u64,

    /// Maximum election timeout in milliseconds.
    /// Actual timeout is randomized between min and max to prevent split votes.
    /// Default: 300ms
    pub election_timeout_max_ms: u64,

    /// Heartbeat interval in milliseconds.
    /// Leaders send heartbeats at this interval to maintain authority.
    /// Must be significantly less than election timeout.
    /// Default: 50ms
    pub heartbeat_interval_ms: u64,

    /// Number of log entries before triggering a snapshot.
    /// Snapshots compact the log to reduce disk usage and speed up recovery.
    /// Default: 10000
    pub snapshot_threshold: u64,

    /// Maximum number of entries to send in a single AppendEntries RPC.
    /// Limits memory usage during replication.
    /// Default: 100
    pub max_entries_per_append: usize,

    /// Enable Pre-Vote extension to prevent term inflation during partitions.
    /// Default: true
    pub pre_vote_enabled: bool,
}

impl Default for RaftConfig {
    fn default() -> Self {
        Self {
            election_timeout_min_ms: 150,
            election_timeout_max_ms: 300,
            heartbeat_interval_ms: 50,
            snapshot_threshold: 10_000,
            max_entries_per_append: 100,
            pre_vote_enabled: true,
        }
    }
}

impl RaftConfig {
    /// Get the heartbeat interval as a Duration.
    #[must_use]
    pub const fn heartbeat_interval(&self) -> Duration {
        Duration::from_millis(self.heartbeat_interval_ms)
    }

    /// Get the minimum election timeout as a Duration.
    #[must_use]
    pub const fn election_timeout_min(&self) -> Duration {
        Duration::from_millis(self.election_timeout_min_ms)
    }

    /// Get the maximum election timeout as a Duration.
    #[must_use]
    pub const fn election_timeout_max(&self) -> Duration {
        Duration::from_millis(self.election_timeout_max_ms)
    }
}

/// Storage and persistence configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StorageConfig {
    /// Directory for WAL segments
    pub wal_dir: PathBuf,

    /// Directory for snapshots
    pub snapshot_dir: PathBuf,

    /// Maximum WAL segment size in bytes before rotation.
    /// Default: 64MB
    pub segment_size_bytes: u64,

    /// fsync policy for WAL writes
    pub sync_policy: SyncPolicy,

    /// Whether to use memory-mapped I/O for WAL
    pub use_mmap: bool,
}

impl Default for StorageConfig {
    fn default() -> Self {
        Self {
            wal_dir: PathBuf::from("./data/wal"),
            snapshot_dir: PathBuf::from("./data/snapshots"),
            segment_size_bytes: 64 * 1024 * 1024, // 64MB
            sync_policy: SyncPolicy::Batched {
                entries: 100,
                timeout_ms: 10,
            },
            use_mmap: true,
        }
    }
}

/// Policy for when to fsync WAL writes to disk.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum SyncPolicy {
    /// fsync after every entry (safest, slowest)
    EveryEntry,

    /// fsync after N entries or M milliseconds, whichever comes first
    Batched {
        /// Number of entries before sync
        entries: usize,
        /// Timeout in milliseconds before sync
        timeout_ms: u64,
    },

    /// Let the OS decide when to sync (fastest, risk of data loss)
    OsDefault,
}

/// Network and transport configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NetworkConfig {
    /// Address to bind the gRPC server to
    pub listen_address: SocketAddr,

    /// Address to bind the metrics HTTP server to
    pub metrics_address: SocketAddr,

    /// Connection timeout for outgoing RPCs
    pub connect_timeout_ms: u64,

    /// Request timeout for RPCs
    pub request_timeout_ms: u64,

    /// Keep-alive interval for connections
    pub keepalive_interval_ms: u64,

    /// Maximum message size in bytes
    pub max_message_size_bytes: usize,
}

impl Default for NetworkConfig {
    fn default() -> Self {
        Self {
            listen_address: "0.0.0.0:50051".parse().unwrap(),
            metrics_address: "0.0.0.0:9090".parse().unwrap(),
            connect_timeout_ms: 5000,
            request_timeout_ms: 10_000,
            keepalive_interval_ms: 10_000,
            max_message_size_bytes: 16 * 1024 * 1024, // 16MB
        }
    }
}

impl NetworkConfig {
    /// Get the connection timeout as a Duration.
    #[must_use]
    pub const fn connect_timeout(&self) -> Duration {
        Duration::from_millis(self.connect_timeout_ms)
    }

    /// Get the request timeout as a Duration.
    #[must_use]
    pub const fn request_timeout(&self) -> Duration {
        Duration::from_millis(self.request_timeout_ms)
    }
}

/// Telemetry configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TelemetryConfig {
    /// Service name for tracing
    pub service_name: String,

    /// OpenTelemetry collector endpoint (optional)
    pub otlp_endpoint: Option<String>,

    /// Log level filter
    pub log_level: String,

    /// Whether to output logs as JSON
    pub json_logs: bool,
}

impl Default for TelemetryConfig {
    fn default() -> Self {
        Self {
            service_name: "vajra".into(),
            otlp_endpoint: None,
            log_level: "info".into(),
            json_logs: true,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = VajraConfig::default();
        assert!(config.validate().is_ok());
    }

    #[test]
    fn test_hnsw_default_ml() {
        let config = HnswConfig::default();
        let expected_ml = 1.0 / (16.0_f64).ln();
        assert!((config.ml - expected_ml).abs() < 0.001);
    }

    #[test]
    fn test_invalid_dimensions() {
        let mut config = VajraConfig::default();
        config.engine.dimensions = 0;
        assert!(config.validate().is_err());
    }

    #[test]
    fn test_invalid_election_timeout() {
        let mut config = VajraConfig::default();
        config.raft.election_timeout_min_ms = 300;
        config.raft.election_timeout_max_ms = 150;
        assert!(config.validate().is_err());
    }

    #[test]
    fn test_invalid_heartbeat() {
        let mut config = VajraConfig::default();
        config.raft.heartbeat_interval_ms = 200; // Greater than min election timeout
        assert!(config.validate().is_err());
    }

    #[test]
    fn test_high_recall_config() {
        let config = HnswConfig::high_recall();
        assert_eq!(config.m, 32);
        assert_eq!(config.ef_construction, 400);
    }

    #[test]
    fn test_fast_search_config() {
        let config = HnswConfig::fast_search();
        assert_eq!(config.m, 12);
        assert_eq!(config.ef_search, 20);
    }
}
