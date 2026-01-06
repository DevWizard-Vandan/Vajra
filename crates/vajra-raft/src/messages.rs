//! Raft protocol messages.
//!
//! These message types implement the Raft consensus protocol RPCs.

use serde::{Deserialize, Serialize};
use vajra_common::NodeId;

/// RequestVote RPC - sent by candidates to gather votes.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RequestVote {
    /// Candidate's term.
    pub term: u64,
    /// Candidate requesting vote.
    pub candidate_id: NodeId,
    /// Index of candidate's last log entry.
    pub last_log_index: u64,
    /// Term of candidate's last log entry.
    pub last_log_term: u64,
}

/// RequestVote RPC response.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RequestVoteResponse {
    /// Current term, for candidate to update itself.
    pub term: u64,
    /// True if candidate received vote.
    pub vote_granted: bool,
}

/// PreVote RPC - sent before RequestVote to check if election would succeed.
/// Prevents term inflation during network partitions.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PreVote {
    /// Candidate's term (would be next term if elected).
    pub term: u64,
    /// Candidate requesting pre-vote.
    pub candidate_id: NodeId,
    /// Index of candidate's last log entry.
    pub last_log_index: u64,
    /// Term of candidate's last log entry.
    pub last_log_term: u64,
}

/// PreVote RPC response.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PreVoteResponse {
    /// Current term.
    pub term: u64,
    /// True if candidate would receive vote.
    pub vote_granted: bool,
}

/// AppendEntries RPC - sent by leader for log replication and heartbeats.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppendEntries {
    /// Leader's term.
    pub term: u64,
    /// Leader ID so follower can redirect clients.
    pub leader_id: NodeId,
    /// Index of log entry immediately preceding new ones.
    pub prev_log_index: u64,
    /// Term of prev_log_index entry.
    pub prev_log_term: u64,
    /// Log entries to store (empty for heartbeat).
    pub entries: Vec<LogEntryData>,
    /// Leader's commit index.
    pub leader_commit: u64,
}

/// AppendEntries RPC response.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppendEntriesResponse {
    /// Current term, for leader to update itself.
    pub term: u64,
    /// True if follower contained entry matching prev_log_index and prev_log_term.
    pub success: bool,
    /// For optimization: last index replicated if success, or conflict index if failure.
    pub match_index: u64,
}

/// Log entry data for network transfer.
/// This is separate from the WAL LogEntry to allow for network-specific optimizations.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LogEntryData {
    /// The term when this entry was created.
    pub term: u64,
    /// The log index of this entry.
    pub index: u64,
    /// The serialized command data.
    pub data: Vec<u8>,
}

/// Wrapper for all Raft messages.
#[derive(Debug, Clone)]
pub enum RaftMessage {
    /// RequestVote request.
    RequestVote(RequestVote),
    /// RequestVote response.
    RequestVoteResponse(RequestVoteResponse),
    /// PreVote request.
    PreVote(PreVote),
    /// PreVote response.
    PreVoteResponse(PreVoteResponse),
    /// AppendEntries request.
    AppendEntries(AppendEntries),
    /// AppendEntries response.
    AppendEntriesResponse(AppendEntriesResponse),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_request_vote_creation() {
        let rv = RequestVote {
            term: 1,
            candidate_id: NodeId(1),
            last_log_index: 10,
            last_log_term: 1,
        };
        assert_eq!(rv.term, 1);
        assert_eq!(rv.candidate_id, NodeId(1));
    }

    #[test]
    fn test_append_entries_heartbeat() {
        let ae = AppendEntries {
            term: 2,
            leader_id: NodeId(1),
            prev_log_index: 5,
            prev_log_term: 1,
            entries: Vec::new(), // Empty = heartbeat
            leader_commit: 4,
        };
        assert!(ae.entries.is_empty()); // This is a heartbeat
    }
}
