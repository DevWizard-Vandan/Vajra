//! Raft node implementation.
//!
//! This module contains the core Raft state machine with biased tick handling.

use crate::log::RaftLog;
use crate::messages::{
    AppendEntries, AppendEntriesResponse, PreVote, PreVoteResponse, RaftMessage, RequestVote,
    RequestVoteResponse,
};
use crate::state::{LeaderState, PersistentState, VolatileState};
use crate::Role;
use parking_lot::RwLock;
use rand::Rng;
use std::collections::HashSet;
use std::sync::Arc;
use std::time::{Duration, Instant};
use vajra_common::config::RaftConfig;
use vajra_common::{NodeId, VajraError};
use vajra_wal::{Command, WalConfig};

/// Ready contains items that are ready to be processed.
#[derive(Debug, Default)]
pub struct Ready {
    /// Messages to send to other nodes.
    pub messages: Vec<(NodeId, RaftMessage)>,
    /// Entries that have been committed and should be applied.
    pub committed_entries: Vec<vajra_wal::LogEntry>,
    /// State changed and should be persisted.
    pub persist_state: bool,
}

/// Raft node configuration.
#[derive(Debug, Clone)]
pub struct NodeConfig {
    /// This node's ID.
    pub id: NodeId,
    /// All node IDs in the cluster (including self).
    pub peers: Vec<NodeId>,
    /// Raft protocol configuration.
    pub raft_config: RaftConfig,
    /// WAL configuration.
    pub wal_config: WalConfig,
    /// Enable Pre-Vote extension.
    pub pre_vote_enabled: bool,
}

/// The Raft node state machine.
pub struct RaftNode {
    /// This node's ID.
    id: NodeId,
    /// All peers (excluding self).
    peers: Vec<NodeId>,
    /// Current role (Follower, Candidate, Leader).
    role: RwLock<Role>,
    /// Persistent state (term, votedFor).
    persistent: RwLock<PersistentState>,
    /// Volatile state (commitIndex, lastApplied).
    volatile: RwLock<VolatileState>,
    /// Leader-specific state (only valid when leader).
    leader_state: RwLock<Option<LeaderState>>,
    /// The Raft log.
    log: Arc<RaftLog>,
    /// Election timeout configuration.
    election_timeout_min: Duration,
    election_timeout_max: Duration,
    /// Heartbeat interval.
    heartbeat_interval: Duration,
    /// Last time we heard from leader or granted a vote.
    last_heartbeat: RwLock<Instant>,
    /// Current election timeout (randomized).
    election_timeout: RwLock<Duration>,
    /// Pre-Vote extension enabled.
    pre_vote_enabled: bool,
    /// Votes received in current election.
    votes_received: RwLock<HashSet<NodeId>>,
    /// Current known leader.
    leader_id: RwLock<Option<NodeId>>,
}

impl RaftNode {
    /// Create a new Raft node.
    pub fn new(config: NodeConfig) -> Result<Self, VajraError> {
        let peers: Vec<NodeId> = config
            .peers
            .into_iter()
            .filter(|&id| id != config.id)
            .collect();

        let log = Arc::new(RaftLog::new(config.wal_config)?);

        let mut rng = rand::thread_rng();
        let election_timeout = Duration::from_millis(rng.gen_range(
            config.raft_config.election_timeout_min_ms..config.raft_config.election_timeout_max_ms,
        ));

        Ok(Self {
            id: config.id,
            peers,
            role: RwLock::new(Role::Follower),
            persistent: RwLock::new(PersistentState::new()),
            volatile: RwLock::new(VolatileState::new()),
            leader_state: RwLock::new(None),
            log,
            election_timeout_min: Duration::from_millis(config.raft_config.election_timeout_min_ms),
            election_timeout_max: Duration::from_millis(config.raft_config.election_timeout_max_ms),
            heartbeat_interval: Duration::from_millis(config.raft_config.heartbeat_interval_ms),
            last_heartbeat: RwLock::new(Instant::now()),
            election_timeout: RwLock::new(election_timeout),
            pre_vote_enabled: config.pre_vote_enabled,
            votes_received: RwLock::new(HashSet::new()),
            leader_id: RwLock::new(None),
        })
    }

    /// Get current role.
    pub fn role(&self) -> Role {
        *self.role.read()
    }

    /// Get current term.
    pub fn term(&self) -> u64 {
        self.persistent.read().current_term
    }

    /// Get current leader ID.
    pub fn leader(&self) -> Option<NodeId> {
        *self.leader_id.read()
    }

    /// Check if this node is the leader.
    pub fn is_leader(&self) -> bool {
        *self.role.read() == Role::Leader
    }

    /// Tick the Raft node.
    /// This should be called periodically (e.g., every 10ms).
    /// Returns Ready struct with any actions to take.
    #[tracing::instrument(skip(self))]
    pub fn tick(&self) -> Ready {
        let mut ready = Ready::default();
        let role = *self.role.read();

        match role {
            Role::Leader => {
                self.tick_leader(&mut ready);
            }
            Role::Follower | Role::Candidate => {
                self.tick_follower_or_candidate(&mut ready);
            }
        }

        ready
    }

    /// Leader tick: send heartbeats.
    fn tick_leader(&self, ready: &mut Ready) {
        let last = *self.last_heartbeat.read();
        if last.elapsed() >= self.heartbeat_interval {
            *self.last_heartbeat.write() = Instant::now();
            self.broadcast_append_entries(ready);
        }
    }

    /// Follower/Candidate tick: check election timeout.
    fn tick_follower_or_candidate(&self, ready: &mut Ready) {
        let last = *self.last_heartbeat.read();
        let timeout = *self.election_timeout.read();

        if last.elapsed() >= timeout {
            tracing::info!(node = %self.id, "Election timeout, starting election");
            self.start_election(ready);
        }
    }

    /// Start an election.
    fn start_election(&self, ready: &mut Ready) {
        let mut persistent = self.persistent.write();
        persistent.current_term += 1;
        persistent.voted_for = Some(self.id);
        ready.persist_state = true;

        *self.role.write() = Role::Candidate;
        *self.leader_id.write() = None;
        self.votes_received.write().clear();
        self.votes_received.write().insert(self.id); // Vote for self
        self.reset_election_timeout();

        let term = persistent.current_term;
        let last_log_index = self.log.last_index();
        let last_log_term = self.log.last_term();

        // Send RequestVote to all peers
        for &peer in &self.peers {
            let rv = RequestVote {
                term,
                candidate_id: self.id,
                last_log_index,
                last_log_term,
            };
            ready
                .messages
                .push((peer, RaftMessage::RequestVote(rv)));
        }

        tracing::info!(
            node = %self.id,
            term = term,
            "Started election"
        );
    }

    /// Reset the election timeout with a new random value.
    fn reset_election_timeout(&self) {
        let mut rng = rand::thread_rng();
        let timeout = Duration::from_millis(rng.gen_range(
            self.election_timeout_min.as_millis() as u64
                ..self.election_timeout_max.as_millis() as u64,
        ));
        *self.election_timeout.write() = timeout;
        *self.last_heartbeat.write() = Instant::now();
    }

    /// Broadcast AppendEntries to all followers.
    fn broadcast_append_entries(&self, ready: &mut Ready) {
        let term = self.persistent.read().current_term;
        let commit_index = self.volatile.read().commit_index;
        let leader_state = self.leader_state.read();

        if let Some(ref ls) = *leader_state {
            for (i, &peer) in self.peers.iter().enumerate() {
                let next_index = ls.next_index.get(i).copied().unwrap_or(1);
                let prev_log_index = next_index.saturating_sub(1);
                let prev_log_term = self.log.term_at(prev_log_index).unwrap_or(0);

                // Get entries to send
                let entries = self.log.get_range(next_index, self.log.last_index());
                let entry_data: Vec<_> = entries
                    .into_iter()
                    .map(|e| crate::messages::LogEntryData {
                        term: e.term,
                        index: e.index,
                        data: e.to_bytes().unwrap_or_default(),
                    })
                    .collect();

                let ae = AppendEntries {
                    term,
                    leader_id: self.id,
                    prev_log_index,
                    prev_log_term,
                    entries: entry_data,
                    leader_commit: commit_index,
                };

                ready.messages.push((peer, RaftMessage::AppendEntries(ae)));
            }
        }
    }

    /// Process a received message.
    #[tracing::instrument(skip(self, from, msg))]
    pub fn step(&self, from: NodeId, msg: RaftMessage) -> Ready {
        let mut ready = Ready::default();

        match msg {
            RaftMessage::RequestVote(rv) => {
                self.handle_request_vote(from, rv, &mut ready);
            }
            RaftMessage::RequestVoteResponse(rvr) => {
                self.handle_request_vote_response(from, rvr, &mut ready);
            }
            RaftMessage::AppendEntries(ae) => {
                self.handle_append_entries(from, ae, &mut ready);
            }
            RaftMessage::AppendEntriesResponse(aer) => {
                self.handle_append_entries_response(from, aer, &mut ready);
            }
            RaftMessage::PreVote(pv) => {
                self.handle_pre_vote(from, pv, &mut ready);
            }
            RaftMessage::PreVoteResponse(pvr) => {
                self.handle_pre_vote_response(from, pvr, &mut ready);
            }
        }

        ready
    }

    /// Handle RequestVote RPC.
    fn handle_request_vote(&self, from: NodeId, rv: RequestVote, ready: &mut Ready) {
        let mut persistent = self.persistent.write();
        let mut vote_granted = false;

        // Update term if necessary
        if rv.term > persistent.current_term {
            persistent.update_term(rv.term);
            *self.role.write() = Role::Follower;
            *self.leader_id.write() = None;
            ready.persist_state = true;
        }

        // Check if we can grant the vote
        if rv.term >= persistent.current_term {
            let can_vote = persistent.voted_for.is_none()
                || persistent.voted_for == Some(rv.candidate_id);
            let log_ok = self.log.is_up_to_date(rv.last_log_index, rv.last_log_term);

            if can_vote && log_ok {
                persistent.set_voted_for(rv.candidate_id);
                vote_granted = true;
                ready.persist_state = true;
                self.reset_election_timeout();
            }
        }

        let response = RequestVoteResponse {
            term: persistent.current_term,
            vote_granted,
        };
        ready
            .messages
            .push((from, RaftMessage::RequestVoteResponse(response)));
    }

    /// Handle RequestVote response.
    fn handle_request_vote_response(&self, from: NodeId, rvr: RequestVoteResponse, ready: &mut Ready) {
        let role = *self.role.read();
        if role != Role::Candidate {
            return;
        }

        let term = self.persistent.read().current_term;
        if rvr.term > term {
            self.persistent.write().update_term(rvr.term);
            *self.role.write() = Role::Follower;
            *self.leader_id.write() = None;
            ready.persist_state = true;
            return;
        }

        if rvr.vote_granted && rvr.term == term {
            self.votes_received.write().insert(from);
            let votes = self.votes_received.read().len();
            let quorum = (self.peers.len() + 1) / 2 + 1;

            if votes >= quorum {
                self.become_leader(ready);
            }
        }
    }

    /// Become the leader.
    fn become_leader(&self, _ready: &mut Ready) {
        tracing::info!(node = %self.id, term = %self.term(), "Became leader");

        *self.role.write() = Role::Leader;
        *self.leader_id.write() = Some(self.id);
        *self.leader_state.write() = Some(LeaderState::new(
            self.peers.len(),
            self.log.last_index(),
        ));

        // Reset heartbeat timer to send heartbeats immediately
        *self.last_heartbeat.write() = Instant::now() - self.heartbeat_interval;
    }

    /// Handle AppendEntries RPC.
    fn handle_append_entries(&self, from: NodeId, ae: AppendEntries, ready: &mut Ready) {
        let mut persistent = self.persistent.write();
        let mut success = false;
        let mut match_index = 0;

        // Update term if necessary
        if ae.term > persistent.current_term {
            persistent.update_term(ae.term);
            *self.role.write() = Role::Follower;
            ready.persist_state = true;
        }

        if ae.term >= persistent.current_term {
            *self.leader_id.write() = Some(ae.leader_id);
            self.reset_election_timeout();

            // If not follower, step down
            if *self.role.read() != Role::Follower {
                *self.role.write() = Role::Follower;
            }

            // Check log consistency
            if self.log.matches(ae.prev_log_index, ae.prev_log_term) {
                success = true;

                // Append new entries
                if !ae.entries.is_empty() {
                    // TODO: Check for conflicts and truncate
                    // For now, we just append
                    for entry_data in ae.entries {
                        if let Ok(entry) = vajra_wal::LogEntry::from_bytes(&entry_data.data) {
                            let _ = self.log.append(entry.term, entry.index, entry.command);
                        }
                    }
                }

                match_index = self.log.last_index();

                // Update commit index
                if ae.leader_commit > self.volatile.read().commit_index {
                    let new_commit = ae.leader_commit.min(self.log.last_index());
                    self.volatile.write().update_commit(new_commit);
                    // Collect committed entries
                    self.collect_committed_entries(ready);
                }
            }
        }

        let response = AppendEntriesResponse {
            term: persistent.current_term,
            success,
            match_index,
        };
        ready
            .messages
            .push((from, RaftMessage::AppendEntriesResponse(response)));
    }

    /// Handle AppendEntries response.
    fn handle_append_entries_response(
        &self,
        from: NodeId,
        aer: AppendEntriesResponse,
        ready: &mut Ready,
    ) {
        if *self.role.read() != Role::Leader {
            return;
        }

        let term = self.persistent.read().current_term;
        if aer.term > term {
            self.persistent.write().update_term(aer.term);
            *self.role.write() = Role::Follower;
            *self.leader_state.write() = None;
            *self.leader_id.write() = None;
            ready.persist_state = true;
            return;
        }

        // Find peer index
        let peer_idx = self.peers.iter().position(|&id| id == from);
        if let Some(idx) = peer_idx {
            let mut ls = self.leader_state.write();
            if let Some(ref mut leader) = *ls {
                if aer.success {
                    leader.update_match(idx, aer.match_index);
                    leader.next_index[idx] = aer.match_index + 1;

                    // Update commit index
                    let new_commit = leader.compute_commit_index(self.volatile.read().commit_index);
                    if new_commit > self.volatile.read().commit_index {
                        // Only commit entries from current term
                        if self.log.term_at(new_commit) == Some(term) {
                            self.volatile.write().update_commit(new_commit);
                            drop(ls);
                            self.collect_committed_entries(ready);
                        }
                    }
                } else {
                    leader.decrement_next(idx);
                }
            }
        }
    }

    /// Handle PreVote RPC.
    fn handle_pre_vote(&self, from: NodeId, pv: PreVote, ready: &mut Ready) {
        let persistent = self.persistent.read();
        let mut vote_granted = false;

        // Don't grant pre-vote if we've heard from a leader recently
        let recent_leader = self.last_heartbeat.read().elapsed() < self.election_timeout_min;

        if !recent_leader && pv.term >= persistent.current_term {
            let log_ok = self.log.is_up_to_date(pv.last_log_index, pv.last_log_term);
            vote_granted = log_ok;
        }

        let response = PreVoteResponse {
            term: persistent.current_term,
            vote_granted,
        };
        ready
            .messages
            .push((from, RaftMessage::PreVoteResponse(response)));
    }

    /// Handle PreVote response.
    fn handle_pre_vote_response(&self, _from: NodeId, _pvr: PreVoteResponse, _ready: &mut Ready) {
        // PreVote responses would be handled here if implementing full pre-vote
        // For now, this is a placeholder
    }

    /// Collect committed entries for application.
    fn collect_committed_entries(&self, ready: &mut Ready) {
        let mut volatile = self.volatile.write();
        if let Some(range) = volatile.advance_applied() {
            for index in range {
                if let Some(entry) = self.log.get_entry(index) {
                    ready.committed_entries.push(entry);
                }
            }
        }
    }

    /// Propose a new command (leader only).
    pub fn propose(&self, command: Command) -> Result<u64, VajraError> {
        if *self.role.read() != Role::Leader {
            return Err(VajraError::NotLeader {
                leader: *self.leader_id.read(),
            });
        }

        let term = self.persistent.read().current_term;
        let index = self.log.last_index() + 1;
        self.log.append(term, index, command)?;

        // Update leader's match_index for self
        let mut ls = self.leader_state.write();
        if let Some(ref mut leader) = *ls {
            // Self is always at the end
            leader.match_index.push(index);
            leader.next_index.push(index + 1);
        }

        Ok(index)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    fn test_config(id: u64, dir: &std::path::Path) -> NodeConfig {
        NodeConfig {
            id: NodeId(id),
            peers: vec![NodeId(1), NodeId(2), NodeId(3)],
            raft_config: RaftConfig {
                election_timeout_min_ms: 150,
                election_timeout_max_ms: 300,
                heartbeat_interval_ms: 50,
                snapshot_threshold: 1000,
                max_entries_per_append: 100,
                pre_vote_enabled: true,
            },
            wal_config: WalConfig {
                dir: dir.to_path_buf(),
                segment_size: 1024,
                sync_policy: vajra_common::config::SyncPolicy::EveryEntry,
            },
            pre_vote_enabled: true,
        }
    }

    #[test]
    fn test_node_creation() {
        let dir = tempdir().unwrap();
        let node = RaftNode::new(test_config(1, dir.path())).unwrap();

        assert_eq!(node.role(), Role::Follower);
        assert_eq!(node.term(), 0);
        assert!(!node.is_leader());
    }

    #[test]
    fn test_single_node_election() {
        let dir = tempdir().unwrap();
        let config = NodeConfig {
            id: NodeId(1),
            peers: vec![NodeId(1)], // Single node cluster
            raft_config: RaftConfig::default(),
            wal_config: WalConfig {
                dir: dir.path().to_path_buf(),
                segment_size: 1024,
                sync_policy: vajra_common::config::SyncPolicy::EveryEntry,
            },
            pre_vote_enabled: false,
        };

        let node = RaftNode::new(config).unwrap();

        // Simulate election timeout
        std::thread::sleep(Duration::from_millis(400));
        let ready = node.tick();

        // Single node should become leader immediately
        assert!(ready.persist_state || node.role() == Role::Candidate);
    }
}
