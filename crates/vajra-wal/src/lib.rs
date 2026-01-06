//! # Vajra Write-Ahead Log
//!
//! A crash-consistent Write-Ahead Log implementation for Vajra.
//!
//! ## Features
//!
//! - **Crash Consistency**: Survives power failures mid-write
//! - **CRC32 Checksums**: Detects corruption from hardware errors
//! - **Little Endian**: Portable across architectures
//! - **Segment Rotation**: Automatic segment management
//! - **Sync Policies**: Configurable durability vs performance
//!
//! ## Entry Format
//!
//! ```text
//! ┌─────────────┬─────────────┬──────────────────┐
//! │ Length (4B) │  CRC32 (4B) │   Payload (var)  │
//! │  u32 LE     │  u32 LE     │   [u8; length]   │
//! └─────────────┴─────────────┴──────────────────┘
//! ```
//!
//! ## Example
//!
//! ```ignore
//! use vajra_wal::{WriteAheadLog, WalConfig, LogEntry, Command};
//!
//! let config = WalConfig::default();
//! let wal = WriteAheadLog::open(config)?;
//!
//! // Append an entry
//! let entry = LogEntry::new(1, 1, Command::Noop);
//! wal.append(entry)?;
//!
//! // Force sync
//! wal.sync()?;
//! ```

#![warn(missing_docs)]
#![warn(clippy::all)]

pub mod entry;
pub mod segment;
pub mod wal;

// Re-export main types
pub use entry::{Command, LogEntry};
pub use segment::Segment;
pub use wal::{WalConfig, WriteAheadLog};
