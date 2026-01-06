//! Error mapping from VajraError to tonic::Status.
//!
//! This module provides proper gRPC error codes for all Vajra errors.

use tonic::{Code, Status};
use vajra_common::VajraError;

/// Extension trait for converting VajraError to tonic::Status.
///
/// We use a trait instead of `impl From<VajraError> for Status` because
/// neither type is defined in this crate (orphan rule).
pub trait IntoStatus {
    /// Convert to a gRPC Status with appropriate error code.
    fn into_status(self) -> Status;
}

impl IntoStatus for VajraError {
    fn into_status(self) -> Status {
        let code = match &self {
            // Client errors (invalid input)
            VajraError::DimensionMismatch { .. } => Code::InvalidArgument,
            VajraError::InvalidSearchParams { .. } => Code::InvalidArgument,
            VajraError::Configuration { .. } => Code::InvalidArgument,

            // Not found errors
            VajraError::VectorNotFound { .. } => Code::NotFound,

            // Already exists
            VajraError::VectorAlreadyExists { .. } => Code::AlreadyExists,

            // Resource exhaustion
            VajraError::CapacityExceeded { .. } => Code::ResourceExhausted,

            // Precondition failures
            VajraError::EmptyIndex => Code::FailedPrecondition,
            VajraError::LogCompacted { .. } => Code::FailedPrecondition,

            // Leadership / consensus errors - retry on another node
            VajraError::NotLeader { .. } => Code::Unavailable,
            VajraError::NoQuorum { .. } => Code::Unavailable,
            VajraError::NodeUnreachable { .. } => Code::Unavailable,
            VajraError::ConnectionFailed { .. } => Code::Unavailable,

            // Timeouts
            VajraError::Timeout { .. } => Code::DeadlineExceeded,

            // Conflicts
            VajraError::TermOutdated { .. } => Code::Aborted,
            VajraError::LogConflict { .. } => Code::Aborted,
            VajraError::ProposalRejected { .. } => Code::Aborted,
            VajraError::NodeIdConflict { .. } => Code::Aborted,

            // Cancellation
            VajraError::Cancelled => Code::Cancelled,
            VajraError::ShuttingDown => Code::Cancelled,

            // Storage/internal errors - should not expose details
            VajraError::WalCorruption { .. } => Code::DataLoss,
            VajraError::ChecksumMismatch { .. } => Code::DataLoss,
            VajraError::Io { .. } => Code::Internal,
            VajraError::Serialization { .. } => Code::Internal,
            VajraError::Snapshot { .. } => Code::Internal,
            VajraError::Transport { .. } => Code::Internal,
            VajraError::Internal { .. } => Code::Internal,
        };

        Status::new(code, self.to_string())
    }
}

/// Create a Status from a VajraError with additional context.
pub fn status_with_context(err: VajraError, context: &str) -> Status {
    let base_status = err.into_status();
    Status::new(base_status.code(), format!("{}: {}", context, base_status.message()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;
    use vajra_common::VectorId;

    #[test]
    fn test_dimension_mismatch_is_invalid_argument() {
        let err = VajraError::DimensionMismatch {
            expected: 128,
            actual: 256,
        };
        let status = err.into_status();
        assert_eq!(status.code(), Code::InvalidArgument);
    }

    #[test]
    fn test_vector_not_found_is_not_found() {
        let err = VajraError::VectorNotFound { id: VectorId(42) };
        let status = err.into_status();
        assert_eq!(status.code(), Code::NotFound);
    }

    #[test]
    fn test_not_leader_is_unavailable() {
        let err = VajraError::NotLeader { leader: None };
        let status = err.into_status();
        assert_eq!(status.code(), Code::Unavailable);
    }

    #[test]
    fn test_capacity_exceeded_is_resource_exhausted() {
        let err = VajraError::CapacityExceeded {
            current: 1000,
            max: 1000,
        };
        let status = err.into_status();
        assert_eq!(status.code(), Code::ResourceExhausted);
    }

    #[test]
    fn test_timeout_is_deadline_exceeded() {
        let err = VajraError::Timeout {
            duration: Duration::from_secs(5),
        };
        let status = err.into_status();
        assert_eq!(status.code(), Code::DeadlineExceeded);
    }

    #[test]
    fn test_status_with_context() {
        let err = VajraError::VectorNotFound { id: VectorId(1) };
        let status = status_with_context(err, "during search");
        assert!(status.message().contains("during search"));
    }
}
