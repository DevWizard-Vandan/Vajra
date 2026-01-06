//! # Vajra Transport
//!
//! gRPC transport layer for Vajra vector database.
//!
//! ## Features
//!
//! - **Streaming Upsert**: Bulk operations without message size limits
//! - **Deterministic ID Mapping**: String IDs hashed to u64 with SipHash
//! - **gRPC Reflection**: Compatible with grpcurl for debugging
//! - **Error Mapping**: VajraError → proper gRPC status codes

#![warn(missing_docs)]
#![warn(clippy::all)]

pub mod error;
pub mod id_mapper;
pub mod service;

/// Generated protobuf types.
pub mod pb {
    tonic::include_proto!("vajra.v1");

    /// File descriptor set for gRPC reflection.
    pub const FILE_DESCRIPTOR_SET: &[u8] =
        tonic::include_file_descriptor_set!("vajra_descriptor");
}

// Re-export main types
pub use pb::vector_service_server::{VectorService, VectorServiceServer};
pub use service::VectorServiceImpl;
