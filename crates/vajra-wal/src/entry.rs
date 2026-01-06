//! Log entry format and serialization.
//!
//! Entry format on disk (all integers Little Endian):
//!
//! ```text
//! ┌─────────────┬─────────────┬──────────────────┐
//! │ Length (4B) │  CRC32 (4B) │   Payload (var)  │
//! │  u32 LE     │  u32 LE     │   [u8; length]   │
//! └─────────────┴─────────────┴──────────────────┘
//! ```

use byteorder::{LittleEndian, ReadBytesExt, WriteBytesExt};
use serde::{Deserialize, Serialize};
use std::io::{self, Read, Write};
use vajra_common::VajraError;

/// Size of the entry header in bytes (length + CRC32).
pub const HEADER_SIZE: usize = 8;

/// A log entry representing a state machine command.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LogEntry {
    /// The Raft term when this entry was created.
    pub term: u64,
    /// The log index of this entry.
    pub index: u64,
    /// The command payload.
    pub command: Command,
}

/// Commands that can be stored in the log.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Command {
    /// Insert a vector into the index.
    Insert {
        /// The vector ID.
        id: u64,
        /// The vector embedding data.
        vector: Vec<f32>,
        /// Optional metadata as serialized bytes.
        metadata: Option<Vec<u8>>,
    },
    /// Delete a vector from the index.
    Delete {
        /// The ID of the vector to delete.
        id: u64,
    },
    /// Batch of commands for micro-batching (performance optimization).
    /// This allows multiple operations to share a single fsync.
    Batch(Vec<Command>),
    /// No-op for leader election confirmation.
    Noop,
}

impl LogEntry {
    /// Create a new log entry.
    pub fn new(term: u64, index: u64, command: Command) -> Self {
        Self {
            term,
            index,
            command,
        }
    }

    /// Serialize the entry to bytes using bincode.
    pub fn to_bytes(&self) -> Result<Vec<u8>, VajraError> {
        bincode::serialize(self).map_err(|e| VajraError::Serialization {
            context: "serializing log entry".to_string(),
            message: e.to_string(),
        })
    }

    /// Deserialize an entry from bytes.
    pub fn from_bytes(data: &[u8]) -> Result<Self, VajraError> {
        bincode::deserialize(data).map_err(|e| VajraError::Serialization {
            context: "deserializing log entry".to_string(),
            message: e.to_string(),
        })
    }
}

/// Write a log entry to a writer with header (length + CRC32).
///
/// Returns the number of bytes written.
pub fn write_entry<W: Write>(writer: &mut W, entry: &LogEntry) -> Result<usize, VajraError> {
    let payload = entry.to_bytes()?;
    let length = payload.len() as u32;
    let crc = crc32fast::hash(&payload);

    // Write header (Little Endian)
    writer
        .write_u32::<LittleEndian>(length)
        .map_err(|e| VajraError::io("writing entry length", e))?;
    writer
        .write_u32::<LittleEndian>(crc)
        .map_err(|e| VajraError::io("writing entry CRC", e))?;

    // Write payload
    writer
        .write_all(&payload)
        .map_err(|e| VajraError::io("writing entry payload", e))?;

    Ok(HEADER_SIZE + payload.len())
}

/// Read a log entry from a reader.
///
/// Returns None if EOF is reached cleanly at the start of an entry.
/// Returns an error if the entry is corrupted (partial read, CRC mismatch).
pub fn read_entry<R: Read>(reader: &mut R) -> Result<Option<LogEntry>, VajraError> {
    // Read length
    let length = match reader.read_u32::<LittleEndian>() {
        Ok(len) => len,
        Err(e) if e.kind() == io::ErrorKind::UnexpectedEof => return Ok(None),
        Err(e) => return Err(VajraError::io("reading entry length", e)),
    };

    // Read CRC
    let expected_crc = reader
        .read_u32::<LittleEndian>()
        .map_err(|e| VajraError::io("reading entry CRC", e))?;

    // Read payload
    let mut payload = vec![0u8; length as usize];
    reader
        .read_exact(&mut payload)
        .map_err(|e| VajraError::io("reading entry payload", e))?;

    // Verify CRC
    let actual_crc = crc32fast::hash(&payload);
    if actual_crc != expected_crc {
        return Err(VajraError::WalCorruption {
            offset: 0, // Offset unknown at this point
            reason: format!(
                "CRC mismatch: expected {:#010x}, got {:#010x}",
                expected_crc, actual_crc
            ),
        });
    }

    // Deserialize
    let entry = LogEntry::from_bytes(&payload)?;
    Ok(Some(entry))
}

/// Result of attempting to read entries, indicating how far we got.
#[derive(Debug)]
pub struct ReadResult {
    /// Successfully read entries.
    pub entries: Vec<LogEntry>,
    /// Number of valid bytes (for truncation).
    pub valid_bytes: u64,
    /// Whether corruption was detected (tail should be truncated).
    pub truncate_at: Option<u64>,
}

/// Read all valid entries from a reader, detecting partial writes.
///
/// This function is tolerant of partial writes at the end of the log.
/// It returns all valid entries and the position where corruption starts.
pub fn read_all_entries<R: Read>(reader: &mut R) -> ReadResult {
    let mut entries = Vec::new();
    let mut valid_bytes: u64 = 0;
    let mut buffer = Vec::new();

    // Read entire content into buffer for position tracking
    if reader.read_to_end(&mut buffer).is_err() {
        return ReadResult {
            entries,
            valid_bytes: 0,
            truncate_at: Some(0),
        };
    }

    let mut cursor = std::io::Cursor::new(&buffer);

    loop {
        let position = cursor.position();

        match read_entry(&mut cursor) {
            Ok(Some(entry)) => {
                entries.push(entry);
                valid_bytes = cursor.position();
            }
            Ok(None) => {
                // Clean EOF
                break;
            }
            Err(_) => {
                // Corruption detected - truncate here
                return ReadResult {
                    entries,
                    valid_bytes,
                    truncate_at: Some(position),
                };
            }
        }
    }

    ReadResult {
        entries,
        valid_bytes,
        truncate_at: None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    #[test]
    fn test_entry_serialization_roundtrip() {
        let entry = LogEntry::new(
            1,
            1,
            Command::Insert {
                id: 42,
                vector: vec![1.0, 2.0, 3.0],
                metadata: None,
            },
        );

        let bytes = entry.to_bytes().unwrap();
        let recovered = LogEntry::from_bytes(&bytes).unwrap();

        assert_eq!(entry, recovered);
    }

    #[test]
    fn test_write_and_read_entry() {
        let entry = LogEntry::new(5, 10, Command::Delete { id: 123 });

        let mut buffer = Vec::new();
        write_entry(&mut buffer, &entry).unwrap();

        let mut cursor = Cursor::new(&buffer);
        let recovered = read_entry(&mut cursor).unwrap().unwrap();

        assert_eq!(entry, recovered);
    }

    #[test]
    fn test_crc_validation_detects_corruption() {
        let entry = LogEntry::new(1, 1, Command::Noop);

        let mut buffer = Vec::new();
        write_entry(&mut buffer, &entry).unwrap();

        // Corrupt the payload (last byte)
        let last_idx = buffer.len() - 1;
        buffer[last_idx] ^= 0xFF;

        let mut cursor = Cursor::new(&buffer);
        let result = read_entry(&mut cursor);

        assert!(matches!(result, Err(VajraError::WalCorruption { .. })));
    }

    #[test]
    fn test_read_entry_at_eof() {
        let buffer: Vec<u8> = Vec::new();
        let mut cursor = Cursor::new(&buffer);

        let result = read_entry(&mut cursor).unwrap();
        assert!(result.is_none());
    }

    #[test]
    fn test_corrupt_tail_recovery() {
        // Write 3 valid entries
        let mut buffer = Vec::new();
        for i in 1..=3 {
            let entry = LogEntry::new(1, i, Command::Noop);
            write_entry(&mut buffer, &entry).unwrap();
        }

        let valid_length = buffer.len();

        // Append 4 bytes of garbage (simulating power cut mid-write)
        buffer.extend_from_slice(&[0xDE, 0xAD, 0xBE, 0xEF]);

        // Recover
        let result = read_all_entries(&mut Cursor::new(&buffer));

        // Should have 3 valid entries
        assert_eq!(result.entries.len(), 3);

        // Should indicate truncation point
        assert_eq!(result.truncate_at, Some(valid_length as u64));

        // Verify entries are correct
        for (i, entry) in result.entries.iter().enumerate() {
            assert_eq!(entry.index, (i + 1) as u64);
        }
    }

    #[test]
    fn test_read_all_entries_clean() {
        let mut buffer = Vec::new();
        for i in 1..=5 {
            let entry = LogEntry::new(1, i, Command::Noop);
            write_entry(&mut buffer, &entry).unwrap();
        }

        let result = read_all_entries(&mut Cursor::new(&buffer));

        assert_eq!(result.entries.len(), 5);
        assert!(result.truncate_at.is_none());
        assert_eq!(result.valid_bytes, buffer.len() as u64);
    }

    #[test]
    fn test_little_endian_encoding() {
        let entry = LogEntry::new(1, 1, Command::Noop);

        let mut buffer = Vec::new();
        write_entry(&mut buffer, &entry).unwrap();

        // First 4 bytes should be length in Little Endian
        let length = u32::from_le_bytes([buffer[0], buffer[1], buffer[2], buffer[3]]);

        // Verify it matches payload size
        let payload = entry.to_bytes().unwrap();
        assert_eq!(length as usize, payload.len());
    }
}
