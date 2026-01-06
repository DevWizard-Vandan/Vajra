//! Outbound transport for Raft message delivery.
//!
//! This module provides the bridge between the Reactor and peer nodes.
//! Each peer has a dedicated worker that maintains a gRPC connection.

use std::collections::HashMap;
use std::time::Duration;
use tokio::sync::mpsc;
use tracing::{error, info, warn};
use vajra_common::NodeId;
use vajra_raft::RaftMessage;

/// Channel capacity for outbound messages per peer.
const PEER_CHANNEL_CAPACITY: usize = 256;

/// Reconnect backoff configuration.
const INITIAL_BACKOFF_MS: u64 = 100;
const MAX_BACKOFF_MS: u64 = 5000;

/// Transport manager that holds channels to all peer workers.
pub struct TransportManager {
    /// Sender channels to each peer worker.
    peer_senders: HashMap<NodeId, mpsc::Sender<RaftMessage>>,
}

impl TransportManager {
    /// Create a new TransportManager and spawn workers for each peer.
    pub fn new(peers: Vec<(NodeId, String)>) -> Self {
        let mut peer_senders = HashMap::new();

        for (peer_id, endpoint) in peers {
            let (tx, rx) = mpsc::channel(PEER_CHANNEL_CAPACITY);
            peer_senders.insert(peer_id, tx);

            // Spawn the worker
            let worker = PeerWorker {
                peer_id,
                endpoint,
                rx,
            };
            tokio::spawn(worker.run());

            info!(peer = %peer_id, "Spawned peer worker");
        }

        Self { peer_senders }
    }

    /// Send a message to a specific peer.
    pub async fn send(&self, to: NodeId, msg: RaftMessage) -> Result<(), TransportError> {
        let sender = self
            .peer_senders
            .get(&to)
            .ok_or(TransportError::UnknownPeer(to))?;

        sender
            .send(msg)
            .await
            .map_err(|_| TransportError::ChannelClosed(to))
    }

    /// Send a message without waiting (fire and forget).
    pub fn try_send(&self, to: NodeId, msg: RaftMessage) -> Result<(), TransportError> {
        let sender = self
            .peer_senders
            .get(&to)
            .ok_or(TransportError::UnknownPeer(to))?;

        sender
            .try_send(msg)
            .map_err(|_| TransportError::ChannelFull(to))
    }

    /// Check if there are no peers configured (single-node cluster).
    pub fn is_empty(&self) -> bool {
        self.peer_senders.is_empty()
    }
}

/// Transport errors.
#[derive(Debug)]
pub enum TransportError {
    /// Peer not found in configuration.
    UnknownPeer(NodeId),
    /// Channel to worker is closed.
    ChannelClosed(NodeId),
    /// Channel is full (backpressure).
    ChannelFull(NodeId),
    /// Connection failed.
    ConnectionFailed(NodeId, String),
}

impl std::fmt::Display for TransportError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::UnknownPeer(id) => write!(f, "Unknown peer: {}", id),
            Self::ChannelClosed(id) => write!(f, "Channel closed for peer: {}", id),
            Self::ChannelFull(id) => write!(f, "Channel full for peer: {}", id),
            Self::ConnectionFailed(id, reason) => {
                write!(f, "Connection failed to {}: {}", id, reason)
            }
        }
    }
}

impl std::error::Error for TransportError {}

/// Per-peer worker that maintains gRPC connection.
struct PeerWorker {
    peer_id: NodeId,
    endpoint: String,
    rx: mpsc::Receiver<RaftMessage>,
}

impl PeerWorker {
    /// Run the worker loop.
    async fn run(mut self) {
        info!(peer = %self.peer_id, endpoint = %self.endpoint, "Starting peer worker");

        let mut backoff_ms = INITIAL_BACKOFF_MS;
        let mut client: Option<RaftClient> = None;

        loop {
            // Ensure we have a connection
            if client.is_none() {
                match self.connect_with_backoff(&mut backoff_ms).await {
                    Ok(c) => {
                        client = Some(c);
                        backoff_ms = INITIAL_BACKOFF_MS; // Reset backoff
                        info!(peer = %self.peer_id, "Connected to peer");
                    }
                    Err(e) => {
                        error!(peer = %self.peer_id, error = %e, "Failed to connect");
                        continue;
                    }
                }
            }

            // Wait for message
            let msg = match self.rx.recv().await {
                Some(m) => m,
                None => {
                    info!(peer = %self.peer_id, "Channel closed, shutting down");
                    break;
                }
            };

            // Send the message
            if let Some(ref mut c) = client {
                if let Err(e) = c.send(&msg).await {
                    warn!(peer = %self.peer_id, error = %e, "Send failed, reconnecting");
                    client = None;
                }
            }
        }
    }

    /// Connect with exponential backoff.
    async fn connect_with_backoff(&self, backoff_ms: &mut u64) -> Result<RaftClient, String> {
        loop {
            match RaftClient::connect(&self.endpoint).await {
                Ok(client) => return Ok(client),
                Err(e) => {
                    warn!(
                        peer = %self.peer_id,
                        backoff_ms = *backoff_ms,
                        error = %e,
                        "Connection failed, retrying"
                    );
                    tokio::time::sleep(Duration::from_millis(*backoff_ms)).await;
                    *backoff_ms = (*backoff_ms * 2).min(MAX_BACKOFF_MS);
                }
            }
        }
    }
}

/// Simple gRPC client wrapper for Raft messages.
/// TODO: Wire this to actual tonic client from vajra-transport.
struct RaftClient {
    _endpoint: String,
}

impl RaftClient {
    /// Connect to the peer endpoint.
    async fn connect(endpoint: &str) -> Result<Self, String> {
        // TODO: Replace with actual tonic client
        // For now, this is a placeholder that simulates connection
        info!(endpoint = endpoint, "Connecting to peer (placeholder)");
        Ok(Self {
            _endpoint: endpoint.to_string(),
        })
    }

    /// Send a Raft message.
    async fn send(&mut self, msg: &RaftMessage) -> Result<(), String> {
        // TODO: Convert RaftMessage to gRPC request and send
        // For now, log the message type
        match msg {
            RaftMessage::RequestVote(_) => {
                info!("Sending RequestVote (placeholder)");
            }
            RaftMessage::RequestVoteResponse(_) => {
                info!("Sending RequestVoteResponse (placeholder)");
            }
            RaftMessage::AppendEntries(_) => {
                info!("Sending AppendEntries (placeholder)");
            }
            RaftMessage::AppendEntriesResponse(_) => {
                info!("Sending AppendEntriesResponse (placeholder)");
            }
            RaftMessage::PreVote(_) => {
                info!("Sending PreVote (placeholder)");
            }
            RaftMessage::PreVoteResponse(_) => {
                info!("Sending PreVoteResponse (placeholder)");
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_transport_manager_creation() {
        let peers = vec![
            (NodeId(2), "127.0.0.1:50052".to_string()),
            (NodeId(3), "127.0.0.1:50053".to_string()),
        ];

        let manager = TransportManager::new(peers);
        assert!(manager.peer_senders.contains_key(&NodeId(2)));
        assert!(manager.peer_senders.contains_key(&NodeId(3)));
    }

    #[test]
    fn test_transport_error_display() {
        let e = TransportError::UnknownPeer(NodeId(5));
        assert!(e.to_string().contains("Unknown peer"));
    }
}
