//! WAL segment management.
//!
//! A segment is a single WAL file. Segments are rotated when they reach
//! a configured size limit.

use crate::entry::{read_all_entries, write_entry, LogEntry, ReadResult};
use parking_lot::Mutex;
use std::fs::{self, File, OpenOptions};
use std::io::{BufReader, BufWriter, Write};
use std::path::{Path, PathBuf};
use vajra_common::config::SyncPolicy;
use vajra_common::VajraError;

/// A single WAL segment file.
pub struct Segment {
    /// Path to the segment file.
    path: PathBuf,
    /// Segment sequence number.
    pub sequence: u64,
    /// Current file size in bytes.
    size: u64,
    /// Maximum segment size before rotation.
    max_size: u64,
    /// File handle for writing.
    writer: Mutex<BufWriter<File>>,
    /// Sync policy.
    sync_policy: SyncPolicy,
    /// Number of entries written since last sync.
    entries_since_sync: Mutex<usize>,
}

impl Segment {
    /// Create a new segment file.
    pub fn create(
        dir: &Path,
        sequence: u64,
        max_size: u64,
        sync_policy: SyncPolicy,
    ) -> Result<Self, VajraError> {
        let path = dir.join(format!("{:020}.wal", sequence));

        let file = OpenOptions::new()
            .create(true)
            .write(true)
            .truncate(true)
            .open(&path)
            .map_err(|e| VajraError::io("creating segment file", e))?;

        Ok(Self {
            path,
            sequence,
            size: 0,
            max_size,
            writer: Mutex::new(BufWriter::new(file)),
            sync_policy,
            entries_since_sync: Mutex::new(0),
        })
    }

    /// Open an existing segment file for appending.
    pub fn open(
        path: PathBuf,
        sequence: u64,
        max_size: u64,
        sync_policy: SyncPolicy,
    ) -> Result<Self, VajraError> {
        let file = OpenOptions::new()
            .write(true)
            .append(true)
            .open(&path)
            .map_err(|e| VajraError::io("opening segment file", e))?;

        let size = file
            .metadata()
            .map_err(|e| VajraError::io("getting segment metadata", e))?
            .len();

        Ok(Self {
            path,
            sequence,
            size,
            max_size,
            writer: Mutex::new(BufWriter::new(file)),
            sync_policy,
            entries_since_sync: Mutex::new(0),
        })
    }

    /// Get the path to this segment.
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Get the current size of this segment.
    pub fn size(&self) -> u64 {
        self.size
    }

    /// Check if this segment is full.
    pub fn is_full(&self) -> bool {
        self.size >= self.max_size
    }

    /// Append an entry to this segment.
    ///
    /// Returns the number of bytes written.
    #[tracing::instrument(skip(self, entry), fields(segment = %self.sequence, index = %entry.index))]
    pub fn append(&mut self, entry: &LogEntry) -> Result<usize, VajraError> {
        let bytes_written = {
            let mut writer = self.writer.lock();
            write_entry(&mut *writer, entry)?
        };

        self.size += bytes_written as u64;

        // Handle sync policy
        self.maybe_sync()?;

        Ok(bytes_written)
    }

    /// Sync based on policy.
    fn maybe_sync(&self) -> Result<(), VajraError> {
        let mut entries_since_sync = self.entries_since_sync.lock();
        *entries_since_sync += 1;

        match self.sync_policy {
            SyncPolicy::EveryEntry => {
                self.sync()?;
                *entries_since_sync = 0;
            }
            SyncPolicy::Batched { entries, .. } => {
                if *entries_since_sync >= entries {
                    self.sync()?;
                    *entries_since_sync = 0;
                }
            }
            SyncPolicy::OsDefault => {
                // No explicit sync
            }
        }

        Ok(())
    }

    /// Force sync to disk.
    pub fn sync(&self) -> Result<(), VajraError> {
        let mut writer = self.writer.lock();
        writer
            .flush()
            .map_err(|e| VajraError::io("flushing segment buffer", e))?;
        writer
            .get_ref()
            .sync_all()
            .map_err(|e| VajraError::io("syncing segment to disk", e))?;
        Ok(())
    }

    /// Read all entries from this segment.
    pub fn read_all(&self) -> Result<ReadResult, VajraError> {
        let file = File::open(&self.path).map_err(|e| VajraError::io("opening segment for read", e))?;

        let mut reader = BufReader::new(file);
        Ok(read_all_entries(&mut reader))
    }

    /// Truncate the segment at the given position.
    ///
    /// This is used during recovery to remove corrupted tail.
    pub fn truncate(&mut self, position: u64) -> Result<(), VajraError> {
        // Sync and close the current writer
        self.sync()?;

        // Truncate the file
        let file = OpenOptions::new()
            .write(true)
            .open(&self.path)
            .map_err(|e| VajraError::io("opening segment for truncate", e))?;

        file.set_len(position)
            .map_err(|e| VajraError::io("truncating segment", e))?;

        file.sync_all()
            .map_err(|e| VajraError::io("syncing truncated segment", e))?;

        // Reopen for appending
        let file = OpenOptions::new()
            .write(true)
            .append(true)
            .open(&self.path)
            .map_err(|e| VajraError::io("reopening segment after truncate", e))?;

        self.size = position;
        *self.writer.lock() = BufWriter::new(file);

        tracing::warn!(
            segment = %self.sequence,
            truncated_to = position,
            "Truncated corrupt segment tail"
        );

        Ok(())
    }
}

/// Parse a segment filename to extract sequence number.
pub fn parse_segment_filename(filename: &str) -> Option<u64> {
    filename.strip_suffix(".wal").and_then(|s| s.parse().ok())
}

/// List all segment files in a directory, sorted by sequence.
pub fn list_segments(dir: &Path) -> Result<Vec<(u64, PathBuf)>, VajraError> {
    let mut segments = Vec::new();

    if !dir.exists() {
        return Ok(segments);
    }

    for entry in fs::read_dir(dir).map_err(|e| VajraError::io("reading WAL directory", e))? {
        let entry = entry.map_err(|e| VajraError::io("reading directory entry", e))?;
        let path = entry.path();

        if let Some(filename) = path.file_name().and_then(|n| n.to_str()) {
            if let Some(seq) = parse_segment_filename(filename) {
                segments.push((seq, path));
            }
        }
    }

    segments.sort_by_key(|(seq, _)| *seq);
    Ok(segments)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::entry::{Command, HEADER_SIZE};
    use tempfile::tempdir;

    #[test]
    fn test_segment_create_and_append() {
        let dir = tempdir().unwrap();
        let mut segment =
            Segment::create(dir.path(), 1, 1024 * 1024, SyncPolicy::EveryEntry).unwrap();

        let entry = LogEntry::new(1, 1, Command::Noop);
        let bytes = segment.append(&entry).unwrap();

        assert!(bytes > HEADER_SIZE);
        assert_eq!(segment.size(), bytes as u64);
    }

    #[test]
    fn test_segment_read_after_write() {
        let dir = tempdir().unwrap();
        let mut segment =
            Segment::create(dir.path(), 1, 1024 * 1024, SyncPolicy::EveryEntry).unwrap();

        // Write entries
        for i in 1..=5 {
            let entry = LogEntry::new(1, i, Command::Noop);
            segment.append(&entry).unwrap();
        }

        // Read back
        let result = segment.read_all().unwrap();

        assert_eq!(result.entries.len(), 5);
        assert!(result.truncate_at.is_none());
    }

    #[test]
    fn test_segment_truncate() {
        let dir = tempdir().unwrap();
        let mut segment =
            Segment::create(dir.path(), 1, 1024 * 1024, SyncPolicy::EveryEntry).unwrap();

        // Write entries
        for i in 1..=3 {
            let entry = LogEntry::new(1, i, Command::Noop);
            segment.append(&entry).unwrap();
        }

        let valid_size = segment.size();

        // Manually append garbage
        {
            let mut writer = segment.writer.lock();
            writer.write_all(&[0xDE, 0xAD, 0xBE, 0xEF]).unwrap();
            writer.flush().unwrap();
        }

        // Truncate
        segment.truncate(valid_size).unwrap();

        // Read should show 3 valid entries
        let result = segment.read_all().unwrap();
        assert_eq!(result.entries.len(), 3);
    }

    #[test]
    fn test_list_segments() {
        let dir = tempdir().unwrap();

        // Create some segments
        Segment::create(dir.path(), 1, 1024, SyncPolicy::OsDefault).unwrap();
        Segment::create(dir.path(), 3, 1024, SyncPolicy::OsDefault).unwrap();
        Segment::create(dir.path(), 2, 1024, SyncPolicy::OsDefault).unwrap();

        let segments = list_segments(dir.path()).unwrap();

        assert_eq!(segments.len(), 3);
        assert_eq!(segments[0].0, 1);
        assert_eq!(segments[1].0, 2);
        assert_eq!(segments[2].0, 3);
    }

    #[test]
    fn test_parse_segment_filename() {
        assert_eq!(
            parse_segment_filename("00000000000000000001.wal"),
            Some(1)
        );
        assert_eq!(
            parse_segment_filename("00000000000000000123.wal"),
            Some(123)
        );
        assert_eq!(parse_segment_filename("invalid.txt"), None);
        assert_eq!(parse_segment_filename("abc.wal"), None);
    }
}
