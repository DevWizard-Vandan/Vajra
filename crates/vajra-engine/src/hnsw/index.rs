//! HNSW index implementation.
//!
//! The main index structure providing insert, search, and delete operations.

use crate::distance::DistanceFunction;
use crate::hnsw::HnswNode;
use dashmap::{DashMap, DashSet};
use parking_lot::RwLock;
use rand::prelude::*;
use rand_chacha::ChaCha8Rng;
use std::cmp::Ordering;
use std::collections::BinaryHeap;
use std::sync::atomic::{AtomicUsize, Ordering as AtomicOrdering};
use std::sync::Arc;
use vajra_common::config::HnswConfig;
use vajra_common::types::SearchResult;
use vajra_common::{VajraError, VectorId};

/// The HNSW index for approximate nearest neighbor search.
pub struct HnswIndex {
    /// Configuration parameters
    config: HnswConfig,
    /// Vector dimension
    dimension: usize,
    /// Maximum number of vectors
    max_vectors: usize,
    /// Distance function
    distance_fn: Box<dyn DistanceFunction>,

    /// Vector storage (separate from graph for cache efficiency)
    vectors: DashMap<VectorId, Arc<[f32]>>,
    /// Graph structure
    nodes: DashMap<VectorId, HnswNode>,
    /// Entry point (node at the highest layer)
    entry_point: RwLock<Option<(VectorId, usize)>>, // (id, max_layer)
    /// Soft-deleted vectors
    deleted: DashSet<VectorId>,

    /// Statistics
    vector_count: AtomicUsize,
    deleted_count: AtomicUsize,
    max_layer_reached: AtomicUsize,
}

/// Candidate for search with distance ordering.
#[derive(Debug, Clone, Copy)]
struct Candidate {
    id: VectorId,
    distance: f32,
}

impl PartialEq for Candidate {
    fn eq(&self, other: &Self) -> bool {
        self.distance == other.distance
    }
}

impl Eq for Candidate {}

impl PartialOrd for Candidate {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for Candidate {
    fn cmp(&self, other: &Self) -> Ordering {
        // For min-heap: smaller distance = higher priority
        other
            .distance
            .partial_cmp(&self.distance)
            .unwrap_or(Ordering::Equal)
    }
}

/// Candidate ordered by distance for max-heap (furthest first).
#[derive(Debug, Clone, Copy)]
struct FurthestCandidate {
    id: VectorId,
    distance: f32,
}

impl PartialEq for FurthestCandidate {
    fn eq(&self, other: &Self) -> bool {
        self.distance == other.distance
    }
}

impl Eq for FurthestCandidate {}

impl PartialOrd for FurthestCandidate {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for FurthestCandidate {
    fn cmp(&self, other: &Self) -> Ordering {
        // For max-heap: larger distance = higher priority
        self.distance
            .partial_cmp(&other.distance)
            .unwrap_or(Ordering::Equal)
    }
}

impl HnswIndex {
    /// Create a new HNSW index.
    pub fn new(
        config: HnswConfig,
        dimension: usize,
        max_vectors: usize,
        distance_fn: Box<dyn DistanceFunction>,
    ) -> Self {
        Self {
            config,
            dimension,
            max_vectors,
            distance_fn,
            vectors: DashMap::new(),
            nodes: DashMap::new(),
            entry_point: RwLock::new(None),
            deleted: DashSet::new(),
            vector_count: AtomicUsize::new(0),
            deleted_count: AtomicUsize::new(0),
            max_layer_reached: AtomicUsize::new(0),
        }
    }

    /// Get the dimension of vectors in this index.
    #[inline]
    pub fn dimension(&self) -> usize {
        self.dimension
    }

    /// Get the current number of vectors (including soft-deleted).
    #[inline]
    pub fn len(&self) -> usize {
        self.vector_count.load(AtomicOrdering::Relaxed)
    }

    /// Check if the index is empty.
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Get the number of soft-deleted vectors.
    #[inline]
    pub fn deleted_count(&self) -> usize {
        self.deleted_count.load(AtomicOrdering::Relaxed)
    }

    /// Get the configuration.
    pub fn config(&self) -> &HnswConfig {
        &self.config
    }

    /// Select the layer for a new node deterministically based on vector ID.
    ///
    /// This uses the vector ID as a seed for reproducible layer assignment,
    /// which is critical for Raft state machine determinism.
    fn select_layer(&self, id: VectorId) -> usize {
        let mut rng = ChaCha8Rng::seed_from_u64(id.0);
        let uniform: f64 = rng.gen();

        // Formula: floor(-ln(uniform) * ml)
        // ml = 1 / ln(M)
        let layer = (-uniform.ln() * self.config.ml).floor() as usize;

        // Cap at a reasonable maximum to prevent pathological cases
        layer.min(16)
    }

    /// Get a vector by ID.
    pub fn get_vector(&self, id: VectorId) -> Option<Arc<[f32]>> {
        self.vectors.get(&id).map(|v| Arc::clone(&v))
    }

    /// Check if a vector is soft-deleted.
    #[inline]
    pub fn is_deleted(&self, id: VectorId) -> bool {
        self.deleted.contains(&id)
    }

    /// Insert a vector into the index.
    ///
    /// # Arguments
    /// * `id` - Unique identifier for the vector
    /// * `vector` - The embedding vector
    ///
    /// # Errors
    /// Returns an error if:
    /// - Vector dimension doesn't match index dimension
    /// - Index is at capacity
    /// - Vector ID already exists
    #[tracing::instrument(skip(self, vector), fields(vector_id = %id))]
    pub fn insert(&self, id: VectorId, vector: Vec<f32>) -> Result<(), VajraError> {
        // Validate dimension
        if vector.len() != self.dimension {
            return Err(VajraError::DimensionMismatch {
                expected: self.dimension,
                actual: vector.len(),
            });
        }

        // Check capacity
        if self.len() >= self.max_vectors {
            return Err(VajraError::CapacityExceeded {
                current: self.len(),
                max: self.max_vectors,
            });
        }

        // Check if already exists
        if self.vectors.contains_key(&id) {
            return Err(VajraError::VectorAlreadyExists { id });
        }

        // Select layer deterministically
        let new_layer = self.select_layer(id);

        // Store the vector
        let vector: Arc<[f32]> = vector.into();
        self.vectors.insert(id, Arc::clone(&vector));

        // Create the node
        let node = HnswNode::new(id, new_layer);
        self.nodes.insert(id, node);

        // Get current entry point
        let entry = { *self.entry_point.read() };

        if let Some((entry_id, entry_layer)) = entry {
            // Find the entry point at each layer and connect
            self.insert_into_graph(id, &vector, new_layer, entry_id, entry_layer)?;

            // Update entry point if new node is higher
            if new_layer > entry_layer {
                let mut ep = self.entry_point.write();
                *ep = Some((id, new_layer));
                self.max_layer_reached
                    .fetch_max(new_layer, AtomicOrdering::Relaxed);
            }
        } else {
            // First node becomes entry point
            let mut ep = self.entry_point.write();
            *ep = Some((id, new_layer));
            self.max_layer_reached
                .store(new_layer, AtomicOrdering::Relaxed);
        }

        self.vector_count.fetch_add(1, AtomicOrdering::Relaxed);

        Ok(())
    }

    /// Insert a node into the graph structure.
    fn insert_into_graph(
        &self,
        id: VectorId,
        vector: &[f32],
        new_layer: usize,
        entry_id: VectorId,
        entry_layer: usize,
    ) -> Result<(), VajraError> {
        let mut current_ep = entry_id;

        // Phase 1: Greedy descent from top to new_layer + 1
        for layer in (new_layer + 1..=entry_layer).rev() {
            current_ep = self.greedy_search_layer(vector, current_ep, layer);
        }

        // Phase 2: Insert at each layer from new_layer down to 0
        let search_layer = new_layer.min(entry_layer);
        for layer in (0..=search_layer).rev() {
            // Find ef_construction nearest neighbors at this layer
            let neighbors = self.search_layer(vector, current_ep, self.config.ef_construction, layer);

            // Select M best neighbors (M_max0 for layer 0)
            let max_connections = if layer == 0 {
                self.config.m_max0
            } else {
                self.config.m
            };

            let selected: Vec<VectorId> = neighbors
                .iter()
                .take(max_connections)
                .map(|c| c.id)
                .collect();

            // Add bidirectional connections
            if let Some(mut node) = self.nodes.get_mut(&id) {
                for &neighbor_id in &selected {
                    node.add_connection(layer, neighbor_id);
                }
            }

            // Add reverse connections and shrink if needed
            for neighbor_id in selected {
                if let Some(mut neighbor_node) = self.nodes.get_mut(&neighbor_id) {
                    neighbor_node.add_connection(layer, id);

                    // Shrink if exceeding max connections
                    if neighbor_node.connection_count(layer) > max_connections {
                        self.shrink_connections(&mut neighbor_node, layer, max_connections);
                    }
                }
            }

            // Update entry point for next layer
            if !neighbors.is_empty() {
                current_ep = neighbors[0].id;
            }
        }

        Ok(())
    }

    /// Greedy search to find the nearest node at a specific layer.
    fn greedy_search_layer(&self, query: &[f32], entry: VectorId, layer: usize) -> VectorId {
        let mut current = entry;
        let mut current_dist = self.compute_distance(query, current);

        loop {
            let mut changed = false;

            if let Some(node) = self.nodes.get(&current) {
                for &neighbor in node.connections_at(layer) {
                    let dist = self.compute_distance(query, neighbor);
                    if dist < current_dist {
                        current = neighbor;
                        current_dist = dist;
                        changed = true;
                    }
                }
            }

            if !changed {
                break;
            }
        }

        current
    }

    /// Search for nearest neighbors at a specific layer.
    fn search_layer(
        &self,
        query: &[f32],
        entry: VectorId,
        ef: usize,
        layer: usize,
    ) -> Vec<Candidate> {
        let visited = DashSet::new();
        let mut candidates: BinaryHeap<Candidate> = BinaryHeap::new(); // Min-heap
        let mut results: BinaryHeap<FurthestCandidate> = BinaryHeap::new(); // Max-heap

        let entry_dist = self.compute_distance(query, entry);
        visited.insert(entry);
        candidates.push(Candidate {
            id: entry,
            distance: entry_dist,
        });
        results.push(FurthestCandidate {
            id: entry,
            distance: entry_dist,
        });

        while let Some(current) = candidates.pop() {
            // Check stopping condition
            if let Some(furthest) = results.peek() {
                if current.distance > furthest.distance {
                    break;
                }
            }

            // Expand neighbors
            if let Some(node) = self.nodes.get(&current.id) {
                for &neighbor in node.connections_at(layer) {
                    if visited.contains(&neighbor) {
                        continue;
                    }
                    visited.insert(neighbor);

                    let dist = self.compute_distance(query, neighbor);

                    // Add to results if better than worst result or results not full
                    let should_add = results.len() < ef
                        || results.peek().map_or(true, |f| dist < f.distance);

                    if should_add {
                        candidates.push(Candidate {
                            id: neighbor,
                            distance: dist,
                        });
                        results.push(FurthestCandidate {
                            id: neighbor,
                            distance: dist,
                        });

                        // Keep only top ef results
                        while results.len() > ef {
                            results.pop();
                        }
                    }
                }
            }
        }

        // Convert to sorted vec
        let mut result_vec: Vec<Candidate> = results
            .into_iter()
            .map(|f| Candidate {
                id: f.id,
                distance: f.distance,
            })
            .collect();
        result_vec.sort_by(|a, b| a.distance.partial_cmp(&b.distance).unwrap_or(Ordering::Equal));
        result_vec
    }

    /// Shrink connections to keep only the best ones.
    fn shrink_connections(&self, node: &mut HnswNode, layer: usize, max_connections: usize) {
        // Get the vector ID before borrowing connections
        let node_id = node.vector_id;

        // Get the node's vector for distance calculations
        let node_vector = match self.get_vector(node_id) {
            Some(v) => v,
            None => return,
        };

        // Get current connections
        let current_conns: Vec<VectorId> = node
            .connections_at(layer)
            .to_vec();

        if current_conns.len() <= max_connections {
            return;
        }

        // Calculate distances and sort
        let mut with_distances: Vec<(VectorId, f32)> = current_conns
            .iter()
            .map(|&neighbor| {
                let dist = self.compute_distance(&node_vector, neighbor);
                (neighbor, dist)
            })
            .collect();

        with_distances.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(Ordering::Equal));

        // Now update the connections
        if let Some(conns) = node.connections_at_mut(layer) {
            conns.clear();
            for (neighbor, _) in with_distances.into_iter().take(max_connections) {
                conns.push(neighbor);
            }
        }
    }

    /// Compute distance between a query vector and a stored vector.
    #[inline]
    fn compute_distance(&self, query: &[f32], id: VectorId) -> f32 {
        if let Some(stored) = self.vectors.get(&id) {
            self.distance_fn.distance(query, &stored)
        } else {
            f32::MAX
        }
    }

    /// Search for k nearest neighbors.
    ///
    /// # Arguments
    /// * `query` - The query vector
    /// * `k` - Number of results to return
    /// * `ef` - Search width (higher = more accurate but slower)
    ///
    /// # Errors
    /// Returns an error if:
    /// - Query dimension doesn't match index dimension
    /// - Index is empty
    #[tracing::instrument(skip(self, query))]
    pub fn search(&self, query: &[f32], k: usize, ef: usize) -> Result<Vec<SearchResult>, VajraError> {
        // Validate dimension
        if query.len() != self.dimension {
            return Err(VajraError::DimensionMismatch {
                expected: self.dimension,
                actual: query.len(),
            });
        }

        // Get entry point
        let (entry_id, entry_layer) = {
            let ep = self.entry_point.read();
            match *ep {
                Some(e) => e,
                None => return Err(VajraError::EmptyIndex),
            }
        };

        // Phase 1: Greedy descent from top layer to layer 1
        let mut current_ep = entry_id;
        for layer in (1..=entry_layer).rev() {
            current_ep = self.greedy_search_layer(query, current_ep, layer);
        }

        // Phase 2: Search at layer 0 with ef candidates
        let ef_search = ef.max(k); // ef must be at least k
        let candidates = self.search_layer(query, current_ep, ef_search, 0);

        // Filter out deleted and return top k
        let results: Vec<SearchResult> = candidates
            .into_iter()
            .filter(|c| !self.is_deleted(c.id))
            .take(k)
            .map(|c| SearchResult::new(c.id, c.distance))
            .collect();

        Ok(results)
    }

    /// Soft-delete a vector from the index.
    ///
    /// The vector remains in the graph but is excluded from search results.
    /// This is intentionally non-destructive for Raft determinism.
    ///
    /// # Errors
    /// Returns an error if the vector doesn't exist.
    #[tracing::instrument(skip(self), fields(vector_id = %id))]
    pub fn delete(&self, id: VectorId) -> Result<(), VajraError> {
        if !self.vectors.contains_key(&id) {
            return Err(VajraError::VectorNotFound { id });
        }

        if self.deleted.insert(id) {
            self.deleted_count.fetch_add(1, AtomicOrdering::Relaxed);
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::distance::EuclideanDistance;

    fn create_test_index() -> HnswIndex {
        HnswIndex::new(
            HnswConfig::default(),
            4, // dimension
            10000,
            Box::new(EuclideanDistance),
        )
    }

    #[test]
    fn test_layer_selection_deterministic() {
        let index = create_test_index();

        let layer1 = index.select_layer(VectorId(12345));
        let layer2 = index.select_layer(VectorId(12345));

        assert_eq!(layer1, layer2, "Same VectorId must produce same layer");
    }

    #[test]
    fn test_layer_selection_varies() {
        let index = create_test_index();

        let mut layers = Vec::new();
        for i in 0..100 {
            layers.push(index.select_layer(VectorId(i)));
        }

        // Most should be layer 0, some higher
        let layer0_count = layers.iter().filter(|&&l| l == 0).count();
        assert!(layer0_count > 50, "Most vectors should be at layer 0");
        assert!(
            layers.iter().any(|&l| l > 0),
            "Some vectors should be at higher layers"
        );
    }

    #[test]
    fn test_insert_single_vector() {
        let index = create_test_index();

        let result = index.insert(VectorId(1), vec![1.0, 2.0, 3.0, 4.0]);
        assert!(result.is_ok());
        assert_eq!(index.len(), 1);
    }

    #[test]
    fn test_insert_dimension_mismatch() {
        let index = create_test_index();

        let result = index.insert(VectorId(1), vec![1.0, 2.0]); // Wrong dimension
        assert!(matches!(result, Err(VajraError::DimensionMismatch { .. })));
    }

    #[test]
    fn test_insert_duplicate() {
        let index = create_test_index();

        index.insert(VectorId(1), vec![1.0, 2.0, 3.0, 4.0]).unwrap();
        let result = index.insert(VectorId(1), vec![5.0, 6.0, 7.0, 8.0]);

        assert!(matches!(result, Err(VajraError::VectorAlreadyExists { .. })));
    }

    #[test]
    fn test_search_empty_index() {
        let index = create_test_index();

        let result = index.search(&[1.0, 2.0, 3.0, 4.0], 10, 50);
        assert!(matches!(result, Err(VajraError::EmptyIndex)));
    }

    #[test]
    fn test_search_dimension_mismatch() {
        let index = create_test_index();
        index.insert(VectorId(1), vec![1.0, 2.0, 3.0, 4.0]).unwrap();

        let result = index.search(&[1.0, 2.0], 10, 50); // Wrong dimension
        assert!(matches!(result, Err(VajraError::DimensionMismatch { .. })));
    }

    #[test]
    fn test_search_self() {
        let index = create_test_index();

        let vector = vec![1.0, 2.0, 3.0, 4.0];
        index.insert(VectorId(1), vector.clone()).unwrap();

        let results = index.search(&vector, 1, 50).unwrap();

        assert_eq!(results.len(), 1);
        assert_eq!(results[0].id, VectorId(1));
        assert!(results[0].score < 0.001); // Should be ~0
    }

    #[test]
    fn test_search_multiple_vectors() {
        let index = create_test_index();

        // Insert several vectors
        for i in 0..10 {
            let vector = vec![i as f32, 0.0, 0.0, 0.0];
            index.insert(VectorId(i), vector).unwrap();
        }

        // Search for vector closest to [5, 0, 0, 0]
        let query = vec![5.0, 0.0, 0.0, 0.0];
        let results = index.search(&query, 3, 50).unwrap();

        assert_eq!(results.len(), 3);
        assert_eq!(results[0].id, VectorId(5)); // Exact match
    }

    #[test]
    fn test_delete_vector() {
        let index = create_test_index();

        let vector = vec![1.0, 2.0, 3.0, 4.0];
        index.insert(VectorId(1), vector.clone()).unwrap();
        index.insert(VectorId(2), vec![5.0, 6.0, 7.0, 8.0]).unwrap();

        // Delete vector 1
        index.delete(VectorId(1)).unwrap();

        assert_eq!(index.deleted_count(), 1);
        assert!(index.is_deleted(VectorId(1)));

        // Search should not return deleted vector
        let results = index.search(&vector, 10, 50).unwrap();
        assert!(results.iter().all(|r| r.id != VectorId(1)));
    }

    #[test]
    fn test_delete_nonexistent() {
        let index = create_test_index();

        let result = index.delete(VectorId(999));
        assert!(matches!(result, Err(VajraError::VectorNotFound { .. })));
    }

    #[test]
    fn test_recall_at_1_self_search() {
        let index = HnswIndex::new(
            HnswConfig::default(),
            128,
            10000,
            Box::new(EuclideanDistance),
        );

        // Use deterministic RNG for reproducibility
        let mut rng = ChaCha8Rng::seed_from_u64(42);

        // Insert 1000 random vectors
        for i in 0..1000 {
            let vector: Vec<f32> = (0..128).map(|_| rng.gen()).collect();
            index.insert(VectorId(i), vector).unwrap();
        }

        // Search for each vector - it should be its own nearest neighbor
        let mut recall_hits = 0;
        for i in 0..1000 {
            let vector = index.get_vector(VectorId(i)).unwrap();
            let results = index.search(&vector, 1, 50).unwrap();

            if !results.is_empty() && results[0].id == VectorId(i) {
                recall_hits += 1;
            }
        }

        let recall = recall_hits as f64 / 1000.0;
        assert!(
            recall >= 0.99,
            "Recall@1 should be >= 99%, got {:.2}%",
            recall * 100.0
        );
    }
}
