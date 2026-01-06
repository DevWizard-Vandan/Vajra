//! Distance metrics for vector similarity calculations.
//!
//! This module provides distance functions used by the HNSW index.
//! All metrics return lower values for more similar vectors.

/// Trait for computing distance between vectors.
pub trait DistanceFunction: Send + Sync {
    /// Compute distance between two vectors.
    /// Lower values indicate more similar vectors.
    fn distance(&self, a: &[f32], b: &[f32]) -> f32;

    /// Name of the distance function for metrics/logging.
    fn name(&self) -> &'static str;
}

/// Euclidean (L2) distance.
///
/// Formula: sqrt(sum((a[i] - b[i])^2))
///
/// Range: [0, ∞) where 0 means identical vectors.
#[derive(Debug, Clone, Copy, Default)]
pub struct EuclideanDistance;

impl DistanceFunction for EuclideanDistance {
    #[inline]
    fn distance(&self, a: &[f32], b: &[f32]) -> f32 {
        debug_assert_eq!(a.len(), b.len(), "Vector dimensions must match");
        euclidean_unrolled(a, b)
    }

    fn name(&self) -> &'static str {
        "euclidean"
    }
}

/// Unrolled Euclidean distance for better auto-vectorization.
#[inline]
fn euclidean_unrolled(a: &[f32], b: &[f32]) -> f32 {
    let chunks = a.len() / 4;
    let mut sum0 = 0.0f32;
    let mut sum1 = 0.0f32;
    let mut sum2 = 0.0f32;
    let mut sum3 = 0.0f32;

    for i in 0..chunks {
        let offset = i * 4;
        let d0 = a[offset] - b[offset];
        let d1 = a[offset + 1] - b[offset + 1];
        let d2 = a[offset + 2] - b[offset + 2];
        let d3 = a[offset + 3] - b[offset + 3];

        sum0 += d0 * d0;
        sum1 += d1 * d1;
        sum2 += d2 * d2;
        sum3 += d3 * d3;
    }

    let mut total = sum0 + sum1 + sum2 + sum3;

    // Handle remainder
    for i in (chunks * 4)..a.len() {
        let diff = a[i] - b[i];
        total += diff * diff;
    }

    total.sqrt()
}

/// Cosine distance.
///
/// Formula: 1 - (a · b) / (||a|| * ||b||)
///
/// Range: [0, 2] where 0 means identical direction, 2 means opposite.
#[derive(Debug, Clone, Copy, Default)]
pub struct CosineDistance;

impl DistanceFunction for CosineDistance {
    #[inline]
    fn distance(&self, a: &[f32], b: &[f32]) -> f32 {
        debug_assert_eq!(a.len(), b.len(), "Vector dimensions must match");
        cosine_unrolled(a, b)
    }

    fn name(&self) -> &'static str {
        "cosine"
    }
}

/// Unrolled cosine distance for better auto-vectorization.
#[inline]
fn cosine_unrolled(a: &[f32], b: &[f32]) -> f32 {
    let chunks = a.len() / 4;

    let mut dot0 = 0.0f32;
    let mut dot1 = 0.0f32;
    let mut dot2 = 0.0f32;
    let mut dot3 = 0.0f32;

    let mut na0 = 0.0f32;
    let mut na1 = 0.0f32;
    let mut na2 = 0.0f32;
    let mut na3 = 0.0f32;

    let mut nb0 = 0.0f32;
    let mut nb1 = 0.0f32;
    let mut nb2 = 0.0f32;
    let mut nb3 = 0.0f32;

    for i in 0..chunks {
        let offset = i * 4;
        let a0 = a[offset];
        let a1 = a[offset + 1];
        let a2 = a[offset + 2];
        let a3 = a[offset + 3];
        let b0 = b[offset];
        let b1 = b[offset + 1];
        let b2 = b[offset + 2];
        let b3 = b[offset + 3];

        dot0 += a0 * b0;
        dot1 += a1 * b1;
        dot2 += a2 * b2;
        dot3 += a3 * b3;

        na0 += a0 * a0;
        na1 += a1 * a1;
        na2 += a2 * a2;
        na3 += a3 * a3;

        nb0 += b0 * b0;
        nb1 += b1 * b1;
        nb2 += b2 * b2;
        nb3 += b3 * b3;
    }

    let mut dot = dot0 + dot1 + dot2 + dot3;
    let mut norm_a = na0 + na1 + na2 + na3;
    let mut norm_b = nb0 + nb1 + nb2 + nb3;

    // Handle remainder
    for i in (chunks * 4)..a.len() {
        dot += a[i] * b[i];
        norm_a += a[i] * a[i];
        norm_b += b[i] * b[i];
    }

    let denom = (norm_a * norm_b).sqrt();
    if denom < f32::EPSILON {
        return 1.0; // Avoid division by zero
    }

    1.0 - (dot / denom)
}

/// Inner Product (Dot Product) distance.
///
/// Formula: -1 * (a · b)
///
/// For normalized vectors, this is equivalent to cosine distance.
/// The negation ensures that higher similarity = lower distance.
///
/// Range: (-∞, ∞) for unnormalized, [-1, 1] negated for normalized.
#[derive(Debug, Clone, Copy, Default)]
pub struct InnerProductDistance;

impl DistanceFunction for InnerProductDistance {
    #[inline]
    fn distance(&self, a: &[f32], b: &[f32]) -> f32 {
        debug_assert_eq!(a.len(), b.len(), "Vector dimensions must match");
        -inner_product_unrolled(a, b)
    }

    fn name(&self) -> &'static str {
        "inner_product"
    }
}

/// Unrolled inner product for better auto-vectorization.
#[inline]
fn inner_product_unrolled(a: &[f32], b: &[f32]) -> f32 {
    let chunks = a.len() / 4;
    let mut sum0 = 0.0f32;
    let mut sum1 = 0.0f32;
    let mut sum2 = 0.0f32;
    let mut sum3 = 0.0f32;

    for i in 0..chunks {
        let offset = i * 4;
        sum0 += a[offset] * b[offset];
        sum1 += a[offset + 1] * b[offset + 1];
        sum2 += a[offset + 2] * b[offset + 2];
        sum3 += a[offset + 3] * b[offset + 3];
    }

    let mut total = sum0 + sum1 + sum2 + sum3;

    // Handle remainder
    for i in (chunks * 4)..a.len() {
        total += a[i] * b[i];
    }

    total
}

/// Create a distance function from the metric enum.
pub fn create_distance_function(
    metric: vajra_common::types::DistanceMetric,
) -> Box<dyn DistanceFunction> {
    use vajra_common::types::DistanceMetric;

    match metric {
        DistanceMetric::Euclidean => Box::new(EuclideanDistance),
        DistanceMetric::Cosine => Box::new(CosineDistance),
        DistanceMetric::InnerProduct => Box::new(InnerProductDistance),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const EPSILON: f32 = 1e-5;

    fn approx_eq(a: f32, b: f32) -> bool {
        (a - b).abs() < EPSILON
    }

    #[test]
    fn test_euclidean_identical_vectors() {
        let dist = EuclideanDistance;
        let v = vec![1.0, 2.0, 3.0, 4.0];
        assert!(approx_eq(dist.distance(&v, &v), 0.0));
    }

    #[test]
    fn test_euclidean_known_distance() {
        let dist = EuclideanDistance;
        let a = vec![0.0, 0.0, 0.0];
        let b = vec![3.0, 4.0, 0.0];
        // sqrt(9 + 16) = 5
        assert!(approx_eq(dist.distance(&a, &b), 5.0));
    }

    #[test]
    fn test_euclidean_large_vector() {
        let dist = EuclideanDistance;
        // Vector with 16 elements to test unrolled path
        let a: Vec<f32> = (0..16).map(|i| i as f32).collect();
        let b: Vec<f32> = (0..16).map(|i| (i + 1) as f32).collect();
        // Each diff is 1, so sqrt(16 * 1) = 4
        assert!(approx_eq(dist.distance(&a, &b), 4.0));
    }

    #[test]
    fn test_cosine_identical_vectors() {
        let dist = CosineDistance;
        let v = vec![1.0, 2.0, 3.0, 4.0];
        assert!(approx_eq(dist.distance(&v, &v), 0.0));
    }

    #[test]
    fn test_cosine_orthogonal_vectors() {
        let dist = CosineDistance;
        let a = vec![1.0, 0.0, 0.0, 0.0];
        let b = vec![0.0, 1.0, 0.0, 0.0];
        // Orthogonal = cosine similarity 0, distance 1
        assert!(approx_eq(dist.distance(&a, &b), 1.0));
    }

    #[test]
    fn test_cosine_opposite_vectors() {
        let dist = CosineDistance;
        let a = vec![1.0, 0.0, 0.0, 0.0];
        let b = vec![-1.0, 0.0, 0.0, 0.0];
        // Opposite = cosine similarity -1, distance 2
        assert!(approx_eq(dist.distance(&a, &b), 2.0));
    }

    #[test]
    fn test_cosine_large_vector() {
        let dist = CosineDistance;
        let v: Vec<f32> = (1..17).map(|i| i as f32).collect();
        assert!(approx_eq(dist.distance(&v, &v), 0.0));
    }

    #[test]
    fn test_inner_product_identical_normalized() {
        let dist = InnerProductDistance;
        // Normalized vector
        let v = vec![0.6, 0.8, 0.0, 0.0];
        // dot(v, v) = 0.36 + 0.64 = 1.0, negated = -1.0
        assert!(approx_eq(dist.distance(&v, &v), -1.0));
    }

    #[test]
    fn test_inner_product_orthogonal() {
        let dist = InnerProductDistance;
        let a = vec![1.0, 0.0, 0.0, 0.0];
        let b = vec![0.0, 1.0, 0.0, 0.0];
        // dot = 0, negated = 0
        assert!(approx_eq(dist.distance(&a, &b), 0.0));
    }

    #[test]
    fn test_inner_product_large_vector() {
        let dist = InnerProductDistance;
        let a: Vec<f32> = vec![1.0; 16];
        let b: Vec<f32> = vec![2.0; 16];
        // dot = 32, negated = -32
        assert!(approx_eq(dist.distance(&a, &b), -32.0));
    }

    #[test]
    fn test_create_distance_function() {
        use vajra_common::types::DistanceMetric;

        let euclidean = create_distance_function(DistanceMetric::Euclidean);
        assert_eq!(euclidean.name(), "euclidean");

        let cosine = create_distance_function(DistanceMetric::Cosine);
        assert_eq!(cosine.name(), "cosine");

        let ip = create_distance_function(DistanceMetric::InnerProduct);
        assert_eq!(ip.name(), "inner_product");
    }
}
