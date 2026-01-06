//! Raft log wrapper around the WAL.
//!
//! This module provides a Raft-specific interface to the Write-Ahead Log.

use vajra_common::VajraError;
use vajra_wal::{Command, LogEntry, WalConfig, WriteAheadLog};

/// Raft log wrapper.
/// Provides Raft-specific operations on top of the WAL.
pub struct RaftLog {
    wal: WriteAheadLog,
}

impl RaftLog {
    /// Create a new Raft log.
    pub fn new(config: WalConfig) -> Result<Self, VajraError> {
        Ok(Self {
            wal: WriteAheadLog::open(config)?,
        })
    }

    /// Get the last log index.
    pub fn last_index(&self) -> u64 {
        self.wal.last_index()
    }

    /// Get the first log index (for log compaction).
    pub fn first_index(&self) -> u64 {
        self.wal.first_index()
    }

    /// Get the term at a given index.
    pub fn term_at(&self, index: u64) -> Option<u64> {
        self.wal.term_at(index)
    }

    /// Get the last log term.
    pub fn last_term(&self) -> u64 {
        self.wal.last_entry().map_or(0, |e| e.term)
    }

    /// Append a new entry.
    pub fn append(&self, term: u64, index: u64, command: Command) -> Result<(), VajraError> {
        let entry = LogEntry::new(term, index, command);
        self.wal.append(entry)
    }

    /// Append multiple entries in a batch (for replication).
    pub fn append_entries(&self, entries: Vec<LogEntry>) -> Result<(), VajraError> {
        for entry in entries {
            self.wal.append(entry)?;
        }
        Ok(())
    }

    /// Get entries in a range [start, end].
    pub fn get_range(&self, start_index: u64, end_index: u64) -> Vec<LogEntry> {
        self.wal.get_range(start_index, end_index)
    }

    /// Get a single entry by index.
    pub fn get_entry(&self, index: u64) -> Option<LogEntry> {
        self.wal.get_entry(index)
    }

    /// Truncate entries from the given index onwards.
    /// Used when receiving conflicting entries from leader.
    pub fn truncate_from(&self, index: u64) {
        self.wal.truncate_after(index);
    }

    /// Check if our log is at least as up-to-date as the candidate's.
    /// Used in vote decision.
    pub fn is_up_to_date(&self, candidate_last_index: u64, candidate_last_term: u64) -> bool {
        let our_last_term = self.last_term();
        let our_last_index = self.last_index();

        // Raft paper Section 5.4.1:
        // Compare last terms first, then indices
        if candidate_last_term != our_last_term {
            candidate_last_term > our_last_term
        } else {
            candidate_last_index >= our_last_index
        }
    }

    /// Check if log contains entry at index with matching term.
    pub fn matches(&self, index: u64, term: u64) -> bool {
        if index == 0 {
            return true; // Index 0 always matches (sentinel)
        }
        self.term_at(index).map_or(false, |t| t == term)
    }

    /// Sync to disk.
    pub fn sync(&self) -> Result<(), VajraError> {
        self.wal.sync()
    }

    /// Check if the log is empty.
    pub fn is_empty(&self) -> bool {
        self.wal.is_empty()
    }

    /// Get the number of entries.
    pub fn len(&self) -> usize {
        self.wal.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    fn test_config(dir: &std::path::Path) -> WalConfig {
        WalConfig {
            dir: dir.to_path_buf(),
            segment_size: 1024,
            sync_policy: vajra_common::config::SyncPolicy::EveryEntry,
        }
    }

    #[test]
    fn test_raft_log_basic() {
        let dir = tempdir().unwrap();
        let log = RaftLog::new(test_config(dir.path())).unwrap();

        assert!(log.is_empty());
        assert_eq!(log.last_index(), 0);
        assert_eq!(log.last_term(), 0);
    }

    #[test]
    fn test_raft_log_append_and_retrieve() {
        let dir = tempdir().unwrap();
        let log = RaftLog::new(test_config(dir.path())).unwrap();

        log.append(1, 1, Command::Noop).unwrap();
        log.append(1, 2, Command::Noop).unwrap();
        log.append(2, 3, Command::Noop).unwrap();

        assert_eq!(log.len(), 3);
        assert_eq!(log.last_index(), 3);
        assert_eq!(log.last_term(), 2);

        let entry = log.get_entry(2).unwrap();
        assert_eq!(entry.term, 1);
        assert_eq!(entry.index, 2);
    }

    #[test]
    fn test_raft_log_is_up_to_date() {
        let dir = tempdir().unwrap();
        let log = RaftLog::new(test_config(dir.path())).unwrap();

        log.append(1, 1, Command::Noop).unwrap();
        log.append(2, 2, Command::Noop).unwrap();

        // Candidate with higher term is more up-to-date
        assert!(log.is_up_to_date(1, 3));

        // Candidate with same term but higher index is more up-to-date
        assert!(log.is_up_to_date(3, 2));

        // Candidate with lower term is not up-to-date
        assert!(!log.is_up_to_date(5, 1));
    }

    #[test]
    fn test_raft_log_matches() {
        let dir = tempdir().unwrap();
        let log = RaftLog::new(test_config(dir.path())).unwrap();

        log.append(1, 1, Command::Noop).unwrap();
        log.append(2, 2, Command::Noop).unwrap();

        assert!(log.matches(0, 0)); // Sentinel
        assert!(log.matches(1, 1)); // Exact match
        assert!(log.matches(2, 2)); // Exact match
        assert!(!log.matches(2, 1)); // Wrong term
        assert!(!log.matches(3, 1)); // Index doesn't exist
    }
}
