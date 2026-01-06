//! # Vajra Common
//!
//! Shared types, error definitions, configuration, and observability utilities
//! for the Vajra distributed vector database.
//!
//! This crate provides the foundational building blocks used by all other Vajra crates:
//!
//! - [`error`] - Comprehensive error types covering all failure modes
//! - [`config`] - Configuration structures for all components
//! - [`telemetry`] - Observability stack (tracing, metrics, OpenTelemetry)
//! - [`types`] - Core type definitions (VectorId, NodeId, etc.)

#![warn(missing_docs)]
#![warn(clippy::all)]
#![warn(clippy::pedantic)]

pub mod config;
pub mod error;
pub mod telemetry;
pub mod types;

// Re-export commonly used types at the crate root
pub use config::VajraConfig;
pub use error::VajraError;
pub use types::{Metadata, NodeId, VectorId};

/// Result type alias using VajraError
pub type Result<T> = std::result::Result<T, VajraError>;
