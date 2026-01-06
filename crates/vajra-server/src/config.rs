//! Server configuration.

use serde::{Deserialize, Serialize};
use std::net::SocketAddr;
use std::path::PathBuf;
use vajra_common::config::{HnswConfig, RaftConfig};
use vajra_common::NodeId;

/// Vajra server configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerConfig {
    /// This node's ID.
    pub node_id: NodeId,
    /// gRPC listen address.
    pub grpc_addr: SocketAddr,
    /// Metrics HTTP listen address.
    pub metrics_addr: SocketAddr,
    /// Peer node addresses.
    pub peers: Vec<PeerConfig>,
    /// Data directory for WAL and snapshots.
    pub data_dir: PathBuf,
    /// HNSW configuration.
    pub hnsw: HnswConfig,
    /// Raft configuration.
    pub raft: RaftConfig,
    /// Vector dimensions.
    pub dimensions: usize,
    /// Maximum number of vectors.
    pub max_vectors: usize,
}

/// Peer node configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PeerConfig {
    /// Peer node ID.
    pub id: NodeId,
    /// Peer gRPC address.
    pub addr: SocketAddr,
}

impl Default for ServerConfig {
    fn default() -> Self {
        Self {
            node_id: NodeId(1),
            grpc_addr: "0.0.0.0:50051".parse().unwrap(),
            metrics_addr: "0.0.0.0:9090".parse().unwrap(),
            peers: Vec::new(),
            data_dir: PathBuf::from("data"),
            hnsw: HnswConfig::default(),
            raft: RaftConfig::default(),
            dimensions: 128,
            max_vectors: 1_000_000,
        }
    }
}

impl ServerConfig {
    /// Load configuration from a TOML file.
    pub fn from_file(path: &std::path::Path) -> Result<Self, anyhow::Error> {
        let content = std::fs::read_to_string(path)?;
        let config: ServerConfig = toml::from_str(&content)?;
        Ok(config)
    }

    /// Get all node IDs (including self).
    pub fn all_node_ids(&self) -> Vec<NodeId> {
        let mut ids: Vec<NodeId> = self.peers.iter().map(|p| p.id).collect();
        ids.push(self.node_id);
        ids.sort_by_key(|id| id.0);
        ids
    }

    /// Check if this is a single-node cluster.
    pub fn is_single_node(&self) -> bool {
        self.peers.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = ServerConfig::default();
        assert_eq!(config.node_id, NodeId(1));
        assert!(config.is_single_node());
    }
}
