//! Raft reactor implementing the event loop.
//!
//! This is the heart of the server - a centralized event loop that
//! serializes all state changes to avoid race conditions.

use crate::config::ServerConfig;
use crate::state_machine::VajraStateMachine;
use crate::transport::TransportManager;
use std::time::Duration;
use tokio::sync::{mpsc, oneshot};
use tracing::{error, info, warn};
use vajra_common::types::SearchResult;
use vajra_common::{NodeId, VajraError};
use vajra_raft::{NodeConfig, RaftMessage, RaftNode, Ready, Role};
use vajra_wal::WalConfig;

/// Client request types sent to the reactor.
#[derive(Debug)]
pub enum ClientRequest {
    /// Propose an insert operation.
    Insert {
        id: String,
        vector: Vec<f32>,
        response: oneshot::Sender<Result<u64, VajraError>>,
    },
    /// Propose a delete operation.
    Delete {
        id: String,
        response: oneshot::Sender<Result<bool, VajraError>>,
    },
    /// Search request (read-only, needs ReadIndex check).
    Search {
        query: Vec<f32>,
        k: usize,
        ef: usize,
        response: oneshot::Sender<Result<Vec<SearchResult>, VajraError>>,
    },
    /// Get current status.
    Status {
        response: oneshot::Sender<NodeStatus>,
    },
}

/// Node status for health checks.
#[derive(Debug, Clone)]
pub struct NodeStatus {
    /// Current role.
    pub role: Role,
    /// Current term.
    pub term: u64,
    /// Current leader.
    pub leader: Option<NodeId>,
    /// Number of vectors.
    pub vector_count: usize,
    /// Last applied index.
    pub last_applied: u64,
}

/// The Vajra node reactor.
pub struct VajraNode {
    /// Node ID.
    id: NodeId,
    /// Raft node.
    raft: RaftNode,
    /// State machine (HNSW index).
    state_machine: VajraStateMachine,
    /// Client request receiver.
    client_rx: mpsc::Receiver<ClientRequest>,
    /// Client request sender (for cloning to gRPC handlers).
    client_tx: mpsc::Sender<ClientRequest>,
    /// Outbound transport manager.
    transport: TransportManager,
    /// Shutdown signal.
    shutdown_rx: oneshot::Receiver<()>,
}

impl VajraNode {
    /// Create a new VajraNode.
    pub fn new(
        config: ServerConfig,
        shutdown_rx: oneshot::Receiver<()>,
    ) -> Result<Self, VajraError> {
        let (client_tx, client_rx) = mpsc::channel(1024);

        // Build peer list for transport
        let peers: Vec<(NodeId, String)> = config
            .peers
            .iter()
            .map(|p| (p.id, p.addr.to_string()))
            .collect();

        // Create transport manager (spawns workers)
        let transport = TransportManager::new(peers);

        // WAL config
        let wal_config = WalConfig {
            dir: config.data_dir.join("wal"),
            segment_size: 64 * 1024 * 1024,
            sync_policy: vajra_common::config::SyncPolicy::Batched {
                entries: 100,
                timeout_ms: 100,
            },
        };

        // Raft node config
        let raft_config = NodeConfig {
            id: config.node_id,
            peers: config.all_node_ids(),
            raft_config: config.raft.clone(),
            wal_config,
            pre_vote_enabled: config.raft.pre_vote_enabled,
        };

        let raft = RaftNode::new(raft_config)?;

        let state_machine = VajraStateMachine::new(
            config.dimensions,
            config.max_vectors,
            config.hnsw.clone(),
        );

        Ok(Self {
            id: config.node_id,
            raft,
            state_machine,
            client_rx,
            client_tx,
            transport,
            shutdown_rx,
        })
    }

    /// Get a client sender for gRPC handlers.
    pub fn client_sender(&self) -> mpsc::Sender<ClientRequest> {
        self.client_tx.clone()
    }

    /// Run the reactor event loop.
    #[tracing::instrument(skip(self))]
    pub async fn run(mut self) -> Result<(), VajraError> {
        let tick_interval = Duration::from_millis(100);
        let mut ticker = tokio::time::interval(tick_interval);

        info!(node = %self.id, "Starting reactor event loop");

        // Bootstrap: if single node, start campaign
        if self.transport.is_empty() {
            info!("Single-node cluster, starting leader campaign");
            // The tick will handle election
        }

        loop {
            tokio::select! {
                biased;  // Priority order!

                // 0. SHUTDOWN - Check first for graceful shutdown
                _ = &mut self.shutdown_rx => {
                    info!("Received shutdown signal");
                    break;
                }

                // 1. TICK - Heartbeats and election timeouts (PRIORITY)
                _ = ticker.tick() => {
                    let ready = self.raft.tick();
                    self.handle_ready(ready).await;
                }

                // 2. CLIENT - Proposals and reads
                Some(req) = self.client_rx.recv() => {
                    self.handle_client_request(req).await;
                }
            }
        }

        info!("Reactor loop ended");
        Ok(())
    }

    /// Handle a client request.
    async fn handle_client_request(&mut self, req: ClientRequest) {
        match req {
            ClientRequest::Insert { id, vector, response } => {
                let result = self.handle_insert(id, vector).await;
                let _ = response.send(result);
            }
            ClientRequest::Delete { id, response } => {
                let result = self.handle_delete(id).await;
                let _ = response.send(result);
            }
            ClientRequest::Search { query, k, ef, response } => {
                let result = self.handle_search(query, k, ef);
                let _ = response.send(result);
            }
            ClientRequest::Status { response } => {
                let status = NodeStatus {
                    role: self.raft.role(),
                    term: self.raft.term(),
                    leader: self.raft.leader(),
                    vector_count: self.state_machine.index().len(),
                    last_applied: self.state_machine.last_applied(),
                };
                let _ = response.send(status);
            }
        }
    }

    /// Handle insert request.
    async fn handle_insert(&mut self, id: String, vector: Vec<f32>) -> Result<u64, VajraError> {
        // Check if we're the leader
        if !self.raft.is_leader() {
            return Err(VajraError::NotLeader {
                leader: self.raft.leader(),
            });
        }

        // Convert string ID to internal ID using SipHash
        let internal_id = vajra_transport::id_mapper::to_vector_id(&id);

        // Propose to Raft
        let command = vajra_wal::Command::Insert {
            id: internal_id.0,
            vector,
            metadata: None,
        };

        self.raft.propose(command)
    }

    /// Handle delete request.
    async fn handle_delete(&mut self, id: String) -> Result<bool, VajraError> {
        if !self.raft.is_leader() {
            return Err(VajraError::NotLeader {
                leader: self.raft.leader(),
            });
        }

        let internal_id = vajra_transport::id_mapper::to_vector_id(&id);

        let command = vajra_wal::Command::Delete {
            id: internal_id.0,
        };

        self.raft.propose(command)?;
        Ok(true)
    }

    /// Handle search request with ReadIndex check.
    fn handle_search(
        &self,
        query: Vec<f32>,
        k: usize,
        ef: usize,
    ) -> Result<Vec<SearchResult>, VajraError> {
        // ReadIndex check: ensure we're leader and fully applied
        // (Simplified version - full implementation would check with Raft)
        if !self.raft.is_leader() {
            warn!("Search request on non-leader, may be stale");
            // For now, we allow reads on followers with a warning
            // A strict implementation would return Err(NotLeader)
        }

        self.state_machine.index().search(&query, k, ef)
    }

    /// Handle Raft ready state.
    async fn handle_ready(&mut self, ready: Ready) {
        // A. APPLY COMMITTED ENTRIES (to state machine)
        for entry in ready.committed_entries {
            if let Err(e) = self.state_machine.apply(&entry) {
                error!(error = %e, index = entry.index, "Failed to apply entry");
            }
        }

        // B. SEND MESSAGES (to other nodes)
        for (to, msg) in ready.messages {
            if let Err(e) = self.send_raft_message(to, msg).await {
                warn!(to = %to, error = %e, "Failed to send Raft message");
            }
        }

        // Note: Persistence is handled by the WAL inside RaftNode
    }

    /// Send a Raft message to another node via TransportManager.
    async fn send_raft_message(&self, to: NodeId, msg: RaftMessage) -> Result<(), VajraError> {
        self.transport
            .send(to, msg)
            .await
            .map_err(|e| VajraError::NodeUnreachable {
                node_id: to,
                attempts: 1,
            })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    fn test_config(node_id: u64, data_dir: &std::path::Path) -> ServerConfig {
        ServerConfig {
            node_id: NodeId(node_id),
            grpc_addr: "127.0.0.1:50051".parse().unwrap(),
            metrics_addr: "127.0.0.1:9090".parse().unwrap(),
            peers: vec![],
            data_dir: data_dir.to_path_buf(),
            hnsw: vajra_common::config::HnswConfig::default(),
            raft: vajra_common::config::RaftConfig::default(),
            dimensions: 4,
            max_vectors: 1000,
        }
    }

    #[tokio::test]
    async fn test_reactor_creation() {
        let dir = tempdir().unwrap();
        let (_shutdown_tx, shutdown_rx) = oneshot::channel();

        let node = VajraNode::new(test_config(1, dir.path()), shutdown_rx);
        assert!(node.is_ok());
    }

    #[tokio::test]
    async fn test_status_request() {
        let dir = tempdir().unwrap();
        let (_shutdown_tx, shutdown_rx) = oneshot::channel();

        let node = VajraNode::new(test_config(1, dir.path()), shutdown_rx).unwrap();
        let tx = node.client_sender();

        let (resp_tx, resp_rx) = oneshot::channel();
        tx.send(ClientRequest::Status { response: resp_tx }).await.unwrap();

        // Note: We can't actually run the reactor in this test,
        // but we verified the channel setup works
    }
}
