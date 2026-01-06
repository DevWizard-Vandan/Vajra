//! Raft persistent and volatile state.
//!
//! This module defines the state that a Raft node must maintain.

use serde::{Deserialize, Serialize};
use vajra_common::NodeId;

/// Persistent state on all servers.
/// Updated on stable storage before responding to RPCs.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PersistentState {
    /// Latest term server has seen (initialized to 0, increases monotonically).
    pub current_term: u64,
    /// CandidateId that received vote in current term (or None if none).
    pub voted_for: Option<NodeId>,
}

impl Default for PersistentState {
    fn default() -> Self {
        Self {
            current_term: 0,
            voted_for: None,
        }
    }
}

impl PersistentState {
    /// Create new persistent state.
    pub fn new() -> Self {
        Self::default()
    }

    /// Update term and clear vote if term increased.
    pub fn update_term(&mut self, new_term: u64) {
        if new_term > self.current_term {
            self.current_term = new_term;
            self.voted_for = None;
        }
    }

    /// Set voted_for for current term.
    pub fn set_voted_for(&mut self, candidate: NodeId) {
        self.voted_for = Some(candidate);
    }
}

/// Volatile state on all servers.
/// Reinitialized after crash.
#[derive(Debug, Clone)]
pub struct VolatileState {
    /// Index of highest log entry known to be committed.
    /// Initialized to 0, increases monotonically.
    pub commit_index: u64,
    /// Index of highest log entry applied to state machine.
    /// Initialized to 0, increases monotonically.
    pub last_applied: u64,
}

impl Default for VolatileState {
    fn default() -> Self {
        Self {
            commit_index: 0,
            last_applied: 0,
        }
    }
}

impl VolatileState {
    /// Create new volatile state.
    pub fn new() -> Self {
        Self::default()
    }

    /// Update commit index if new value is greater.
    pub fn update_commit(&mut self, new_commit: u64) {
        if new_commit > self.commit_index {
            self.commit_index = new_commit;
        }
    }

    /// Advance last_applied to commit_index, returning entries to apply.
    pub fn advance_applied(&mut self) -> Option<std::ops::Range<u64>> {
        if self.last_applied < self.commit_index {
            let range = self.last_applied + 1..self.commit_index + 1;
            self.last_applied = self.commit_index;
            Some(range)
        } else {
            None
        }
    }
}

/// Volatile state on leaders only.
/// Reinitialized after election.
#[derive(Debug, Clone)]
pub struct LeaderState {
    /// For each server, index of the next log entry to send.
    /// Initialized to leader's last log index + 1.
    pub next_index: Vec<u64>,
    /// For each server, index of highest log entry known to be replicated.
    /// Initialized to 0, increases monotonically.
    pub match_index: Vec<u64>,
}

impl LeaderState {
    /// Create new leader state for a cluster of given size.
    pub fn new(cluster_size: usize, last_log_index: u64) -> Self {
        Self {
            next_index: vec![last_log_index + 1; cluster_size],
            match_index: vec![0; cluster_size],
        }
    }

    /// Update match_index for a follower after successful replication.
    pub fn update_match(&mut self, follower_idx: usize, match_idx: u64) {
        if follower_idx < self.match_index.len() {
            self.match_index[follower_idx] = match_idx;
        }
    }

    /// Decrement next_index for a follower after failed replication.
    pub fn decrement_next(&mut self, follower_idx: usize) {
        if follower_idx < self.next_index.len() && self.next_index[follower_idx] > 1 {
            self.next_index[follower_idx] -= 1;
        }
    }

    /// Calculate the commit index based on majority replication.
    pub fn compute_commit_index(&self, current_commit: u64) -> u64 {
        let mut sorted: Vec<u64> = self.match_index.clone();
        sorted.sort_unstable();
        // Majority = N/2 + 1, so the median is the commit index
        let majority_idx = sorted.len() / 2;
        sorted.get(majority_idx).copied().unwrap_or(current_commit).max(current_commit)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_persistent_state_update_term() {
        let mut state = PersistentState::new();
        state.set_voted_for(NodeId(1));
        assert_eq!(state.voted_for, Some(NodeId(1)));

        state.update_term(2);
        assert_eq!(state.current_term, 2);
        assert_eq!(state.voted_for, None); // Vote cleared on term change
    }

    #[test]
    fn test_volatile_state_advance_applied() {
        let mut state = VolatileState::new();
        state.commit_index = 5;

        let range = state.advance_applied();
        assert_eq!(range, Some(1..6));
        assert_eq!(state.last_applied, 5);

        // Second call should return None
        let range = state.advance_applied();
        assert_eq!(range, None);
    }

    #[test]
    fn test_leader_state_compute_commit() {
        let mut state = LeaderState::new(5, 10);
        // Simulate replication: indices [8, 10, 10, 5, 7]
        state.match_index = vec![8, 10, 10, 5, 7];

        // Sorted: [5, 7, 8, 10, 10], median (index 2) = 8
        let commit = state.compute_commit_index(0);
        assert_eq!(commit, 8);
    }
}
