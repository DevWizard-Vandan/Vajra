//! State machine wrapping the HNSW index.
//!
//! This module provides the state machine that applies committed
//! Raft log entries to the vector index.

use std::sync::Arc;
use tracing::{info, warn};
use vajra_common::types::DistanceMetric;
use vajra_common::{VajraError, VectorId};
use vajra_engine::{distance::create_distance_function, HnswIndex};
use vajra_wal::{Command, LogEntry};

/// State machine wrapping the HNSW index.
pub struct VajraStateMachine {
    /// The HNSW index.
    index: Arc<HnswIndex>,
    /// Last applied log index.
    last_applied: u64,
}

impl VajraStateMachine {
    /// Create a new state machine.
    pub fn new(
        dimension: usize,
        max_vectors: usize,
        config: vajra_common::config::HnswConfig,
    ) -> Self {
        let index = HnswIndex::new(
            config,
            dimension,
            max_vectors,
            create_distance_function(DistanceMetric::Euclidean),
        );

        Self {
            index: Arc::new(index),
            last_applied: 0,
        }
    }

    /// Get a reference to the index for read operations.
    pub fn index(&self) -> &Arc<HnswIndex> {
        &self.index
    }

    /// Get the last applied log index.
    pub fn last_applied(&self) -> u64 {
        self.last_applied
    }

    /// Apply a committed log entry to the state machine.
    #[tracing::instrument(skip(self, entry), fields(index = %entry.index))]
    pub fn apply(&mut self, entry: &LogEntry) -> Result<(), VajraError> {
        // Idempotency check
        if entry.index <= self.last_applied {
            return Ok(());
        }

        self.apply_command(&entry.command)?;
        self.last_applied = entry.index;

        Ok(())
    }

    /// Apply a single command.
    fn apply_command(&self, command: &Command) -> Result<(), VajraError> {
        match command {
            Command::Insert {
                id,
                vector,
                metadata: _,
            } => {
                let vector_id = VectorId(*id);

                // Collision warning (Senior Architect feedback)
                if self.index.get_vector(vector_id).is_some() {
                    warn!(
                        id = *id,
                        "SipHash collision or duplicate: overwriting existing vector"
                    );
                }

                self.index.insert(vector_id, vector.clone())?;
                info!(id = *id, "Applied Insert");
            }
            Command::Delete { id } => {
                let vector_id = VectorId(*id);
                match self.index.delete(vector_id) {
                    Ok(()) => info!(id = *id, "Applied Delete"),
                    Err(VajraError::VectorNotFound { .. }) => {
                        warn!(id = *id, "Delete: vector not found (already deleted?)");
                    }
                    Err(e) => return Err(e),
                }
            }
            Command::Batch(commands) => {
                for cmd in commands {
                    self.apply_command(cmd)?;
                }
            }
            Command::Noop => {
                // No-op entries are just for leader confirmation
            }
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use vajra_common::config::HnswConfig;

    #[test]
    fn test_state_machine_apply() {
        let mut sm = VajraStateMachine::new(4, 1000, HnswConfig::default());

        let entry = LogEntry::new(
            1,
            1,
            Command::Insert {
                id: 42,
                vector: vec![1.0, 2.0, 3.0, 4.0],
                metadata: None,
            },
        );

        sm.apply(&entry).unwrap();
        assert_eq!(sm.last_applied(), 1);

        // Verify vector was inserted
        let vector = sm.index().get_vector(VectorId(42));
        assert!(vector.is_some());
    }

    #[test]
    fn test_state_machine_idempotent() {
        let mut sm = VajraStateMachine::new(4, 1000, HnswConfig::default());

        let entry = LogEntry::new(1, 1, Command::Noop);

        sm.apply(&entry).unwrap();
        sm.apply(&entry).unwrap(); // Should be idempotent

        assert_eq!(sm.last_applied(), 1);
    }
}
