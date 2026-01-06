//! Type definitions for the Vajra engine.
//!
//! This module defines engine-specific types and re-exports common types.

use serde::{Deserialize, Serialize};

// Re-export common types for convenience
pub use vajra_common::types::{DistanceMetric, Metadata, SearchResult, VectorId};

/// A stored vector with its associated data.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VectorRecord {
    /// Unique identifier for this vector
    pub id: VectorId,
    /// The embedding vector data
    pub embedding: Vec<f32>,
    /// Optional metadata attached to the vector
    pub metadata: Option<Metadata>,
    /// The Raft log index at which this vector was inserted
    /// Used for consistency tracking
    pub created_at_log_index: u64,
}

impl VectorRecord {
    /// Create a new vector record.
    pub fn new(id: VectorId, embedding: Vec<f32>, log_index: u64) -> Self {
        Self {
            id,
            embedding,
            metadata: None,
            created_at_log_index: log_index,
        }
    }

    /// Create a new vector record with metadata.
    pub fn with_metadata(
        id: VectorId,
        embedding: Vec<f32>,
        metadata: Metadata,
        log_index: u64,
    ) -> Self {
        Self {
            id,
            embedding,
            metadata: Some(metadata),
            created_at_log_index: log_index,
        }
    }

    /// Get the dimension of the embedding.
    pub fn dimension(&self) -> usize {
        self.embedding.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_vector_record_creation() {
        let record = VectorRecord::new(VectorId(1), vec![1.0, 2.0, 3.0], 100);
        assert_eq!(record.id, VectorId(1));
        assert_eq!(record.dimension(), 3);
        assert_eq!(record.created_at_log_index, 100);
        assert!(record.metadata.is_none());
    }

    #[test]
    fn test_vector_record_with_metadata() {
        let mut metadata = Metadata::new();
        metadata.insert("key".to_string(), serde_json::json!("value"));

        let record =
            VectorRecord::with_metadata(VectorId(2), vec![1.0, 2.0], metadata.clone(), 200);

        assert_eq!(record.id, VectorId(2));
        assert!(record.metadata.is_some());
    }
}
