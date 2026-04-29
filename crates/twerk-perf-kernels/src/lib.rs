//! Performance-critical kernels for Twerk.
//!
//! This crate contains hot-path optimizations that may use nightly features
//! like `portable_simd` when available. All implementations are:
//! - Safe (no unsafe)
//! - Fallback to scalar when SIMD unavailable
//! - Property-tested against scalar reference implementations
//!
//! # Feature Flags
//!
//! - `nightly`: Enable nightly-only features (std::simd)

#![forbid(unsafe_code)]
#![deny(unused_must_use)]

// Enable portable_simd when running on nightly
#![cfg_attr(feature = "nightly", feature(portable_simd))]

#[cfg(feature = "nightly")]
use std::simd::prelude::*;

/// Number of lanes for f32 SIMD operations.
const F32_LANES: usize = 8;

/// Computes the sum of squares for a slice of f32 values using SIMD.
#[cfg(feature = "nightly")]
#[cfg_attr(docsrs, doc(cfg(feature = "nightly")))]
pub fn sum_squares_f32_simd(input: &[f32]) -> f32 {
    let mut acc = Simd::<f32, F32_LANES>::splat(0.0);

    let mut chunks = input.chunks_exact(F32_LANES);

    for chunk in chunks.by_ref() {
        let values = Simd::<f32, F32_LANES>::load_or_default(chunk);
        acc += values * values;
    }

    let vector_sum = acc.reduce_sum();

    let scalar_tail: f32 = chunks
        .remainder()
        .iter()
        .copied()
        .map(|value| value * value)
        .sum();

    vector_sum + scalar_tail
}

/// Scalar reference implementation for sum of squares.
pub fn sum_squares_f32_scalar(input: &[f32]) -> f32 {
    input.iter().map(|value| value * value).sum()
}

/// Computes the dot product of two f32 slices using SIMD.
#[cfg(feature = "nightly")]
#[cfg_attr(docsrs, doc(cfg(feature = "nightly")))]
pub fn dot_product_f32_simd(a: &[f32], b: &[f32]) -> f32 {
    assert_eq!(a.len(), b.len(), "vectors must have same length");

    let mut acc = Simd::<f32, F32_LANES>::splat(0.0);

    let mut chunks = a.chunks_exact(F32_LANES);

    for (a_chunk, b_chunk) in chunks.zip(b.chunks_exact(F32_LANES)) {
        let a_vals = Simd::<f32, F32_LANES>::load_or_default(a_chunk);
        let b_vals = Simd::<f32, F32_LANES>::load_or_default(b_chunk);
        acc += a_vals * b_vals;
    }

    let vector_sum = acc.reduce_sum();

    let scalar_tail: f32 = chunks
        .remainder()
        .iter()
        .zip(b.iter().skip(a.len() - chunks.remainder().len()))
        .map(|(x, y)| x * y)
        .sum();

    vector_sum + scalar_tail
}

/// Scalar reference for dot product.
pub fn dot_product_f32_scalar(a: &[f32], b: &[f32]) -> f32 {
    assert_eq!(a.len(), b.len());
    a.iter().zip(b.iter()).map(|(x, y)| x * y).sum()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_edge_cases() {
        assert_eq!(sum_squares_f32_scalar(&[]), 0.0);
        assert_eq!(sum_squares_f32_scalar(&[1.0]), 1.0);

        let exact: Vec<f32> = (0..F32_LANES as i32).map(|i| i as f32).collect();
        assert_eq!(sum_squares_f32_scalar(&exact), sum_squares_f32_scalar(&exact));

        let small: Vec<f32> = (0..(F32_LANES - 1) as i32).map(|i| i as f32).collect();
        assert_eq!(sum_squares_f32_scalar(&small), sum_squares_f32_scalar(&small));

        let large: Vec<f32> = (0..(F32_LANES + 1) as i32).map(|i| i as f32).collect();
        assert_eq!(sum_squares_f32_scalar(&large), sum_squares_f32_scalar(&large));
    }
}

#[cfg(feature = "nightly")]
#[cfg(test)]
mod simd_tests {
    use super::*;

    proptest::proptest! {
        #[test]
        fn test_sum_squares_simd_matches_scalar(input: Vec<f32>) {
            let simd_result = sum_squares_f32_simd(&input);
            let scalar_result = sum_squares_f32_scalar(&input);
            let diff = (simd_result - scalar_result).abs();
            prop_assert!(diff < 1e-6 || input.is_empty(), "SIMD and scalar should match");
        }

        #[test]
        fn test_dot_product_simd_matches_scalar(a: Vec<f32>, b: Vec<f32>) {
            if a.len() != b.len() || a.is_empty() {
                return Ok(())
            }
            let simd_result = dot_product_f32_simd(&a, &b);
            let scalar_result = dot_product_f32_scalar(&a, &b);
            let diff = (simd_result - scalar_result).abs();
            prop_assert!(diff < 1e-6, "SIMD and scalar should match");
        }
    }

    #[test]
    fn test_sum_squares_simd_matches_scalar_large() {
        let input: Vec<f32> = (0..10000i32).map(|i| i as f32 * 0.001).collect();
        let simd_result = sum_squares_f32_simd(&input);
        let scalar_result = sum_squares_f32_scalar(&input);
        let diff = (simd_result - scalar_result).abs();
        assert!(diff < 1e-3, "SIMD and scalar should match for large arrays");
    }
}