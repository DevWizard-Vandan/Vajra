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
//! - **Micro-Batching Ready**: WAL supports batch commands for performance
//!
//! ## Vajra Philosophy
//!
//! > "The heartbeat must never wait for a search."
//!
//! The tick handler prioritizes heartbeats above all other operations.

#![warn(missing_docs)]
#![warn(clippy::all)]

pub mod log;
pub mod messages;
pub mod node;
pub mod state;

// Re-export main types
pub use log::RaftLog;
pub use messages::{AppendEntries, AppendEntriesResponse, RaftMessage, RequestVote, RequestVoteResponse};
pub use node::{NodeConfig, RaftNode, Ready};
pub use state::{LeaderState, PersistentState, VolatileState};

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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_role_equality() {
        assert_eq!(Role::Follower, Role::Follower);
        assert_ne!(Role::Follower, Role::Leader);
    }
}
