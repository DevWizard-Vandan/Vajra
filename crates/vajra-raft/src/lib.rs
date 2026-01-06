//! # Vajra Raft
//!
//! Raft consensus protocol implementation for distributed coordination.
//!
//! This crate implements the Raft consensus algorithm with the Pre-Vote
//! extension for improved partition tolerance.
//!
//! ## Features
//!
//! - **Leader Election**: Randomized timeouts to prevent split-brain
//! - **Log Replication**: Reliable replication to followers
//! - **Pre-Vote Extension**: Prevents term inflation during partitions
//! - **Read Index**: Linearizable reads without log replication
//! - **Snapshots**: Log compaction for efficient recovery
//!
//! ## Roles
//!
//! - **Leader**: Handles all client requests, replicates to followers
//! - **Follower**: Passive, responds to leader RPCs
//! - **Candidate**: Seeking election to become leader

#![warn(missing_docs)]
#![warn(clippy::all)]
#![warn(clippy::pedantic)]

// Modules will be implemented in Phase 4
// pub mod node;
// pub mod state;
// pub mod log;
// pub mod rpc;
// pub mod snapshot;

/// Raft role enumeration.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Role {
    /// Passive node that responds to RPCs from leaders and candidates
    Follower,
    /// Node seeking election to become leader
    Candidate,
    /// Active node that handles all client requests
    Leader,
}

/// Placeholder for the Raft node (to be implemented in Phase 4)
pub struct RaftNode {
    _private: (),
}

impl RaftNode {
    /// Create a new Raft node (placeholder)
    #[must_use]
    pub fn new() -> Self {
        Self { _private: () }
    }
}

impl Default for RaftNode {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_placeholder() {
        let _node = RaftNode::new();
    }

    #[test]
    fn test_role_equality() {
        assert_eq!(Role::Follower, Role::Follower);
        assert_ne!(Role::Follower, Role::Leader);
    }
}
