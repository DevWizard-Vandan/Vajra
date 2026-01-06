//! Core type definitions for Vajra.
//!
//! This module defines the fundamental types used throughout the system:
//! - [`VectorId`] - Unique identifier for vectors in the index
//! - [`NodeId`] - Unique identifier for cluster nodes
//! - [`Metadata`] - JSON metadata attached to vectors

use serde::{Deserialize, Serialize};
use std::fmt;

/// Unique identifier for a vector in the index.
///
/// Vector IDs are assigned by the Raft leader to ensure deterministic
/// ordering across all replicas. The ID is derived from the log index
/// at which the insert command was committed.
///
/// # Example
/// ```
/// use vajra_common::VectorId;
///
/// let id = VectorId(12345);
/// assert_eq!(id.0, 12345);
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, PartialOrd, Ord)]
pub struct VectorId(pub u64);

impl VectorId {
    /// Create a new VectorId from a u64 value.
    #[must_use]
    pub const fn new(id: u64) -> Self {
        Self(id)
    }

    /// Get the underlying u64 value.
    #[must_use]
    pub const fn get(self) -> u64 {
        self.0
    }

    /// Create a VectorId from a client-provided string ID.
    ///
    /// This uses a hash of the client ID to generate a deterministic internal ID.
    /// Note: In production, the actual ID assignment happens through Raft to ensure
    /// consistency across replicas.
    #[must_use]
    pub fn from_client_id(client_id: &str) -> Self {
        use std::hash::{Hash, Hasher};
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        client_id.hash(&mut hasher);
        Self(hasher.finish())
    }
}

impl fmt::Display for VectorId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "v{}", self.0)
    }
}

impl From<u64> for VectorId {
    fn from(id: u64) -> Self {
        Self(id)
    }
}

impl From<VectorId> for u64 {
    fn from(id: VectorId) -> Self {
        id.0
    }
}

/// Unique identifier for a node in the cluster.
///
/// Node IDs must be unique within a cluster and are typically assigned
/// during initial cluster configuration.
///
/// # Example
/// ```
/// use vajra_common::NodeId;
///
/// let node = NodeId(1);
/// assert_eq!(node.0, 1);
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, PartialOrd, Ord)]
pub struct NodeId(pub u64);

impl NodeId {
    /// Create a new NodeId from a u64 value.
    #[must_use]
    pub const fn new(id: u64) -> Self {
        Self(id)
    }

    /// Get the underlying u64 value.
    #[must_use]
    pub const fn get(self) -> u64 {
        self.0
    }
}

impl fmt::Display for NodeId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "n{}", self.0)
    }
}

impl From<u64> for NodeId {
    fn from(id: u64) -> Self {
        Self(id)
    }
}

impl From<NodeId> for u64 {
    fn from(id: NodeId) -> Self {
        id.0
    }
}

impl From<String> for NodeId {
    fn from(s: String) -> Self {
        Self(s.parse().unwrap_or(0))
    }
}

impl From<&str> for NodeId {
    fn from(s: &str) -> Self {
        Self(s.parse().unwrap_or(0))
    }
}

/// Metadata attached to vectors.
///
/// Metadata is stored as a JSON object and can be used for filtering
/// during search operations.
///
/// # Example
/// ```
/// use vajra_common::Metadata;
///
/// let mut metadata = Metadata::new();
/// metadata.insert("category".to_string(), serde_json::json!("document"));
/// metadata.insert("author".to_string(), serde_json::json!("Alice"));
/// ```
pub type Metadata = serde_json::Map<String, serde_json::Value>;

/// Distance metric for vector similarity calculations.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum DistanceMetric {
    /// Euclidean (L2) distance - lower is more similar
    #[default]
    Euclidean,
    /// Cosine distance - 1 - cosine_similarity, lower is more similar
    Cosine,
    /// Inner product (dot product) - for normalized vectors
    InnerProduct,
}

impl fmt::Display for DistanceMetric {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Euclidean => write!(f, "euclidean"),
            Self::Cosine => write!(f, "cosine"),
            Self::InnerProduct => write!(f, "inner_product"),
        }
    }
}

/// Result of a vector search operation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchResult {
    /// The ID of the matching vector
    pub id: VectorId,
    /// The distance/score (lower is more similar for distance metrics)
    pub score: f32,
    /// Optional metadata attached to the vector
    pub metadata: Option<Metadata>,
}

impl SearchResult {
    /// Create a new search result.
    #[must_use]
    pub const fn new(id: VectorId, score: f32) -> Self {
        Self {
            id,
            score,
            metadata: None,
        }
    }

    /// Create a new search result with metadata.
    #[must_use]
    pub fn with_metadata(id: VectorId, score: f32, metadata: Metadata) -> Self {
        Self {
            id,
            score,
            metadata: Some(metadata),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_vector_id_display() {
        let id = VectorId(12345);
        assert_eq!(format!("{id}"), "v12345");
    }

    #[test]
    fn test_vector_id_from_client_id_deterministic() {
        let id1 = VectorId::from_client_id("test-vector-1");
        let id2 = VectorId::from_client_id("test-vector-1");
        assert_eq!(id1, id2);
    }

    #[test]
    fn test_node_id_display() {
        let id = NodeId(1);
        assert_eq!(format!("{id}"), "n1");
    }

    #[test]
    fn test_distance_metric_default() {
        let metric = DistanceMetric::default();
        assert_eq!(metric, DistanceMetric::Euclidean);
    }

    #[test]
    fn test_search_result_creation() {
        let result = SearchResult::new(VectorId(1), 0.5);
        assert_eq!(result.id, VectorId(1));
        assert!((result.score - 0.5).abs() < f32::EPSILON);
        assert!(result.metadata.is_none());
    }
}
