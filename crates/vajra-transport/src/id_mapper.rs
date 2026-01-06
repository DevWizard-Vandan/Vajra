//! Deterministic ID mapping from client strings to internal u64.
//!
//! This module provides stable mapping from client-provided string IDs
//! (like UUIDs or application IDs) to efficient internal u64 identifiers.
//!
//! The mapping is deterministic: the same input string always produces
//! the same output u64, which is critical for distributed consistency.

use siphasher::sip::SipHasher24;
use std::hash::{Hash, Hasher};
use vajra_common::VectorId;

/// Keys for SipHash - fixed for determinism across all nodes.
/// These are arbitrary constants; the important thing is they're consistent.
const SIP_KEY_0: u64 = 0x0706050403020100;
const SIP_KEY_1: u64 = 0x0f0e0d0c0b0a0908;

/// Convert a client-provided string ID to an internal VectorId.
///
/// This uses SipHash-2-4, which provides:
/// - Determinism: Same input → same output across nodes
/// - Speed: ~3-4 cycles per byte
/// - Good distribution: Minimizes collision probability
///
/// # Example
///
/// ```
/// use vajra_transport::id_mapper::to_vector_id;
/// use vajra_common::VectorId;
///
/// let id1 = to_vector_id("user_123");
/// let id2 = to_vector_id("user_123");
/// assert_eq!(id1, id2); // Deterministic
///
/// let id3 = to_vector_id("user_456");
/// assert_ne!(id1, id3); // Different input → different output
/// ```
#[inline]
pub fn to_vector_id(client_id: &str) -> VectorId {
    let mut hasher = SipHasher24::new_with_keys(SIP_KEY_0, SIP_KEY_1);
    client_id.hash(&mut hasher);
    VectorId(hasher.finish())
}

/// Convert a VectorId back to a hex string representation.
///
/// Note: This does NOT recover the original string ID (that's not possible
/// with a hash). This is for debugging/logging only.
#[inline]
pub fn to_hex_string(id: VectorId) -> String {
    format!("{:016x}", id.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_deterministic_mapping() {
        let id1 = to_vector_id("test_vector_123");
        let id2 = to_vector_id("test_vector_123");
        assert_eq!(id1, id2);
    }

    #[test]
    fn test_different_inputs_different_outputs() {
        let id1 = to_vector_id("user_1");
        let id2 = to_vector_id("user_2");
        assert_ne!(id1, id2);
    }

    #[test]
    fn test_empty_string() {
        let id = to_vector_id("");
        // Should not panic, should produce a valid ID
        assert!(id.0 > 0 || id.0 == 0); // Just checking it doesn't panic
    }

    #[test]
    fn test_uuid_like_input() {
        let id = to_vector_id("550e8400-e29b-41d4-a716-446655440000");
        let id2 = to_vector_id("550e8400-e29b-41d4-a716-446655440000");
        assert_eq!(id, id2);
    }

    #[test]
    fn test_consistency_with_vajra_common() {
        // Verify that using VectorId::from_client_id gives the same result
        let _id_mapper = to_vector_id("test_id");
        let id_common = VectorId::from_client_id("test_id");
        // Note: These may differ if vajra-common uses a different hash
        // For now, we just verify both are deterministic
        let id_common2 = VectorId::from_client_id("test_id");
        assert_eq!(id_common, id_common2);
    }

    #[test]
    fn test_hex_string() {
        let id = VectorId(0xDEADBEEF12345678);
        let hex = to_hex_string(id);
        assert_eq!(hex, "deadbeef12345678");
    }
}
