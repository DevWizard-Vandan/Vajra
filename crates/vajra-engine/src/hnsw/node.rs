//! HNSW graph node structure.

use smallvec::SmallVec;
use vajra_common::VectorId;

/// A node in the HNSW graph.
///
/// Each node corresponds to one vector and stores connections
/// to other nodes at each layer of the hierarchy.
#[derive(Debug, Clone)]
pub struct HnswNode {
    /// The vector ID this node represents
    pub vector_id: VectorId,
    /// The maximum layer this node exists in (0 = bottom only)
    pub max_layer: usize,
    /// Connections at each layer.
    /// `connections[i]` contains neighbors at layer i.
    /// Layer 0 can have up to M_max0 connections, others up to M.
    pub connections: Vec<SmallVec<[VectorId; 32]>>,
}

impl HnswNode {
    /// Create a new HNSW node.
    ///
    /// # Arguments
    /// * `vector_id` - The ID of the vector this node represents
    /// * `max_layer` - The highest layer this node will exist in
    pub fn new(vector_id: VectorId, max_layer: usize) -> Self {
        // Initialize empty connection lists for each layer
        let connections = (0..=max_layer)
            .map(|_| SmallVec::new())
            .collect();

        Self {
            vector_id,
            max_layer,
            connections,
        }
    }

    /// Get connections at a specific layer.
    #[inline]
    pub fn connections_at(&self, layer: usize) -> &[VectorId] {
        self.connections.get(layer).map_or(&[], |c| c.as_slice())
    }

    /// Get mutable connections at a specific layer.
    #[inline]
    pub fn connections_at_mut(&mut self, layer: usize) -> Option<&mut SmallVec<[VectorId; 32]>> {
        self.connections.get_mut(layer)
    }

    /// Add a connection at a specific layer.
    ///
    /// Returns true if the connection was added (not already present).
    pub fn add_connection(&mut self, layer: usize, neighbor: VectorId) -> bool {
        if let Some(conns) = self.connections.get_mut(layer) {
            if !conns.contains(&neighbor) {
                conns.push(neighbor);
                return true;
            }
        }
        false
    }

    /// Remove a connection at a specific layer.
    ///
    /// Returns true if the connection was removed.
    pub fn remove_connection(&mut self, layer: usize, neighbor: VectorId) -> bool {
        if let Some(conns) = self.connections.get_mut(layer) {
            if let Some(pos) = conns.iter().position(|&id| id == neighbor) {
                conns.swap_remove(pos);
                return true;
            }
        }
        false
    }

    /// Get the number of connections at a specific layer.
    #[inline]
    pub fn connection_count(&self, layer: usize) -> usize {
        self.connections.get(layer).map_or(0, |c| c.len())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_node_creation() {
        let node = HnswNode::new(VectorId(42), 3);
        assert_eq!(node.vector_id, VectorId(42));
        assert_eq!(node.max_layer, 3);
        assert_eq!(node.connections.len(), 4); // Layers 0, 1, 2, 3
    }

    #[test]
    fn test_add_connection() {
        let mut node = HnswNode::new(VectorId(1), 2);

        assert!(node.add_connection(0, VectorId(10)));
        assert!(node.add_connection(0, VectorId(11)));
        assert!(!node.add_connection(0, VectorId(10))); // Duplicate

        assert_eq!(node.connection_count(0), 2);
        assert_eq!(node.connections_at(0), &[VectorId(10), VectorId(11)]);
    }

    #[test]
    fn test_remove_connection() {
        let mut node = HnswNode::new(VectorId(1), 1);

        node.add_connection(0, VectorId(10));
        node.add_connection(0, VectorId(11));
        node.add_connection(0, VectorId(12));

        assert!(node.remove_connection(0, VectorId(11)));
        assert!(!node.remove_connection(0, VectorId(99))); // Not present

        assert_eq!(node.connection_count(0), 2);
    }

    #[test]
    fn test_connections_at_invalid_layer() {
        let node = HnswNode::new(VectorId(1), 1);
        assert!(node.connections_at(5).is_empty());
    }
}
