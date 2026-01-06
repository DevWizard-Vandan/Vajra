//! Comprehensive error types for Vajra.
//!
//! This module defines the [`VajraError`] enum which covers all possible
//! failure modes across the system:
//!
//! - **Engine errors**: Vector operations (dimension mismatch, not found, capacity)
//! - **Storage errors**: WAL and persistence (corruption, checksum failures)
//! - **Consensus errors**: Raft protocol (not leader, term conflicts, log issues)
//! - **Network errors**: Communication failures (unreachable, timeout, no quorum)

use crate::types::{NodeId, VectorId};
use std::time::Duration;
use thiserror::Error;

/// The main error type for Vajra operations.
///
/// This enum comprehensively covers all failure modes that can occur
/// in the distributed vector database system.
#[derive(Debug, Error)]
pub enum VajraError {
    // =========================================================================
    // Engine Errors - Vector index operations
    // =========================================================================
    /// Vector dimension does not match the index configuration.
    #[error("vector dimension mismatch: expected {expected}, got {actual}")]
    DimensionMismatch {
        /// Expected dimension from index configuration
        expected: usize,
        /// Actual dimension of the provided vector
        actual: usize,
    },

    /// The requested vector was not found in the index.
    #[error("vector not found: {id}")]
    VectorNotFound {
        /// The ID of the missing vector
        id: VectorId,
    },

    /// The index has reached its maximum capacity.
    #[error("index capacity exceeded: {current}/{max}")]
    CapacityExceeded {
        /// Current number of vectors
        current: usize,
        /// Maximum allowed vectors
        max: usize,
    },

    /// A vector with this ID already exists.
    #[error("vector already exists: {id}")]
    VectorAlreadyExists {
        /// The ID of the existing vector
        id: VectorId,
    },

    /// The index is empty and cannot perform searches.
    #[error("index is empty")]
    EmptyIndex,

    /// Invalid search parameters were provided.
    #[error("invalid search parameters: {reason}")]
    InvalidSearchParams {
        /// Description of what's wrong
        reason: String,
    },

    // =========================================================================
    // Storage Errors - WAL and persistence
    // =========================================================================
    /// WAL corruption detected at a specific offset.
    #[error("WAL corruption at offset {offset}: {reason}")]
    WalCorruption {
        /// Byte offset where corruption was detected
        offset: u64,
        /// Description of the corruption
        reason: String,
    },

    /// Checksum validation failed.
    #[error("checksum mismatch: expected {expected:#x}, got {actual:#x}")]
    ChecksumMismatch {
        /// Expected checksum value
        expected: u32,
        /// Actual computed checksum
        actual: u32,
    },

    /// Failed to open or create a file.
    #[error("IO error: {context}: {source}")]
    Io {
        /// What operation was being attempted
        context: String,
        /// The underlying IO error
        #[source]
        source: std::io::Error,
    },

    /// Serialization/deserialization failed.
    #[error("serialization error: {context}: {message}")]
    Serialization {
        /// What was being serialized/deserialized
        context: String,
        /// Error message
        message: String,
    },

    /// Snapshot operation failed.
    #[error("snapshot error: {reason}")]
    Snapshot {
        /// Description of the failure
        reason: String,
    },

    // =========================================================================
    // Consensus Errors - Raft protocol
    // =========================================================================
    /// This node is not the leader and cannot process write requests.
    #[error("not leader: current leader is {leader:?}")]
    NotLeader {
        /// The current known leader, if any
        leader: Option<NodeId>,
    },

    /// The requested log entry has been compacted and is no longer available.
    #[error("log compacted: requested index {requested}, first available {available}")]
    LogCompacted {
        /// The requested log index
        requested: u64,
        /// The first available log index
        available: u64,
    },

    /// The term in the request is outdated.
    #[error("term outdated: current {current}, received {received}")]
    TermOutdated {
        /// Current term on this node
        current: u64,
        /// Term received in the request
        received: u64,
    },

    /// A proposal was rejected (e.g., due to leadership change).
    #[error("proposal rejected: {reason}")]
    ProposalRejected {
        /// Reason for rejection
        reason: String,
    },

    /// Log entry conflict during replication.
    #[error("log conflict at index {index}: expected term {expected_term}, got {actual_term}")]
    LogConflict {
        /// The conflicting log index
        index: u64,
        /// Expected term at this index
        expected_term: u64,
        /// Actual term found
        actual_term: u64,
    },

    // =========================================================================
    // Network Errors - Communication
    // =========================================================================
    /// A node is unreachable after multiple attempts.
    #[error("node unreachable: {node_id} after {attempts} attempts")]
    NodeUnreachable {
        /// The unreachable node's ID
        node_id: NodeId,
        /// Number of connection attempts made
        attempts: u32,
    },

    /// Operation timed out.
    #[error("request timeout after {duration:?}")]
    Timeout {
        /// How long we waited
        duration: Duration,
    },

    /// The cluster does not have enough nodes for quorum.
    #[error("cluster has no quorum: {available}/{required} nodes")]
    NoQuorum {
        /// Number of available nodes
        available: usize,
        /// Number of nodes required for quorum
        required: usize,
    },

    /// Connection to a peer failed.
    #[error("connection failed to {endpoint}: {reason}")]
    ConnectionFailed {
        /// The endpoint we tried to connect to
        endpoint: String,
        /// Reason for failure
        reason: String,
    },

    /// gRPC transport error.
    #[error("transport error: {message}")]
    Transport {
        /// Error message
        message: String,
    },

    // =========================================================================
    // Configuration Errors
    // =========================================================================
    /// Invalid configuration.
    #[error("configuration error: {message}")]
    Configuration {
        /// Description of the configuration issue
        message: String,
    },

    /// Node ID conflict.
    #[error("node ID {id} already exists in cluster")]
    NodeIdConflict {
        /// The conflicting node ID
        id: NodeId,
    },

    // =========================================================================
    // Internal Errors
    // =========================================================================
    /// An internal invariant was violated.
    #[error("internal error: {message}")]
    Internal {
        /// Description of the internal error
        message: String,
    },

    /// The operation was cancelled.
    #[error("operation cancelled")]
    Cancelled,

    /// The system is shutting down.
    #[error("system shutting down")]
    ShuttingDown,
}

impl VajraError {
    /// Create an IO error with context.
    pub fn io(context: impl Into<String>, source: std::io::Error) -> Self {
        Self::Io {
            context: context.into(),
            source,
        }
    }

    /// Create a serialization error.
    pub fn serialization(context: impl Into<String>, message: impl Into<String>) -> Self {
        Self::Serialization {
            context: context.into(),
            message: message.into(),
        }
    }

    /// Create an internal error.
    pub fn internal(message: impl Into<String>) -> Self {
        Self::Internal {
            message: message.into(),
        }
    }

    /// Check if this error indicates a retriable condition.
    #[must_use]
    pub const fn is_retriable(&self) -> bool {
        matches!(
            self,
            Self::NotLeader { .. }
                | Self::NodeUnreachable { .. }
                | Self::Timeout { .. }
                | Self::NoQuorum { .. }
                | Self::ConnectionFailed { .. }
        )
    }

    /// Check if this error indicates a fatal/unrecoverable condition.
    #[must_use]
    pub const fn is_fatal(&self) -> bool {
        matches!(
            self,
            Self::WalCorruption { .. } | Self::ChecksumMismatch { .. } | Self::Internal { .. }
        )
    }

    /// Get an error code suitable for metrics/logging.
    #[must_use]
    pub const fn error_code(&self) -> &'static str {
        match self {
            Self::DimensionMismatch { .. } => "DIMENSION_MISMATCH",
            Self::VectorNotFound { .. } => "VECTOR_NOT_FOUND",
            Self::CapacityExceeded { .. } => "CAPACITY_EXCEEDED",
            Self::VectorAlreadyExists { .. } => "VECTOR_EXISTS",
            Self::EmptyIndex => "EMPTY_INDEX",
            Self::InvalidSearchParams { .. } => "INVALID_PARAMS",
            Self::WalCorruption { .. } => "WAL_CORRUPTION",
            Self::ChecksumMismatch { .. } => "CHECKSUM_MISMATCH",
            Self::Io { .. } => "IO_ERROR",
            Self::Serialization { .. } => "SERIALIZATION_ERROR",
            Self::Snapshot { .. } => "SNAPSHOT_ERROR",
            Self::NotLeader { .. } => "NOT_LEADER",
            Self::LogCompacted { .. } => "LOG_COMPACTED",
            Self::TermOutdated { .. } => "TERM_OUTDATED",
            Self::ProposalRejected { .. } => "PROPOSAL_REJECTED",
            Self::LogConflict { .. } => "LOG_CONFLICT",
            Self::NodeUnreachable { .. } => "NODE_UNREACHABLE",
            Self::Timeout { .. } => "TIMEOUT",
            Self::NoQuorum { .. } => "NO_QUORUM",
            Self::ConnectionFailed { .. } => "CONNECTION_FAILED",
            Self::Transport { .. } => "TRANSPORT_ERROR",
            Self::Configuration { .. } => "CONFIG_ERROR",
            Self::NodeIdConflict { .. } => "NODE_ID_CONFLICT",
            Self::Internal { .. } => "INTERNAL_ERROR",
            Self::Cancelled => "CANCELLED",
            Self::ShuttingDown => "SHUTTING_DOWN",
        }
    }
}

// Implement From for common error types
impl From<std::io::Error> for VajraError {
    fn from(err: std::io::Error) -> Self {
        Self::Io {
            context: "IO operation".into(),
            source: err,
        }
    }
}

impl From<serde_json::Error> for VajraError {
    fn from(err: serde_json::Error) -> Self {
        Self::Serialization {
            context: "JSON".into(),
            message: err.to_string(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_dimension_mismatch_display() {
        let err = VajraError::DimensionMismatch {
            expected: 128,
            actual: 256,
        };
        assert_eq!(
            format!("{err}"),
            "vector dimension mismatch: expected 128, got 256"
        );
    }

    #[test]
    fn test_not_leader_display() {
        let err = VajraError::NotLeader {
            leader: Some(NodeId(1)),
        };
        assert_eq!(
            format!("{err}"),
            "not leader: current leader is Some(NodeId(1))"
        );
    }

    #[test]
    fn test_is_retriable() {
        assert!(VajraError::NotLeader { leader: None }.is_retriable());
        assert!(VajraError::Timeout {
            duration: Duration::from_secs(5)
        }
        .is_retriable());
        assert!(!VajraError::DimensionMismatch {
            expected: 128,
            actual: 256
        }
        .is_retriable());
    }

    #[test]
    fn test_is_fatal() {
        assert!(VajraError::WalCorruption {
            offset: 0,
            reason: "test".into()
        }
        .is_fatal());
        assert!(VajraError::ChecksumMismatch {
            expected: 0,
            actual: 1
        }
        .is_fatal());
        assert!(!VajraError::NotLeader { leader: None }.is_fatal());
    }

    #[test]
    fn test_error_codes() {
        assert_eq!(
            VajraError::DimensionMismatch {
                expected: 0,
                actual: 0
            }
            .error_code(),
            "DIMENSION_MISMATCH"
        );
        assert_eq!(
            VajraError::NotLeader { leader: None }.error_code(),
            "NOT_LEADER"
        );
    }
}
