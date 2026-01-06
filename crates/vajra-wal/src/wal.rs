//! Write-Ahead Log manager.
//!
//! Manages multiple segments, rotation, and recovery.

use crate::entry::LogEntry;
use crate::segment::{list_segments, Segment};
use parking_lot::RwLock;
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use vajra_common::config::SyncPolicy;
use vajra_common::VajraError;

/// Write-Ahead Log configuration.
#[derive(Debug, Clone)]
pub struct WalConfig {
    /// Directory to store WAL files.
    pub dir: PathBuf,
    /// Maximum size of each segment in bytes.
    pub segment_size: u64,
    /// Sync policy.
    pub sync_policy: SyncPolicy,
}

impl Default for WalConfig {
    fn default() -> Self {
        Self {
            dir: PathBuf::from("data/wal"),
            segment_size: 64 * 1024 * 1024, // 64 MB
            sync_policy: SyncPolicy::Batched {
                entries: 100,
                timeout_ms: 100,
            },
        }
    }
}

/// The Write-Ahead Log.
///
/// All state machine mutations must be written to the WAL before being applied.
/// The WAL provides crash consistency by persisting entries to disk with CRC32
/// checksums for corruption detection.
pub struct WriteAheadLog {
    config: WalConfig,
    /// Current active segment for writing.
    active_segment: RwLock<Option<Segment>>,
    /// Next sequence number for new segments.
    next_sequence: RwLock<u64>,
    /// All entries currently in the WAL (cached for fast access).
    entries: RwLock<Vec<LogEntry>>,
    /// Index map for O(1) lookup: LogIndex -> position in entries vec.
    /// Enables fast "give me entries 50,000 to 55,000" for Raft replication.
    index_map: RwLock<BTreeMap<u64, usize>>,
}

impl WriteAheadLog {
    /// Create or open a Write-Ahead Log.
    ///
    /// If the directory exists, it will recover existing entries.
    /// If not, it creates a new empty WAL.
    #[tracing::instrument(skip(config))]
    pub fn open(config: WalConfig) -> Result<Self, VajraError> {
        // Ensure directory exists
        fs::create_dir_all(&config.dir).map_err(|e| VajraError::io("creating WAL directory", e))?;

        let wal = Self {
            config,
            active_segment: RwLock::new(None),
            next_sequence: RwLock::new(1),
            entries: RwLock::new(Vec::new()),
            index_map: RwLock::new(BTreeMap::new()),
        };

        wal.recover()?;
        wal.ensure_active_segment()?;

        Ok(wal)
    }

    /// Recover entries from existing segments.
    fn recover(&self) -> Result<(), VajraError> {
        let segments = list_segments(&self.config.dir)?;

        if segments.is_empty() {
            tracing::info!("No existing WAL segments found, starting fresh");
            return Ok(());
        }

        tracing::info!(segment_count = segments.len(), "Recovering WAL segments");

        let mut all_entries = Vec::new();
        let mut last_sequence = 0u64;

        for (sequence, path) in segments {
            last_sequence = sequence;

            let mut segment = Segment::open(
                path.clone(),
                sequence,
                self.config.segment_size,
                self.config.sync_policy.clone(),
            )?;

            let result = segment.read_all()?;

            tracing::info!(
                segment = sequence,
                entries = result.entries.len(),
                valid_bytes = result.valid_bytes,
                "Read segment"
            );

            // If corruption detected, truncate
            if let Some(truncate_pos) = result.truncate_at {
                tracing::warn!(
                    segment = sequence,
                    position = truncate_pos,
                    discarded_bytes = segment.size() - truncate_pos,
                    "Truncating corrupted tail"
                );
                segment.truncate(truncate_pos)?;
            }

            all_entries.extend(result.entries);
        }

        // Update state and build index map
        let mut entries_guard = self.entries.write();
        let mut index_map_guard = self.index_map.write();
        
        for (pos, entry) in all_entries.iter().enumerate() {
            index_map_guard.insert(entry.index, pos);
        }
        *entries_guard = all_entries;
        drop(entries_guard);
        drop(index_map_guard);
        
        *self.next_sequence.write() = last_sequence + 1;

        let entry_count = self.entries.read().len();
        tracing::info!(
            total_entries = entry_count,
            next_sequence = last_sequence + 1,
            "WAL recovery complete"
        );

        Ok(())
    }

    /// Ensure we have an active segment for writing.
    fn ensure_active_segment(&self) -> Result<(), VajraError> {
        let mut active = self.active_segment.write();

        // Check if current segment exists and is not full
        if let Some(ref segment) = *active {
            if !segment.is_full() {
                return Ok(());
            }
        }

        // Create new segment
        let sequence = {
            let mut next = self.next_sequence.write();
            let seq = *next;
            *next += 1;
            seq
        };

        let segment = Segment::create(
            &self.config.dir,
            sequence,
            self.config.segment_size,
            self.config.sync_policy.clone(),
        )?;

        tracing::debug!(segment = sequence, "Created new WAL segment");

        *active = Some(segment);
        Ok(())
    }

    /// Append an entry to the WAL.
    ///
    /// This will rotate to a new segment if the current one is full.
    #[tracing::instrument(skip(self, entry), fields(index = %entry.index))]
    pub fn append(&self, entry: LogEntry) -> Result<(), VajraError> {
        // Rotate if needed
        {
            let active = self.active_segment.read();
            if active.as_ref().map_or(true, |s| s.is_full()) {
                drop(active);
                self.ensure_active_segment()?;
            }
        }

        // Write to active segment
        {
            let mut active = self.active_segment.write();
            if let Some(ref mut segment) = *active {
                segment.append(&entry)?;
            }
        }

        // Cache entry and update index map
        let log_index = entry.index;
        let mut entries = self.entries.write();
        let position = entries.len();
        entries.push(entry);
        drop(entries);
        
        self.index_map.write().insert(log_index, position);

        Ok(())
    }

    /// Get all entries in the WAL.
    pub fn entries(&self) -> Vec<LogEntry> {
        self.entries.read().clone()
    }

    /// Get entries starting from a given index.
    pub fn entries_from(&self, start_index: u64) -> Vec<LogEntry> {
        self.entries
            .read()
            .iter()
            .filter(|e| e.index >= start_index)
            .cloned()
            .collect()
    }

    /// Get the last entry in the WAL.
    pub fn last_entry(&self) -> Option<LogEntry> {
        self.entries.read().last().cloned()
    }

    /// Get the last log index in the WAL.
    pub fn last_index(&self) -> u64 {
        self.entries.read().last().map_or(0, |e| e.index)
    }

    /// Get the number of entries in the WAL.
    pub fn len(&self) -> usize {
        self.entries.read().len()
    }

    /// Check if the WAL is empty.
    pub fn is_empty(&self) -> bool {
        self.entries.read().is_empty()
    }

    /// Force sync all pending writes to disk.
    pub fn sync(&self) -> Result<(), VajraError> {
        let active = self.active_segment.read();
        if let Some(ref segment) = *active {
            segment.sync()?;
        }
        Ok(())
    }

    /// Get the WAL directory.
    pub fn dir(&self) -> &Path {
        &self.config.dir
    }

    // =========================================================================
    // Efficient Index-based Lookups (for Raft replication)
    // =========================================================================

    /// Get a single entry by its log index. O(1) lookup.
    pub fn get_entry(&self, index: u64) -> Option<LogEntry> {
        let index_map = self.index_map.read();
        let position = index_map.get(&index)?;
        self.entries.read().get(*position).cloned()
    }

    /// Get the term at a given log index. O(1) lookup.
    pub fn term_at(&self, index: u64) -> Option<u64> {
        self.get_entry(index).map(|e| e.term)
    }

    /// Get a range of entries [start_index, end_index]. O(k) where k = range size.
    ///
    /// This is used for Raft log replication: "Send me entries 50,000 to 55,000".
    pub fn get_range(&self, start_index: u64, end_index: u64) -> Vec<LogEntry> {
        let index_map = self.index_map.read();
        let entries = self.entries.read();

        // Use BTreeMap range for efficient iteration
        index_map
            .range(start_index..=end_index)
            .filter_map(|(_, &pos)| entries.get(pos).cloned())
            .collect()
    }

    /// Truncate all entries after (and including) the given index.
    ///
    /// Used when a Raft follower receives conflicting entries from leader.
    /// This removes entries from memory cache only (segment files are not modified).
    pub fn truncate_after(&self, index: u64) {
        let mut entries = self.entries.write();
        let mut index_map = self.index_map.write();

        // Find first position to remove
        if let Some(&start_pos) = index_map.get(&index) {
            entries.truncate(start_pos);
        }

        // Remove all indices >= index
        let to_remove: Vec<u64> = index_map
            .range(index..)
            .map(|(&idx, _)| idx)
            .collect();
        
        for idx in to_remove {
            index_map.remove(&idx);
        }

        tracing::warn!(from_index = index, "Truncated WAL entries in memory");
    }

    /// Get the first log index in the WAL.
    pub fn first_index(&self) -> u64 {
        self.entries.read().first().map_or(0, |e| e.index)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::entry::Command;
    use tempfile::tempdir;

    fn test_config(dir: &Path) -> WalConfig {
        WalConfig {
            dir: dir.to_path_buf(),
            segment_size: 1024, // Small for testing rotation
            sync_policy: SyncPolicy::EveryEntry,
        }
    }

    #[test]
    fn test_wal_create_empty() {
        let dir = tempdir().unwrap();
        let wal = WriteAheadLog::open(test_config(dir.path())).unwrap();

        assert!(wal.is_empty());
        assert_eq!(wal.last_index(), 0);
    }

    #[test]
    fn test_wal_append_and_read() {
        let dir = tempdir().unwrap();
        let wal = WriteAheadLog::open(test_config(dir.path())).unwrap();

        for i in 1..=5 {
            let entry = LogEntry::new(1, i, Command::Noop);
            wal.append(entry).unwrap();
        }

        assert_eq!(wal.len(), 5);
        assert_eq!(wal.last_index(), 5);

        let entries = wal.entries();
        assert_eq!(entries[0].index, 1);
        assert_eq!(entries[4].index, 5);
    }

    #[test]
    fn test_wal_recovery() {
        let dir = tempdir().unwrap();

        // Write some entries
        {
            let wal = WriteAheadLog::open(test_config(dir.path())).unwrap();

            for i in 1..=10 {
                let entry = LogEntry::new(1, i, Command::Noop);
                wal.append(entry).unwrap();
            }

            wal.sync().unwrap();
        }

        // Reopen and verify recovery
        {
            let wal = WriteAheadLog::open(test_config(dir.path())).unwrap();

            assert_eq!(wal.len(), 10);
            assert_eq!(wal.last_index(), 10);
        }
    }

    #[test]
    fn test_wal_corrupt_tail_recovery() {
        let dir = tempdir().unwrap();

        // Write some entries
        {
            let wal = WriteAheadLog::open(test_config(dir.path())).unwrap();

            for i in 1..=3 {
                let entry = LogEntry::new(1, i, Command::Noop);
                wal.append(entry).unwrap();
            }

            wal.sync().unwrap();
        }

        // Append garbage to simulate power cut
        {
            let segments = list_segments(dir.path()).unwrap();
            let last_segment = &segments.last().unwrap().1;

            let mut file = fs::OpenOptions::new()
                .append(true)
                .open(last_segment)
                .unwrap();

            use std::io::Write;
            file.write_all(&[0xDE, 0xAD, 0xBE, 0xEF]).unwrap();
            file.sync_all().unwrap();
        }

        // Reopen - should recover 3 valid entries
        {
            let wal = WriteAheadLog::open(test_config(dir.path())).unwrap();

            assert_eq!(wal.len(), 3, "Should have exactly 3 entries after recovery");
            assert_eq!(wal.last_index(), 3);

            // Verify entries are correct
            let entries = wal.entries();
            for (i, entry) in entries.iter().enumerate() {
                assert_eq!(entry.index, (i + 1) as u64);
            }
        }
    }

    #[test]
    fn test_wal_segment_rotation() {
        let dir = tempdir().unwrap();

        let config = WalConfig {
            dir: dir.path().to_path_buf(),
            segment_size: 100, // Very small to force rotation
            sync_policy: SyncPolicy::EveryEntry,
        };

        let wal = WriteAheadLog::open(config).unwrap();

        // Write entries until we rotate
        for i in 1..=20 {
            let entry = LogEntry::new(
                1,
                i,
                Command::Insert {
                    id: i,
                    vector: vec![1.0, 2.0, 3.0],
                    metadata: None,
                },
            );
            wal.append(entry).unwrap();
        }

        // Should have multiple segments
        let segments = list_segments(dir.path()).unwrap();
        assert!(segments.len() > 1, "Should have rotated to multiple segments");

        // All entries should be recoverable
        assert_eq!(wal.len(), 20);
    }

    #[test]
    fn test_entries_from() {
        let dir = tempdir().unwrap();
        let wal = WriteAheadLog::open(test_config(dir.path())).unwrap();

        for i in 1..=10 {
            let entry = LogEntry::new(1, i, Command::Noop);
            wal.append(entry).unwrap();
        }

        let from_5 = wal.entries_from(5);
        assert_eq!(from_5.len(), 6); // Entries 5, 6, 7, 8, 9, 10
        assert_eq!(from_5[0].index, 5);
    }
}
