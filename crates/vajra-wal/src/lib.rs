//! # Vajra WAL
//!
//! Write-Ahead Log implementation for crash-consistent persistence.
//!
//! This crate provides a durable, crash-consistent log for storing
//! Raft log entries before they are applied to the state machine.
//!
//! ## Features
//!
//! - **Crash Consistency**: CRC32 checksums detect corruption
//! - **Segment Rotation**: Automatic log file rotation
//! - **Configurable Sync**: Trade durability for performance
//! - **Fast Recovery**: Efficient log replay on startup
//!
//! ## Log Entry Format
//!
//! ```text
//! ┌─────────┬─────────┬──────────┬─────────────────────┬────────────┐
//! │ Magic   │ Length  │ CRC32    │ Payload             │ Padding    │
//! │ 4 bytes │ 4 bytes │ 4 bytes  │ variable            │ to 8-align │
//! └─────────┴─────────┴──────────┴─────────────────────┴────────────┘
//! ```

#![warn(missing_docs)]
#![warn(clippy::all)]
#![warn(clippy::pedantic)]

// Modules will be implemented in Phase 2
// pub mod entry;
// pub mod segment;
// pub mod wal;
// pub mod recovery;

/// Placeholder for the Write-Ahead Log (to be implemented in Phase 2)
pub struct WriteAheadLog {
    _private: (),
}

impl WriteAheadLog {
    /// Create a new WAL (placeholder)
    #[must_use]
    pub fn new() -> Self {
        Self { _private: () }
    }
}

impl Default for WriteAheadLog {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_placeholder() {
        let _wal = WriteAheadLog::new();
    }
}
