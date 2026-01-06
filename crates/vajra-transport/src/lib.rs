//! # Vajra Transport
//!
//! gRPC transport layer for Vajra client-server and inter-node communication.
//!
//! This crate defines the protocol buffer schemas and implements the
//! gRPC services for:
//!
//! - **Vector Operations**: Upsert, Search, Delete, Get
//! - **Raft RPCs**: RequestVote, AppendEntries, InstallSnapshot
//!
//! ## Services
//!
//! - `VectorService`: Client-facing API for vector operations
//! - `RaftService`: Internal Raft consensus protocol RPCs

#![warn(missing_docs)]
#![warn(clippy::all)]
#![warn(clippy::pedantic)]

// Modules will be implemented in Phase 3
// pub mod server;
// pub mod client;
// pub mod proto;

/// Placeholder for the gRPC server (to be implemented in Phase 3)
pub struct VajraServer {
    _private: (),
}

impl VajraServer {
    /// Create a new server (placeholder)
    #[must_use]
    pub fn new() -> Self {
        Self { _private: () }
    }
}

impl Default for VajraServer {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_placeholder() {
        let _server = VajraServer::new();
    }
}
