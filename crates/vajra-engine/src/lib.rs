//! # Vajra Engine
//!
//! The core vector indexing engine for Vajra, implementing the HNSW
//! (Hierarchical Navigable Small World) algorithm.
//!
//! This crate is designed to function as a standalone library (`libvajra`)
//! with zero network dependencies.
//!
//! ## Features
//!
//! - **HNSW Index**: Approximate nearest neighbor search with configurable recall
//! - **Multiple Distance Metrics**: Euclidean, Cosine, Inner Product
//! - **Concurrent Operations**: Lock-free reads, thread-safe writes
//! - **Soft Deletes**: Efficient deletion without graph restructuring
//! - **Deterministic**: Reproducible behavior for distributed consensus
//!
//! ## Example
//!
//! ```
//! use vajra_engine::{HnswIndex, distance::EuclideanDistance};
//! use vajra_common::{VectorId, config::HnswConfig};
//!
//! // Create an index
//! let config = HnswConfig::default();
//! let index = HnswIndex::new(config, 128, 10000, Box::new(EuclideanDistance));
//!
//! // Insert vectors
//! index.insert(VectorId(1), vec![0.1; 128]).unwrap();
//! index.insert(VectorId(2), vec![0.2; 128]).unwrap();
//!
//! // Search for nearest neighbors
//! let results = index.search(&[0.15; 128], 10, 50).unwrap();
//! ```

#![warn(missing_docs)]
#![warn(clippy::all)]

pub mod distance;
pub mod hnsw;
pub mod types;

// Re-export main types
pub use distance::{CosineDistance, DistanceFunction, EuclideanDistance, InnerProductDistance};
pub use hnsw::HnswIndex;
pub use types::VectorRecord;
