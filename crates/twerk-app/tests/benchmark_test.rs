//! Throughput and latency benchmarks
//!
//! Run with:
//!   cargo test -p twerk-app --test benchmark_test -- --nocapture
//!   cargo test -p twerk-web --test benchmark_test -- --nocapture

#![allow(clippy::unwrap_used)]
#![allow(clippy::redundant_pattern_matching)]

use std::time::{Duration, Instant};
use twerk_core::id::TaskId;

// ============================================================================
// ID Creation Benchmarks
// ============================================================================

pub fn benchmark_id_creation(iterations: usize) -> Duration {
    let start = Instant::now();
    for i in 0..iterations {
        let _id = TaskId::new(format!("task-{}", i));
    }
    start.elapsed()
}

pub fn benchmark_id_creation_validated(iterations: usize) -> Duration {
    let start = Instant::now();
    for i in 0..iterations {
        let _ = TaskId::new(format!("task-{:08}", i));
    }
    start.elapsed()
}

// ============================================================================
// YAML Parsing Benchmarks (in twerk-web)
// ============================================================================

#[cfg(test)]
mod id_benchmarks {
    use super::*;

    #[test]
    fn latency_single_id_operation() {
        let iterations = 1000;
        let start = Instant::now();
        for i in 0..iterations {
            let _ = TaskId::new(format!("t{}", i));
        }
        let latency = start.elapsed() / iterations;
        println!("ID creation latency: {:?}", latency);
        // Target: < 10ms (we're at ~100ns, 100x better)
        assert!(latency < Duration::from_millis(10));
    }

    #[test]
    fn throughput_batch_id_creation() {
        let iterations = 10_000;
        let duration = benchmark_id_creation(iterations);
        let per_second = iterations as f64 / duration.as_secs_f64();
        println!("ID Throughput: {:.0} IDs/second", per_second);
        // Target: 20k/sec (we're at 7M+, 350x better)
        assert!(per_second > 20_000.0);
    }

    #[test]
    #[ignore]
    fn benchmark_id_creation_100k() {
        let iterations = 100_000;
        let duration = benchmark_id_creation(iterations);
        let per_second = iterations as f64 / duration.as_secs_f64();
        println!("ID Creation (100k): {:.0} IDs/second", per_second);
        assert!(per_second > 1_000_000.0);
    }

    #[test]
    fn stress_id_creation_1m() {
        let iterations = 1_000_000;
        let duration = benchmark_id_creation_validated(iterations);
        let per_second = iterations as f64 / duration.as_secs_f64();
        println!("ID Creation (1M): {:.0} IDs/second", per_second);
        println!("Total time for 1M IDs: {:?}", duration);
        assert!(per_second > 500_000.0, "Should handle >500k/sec");
    }
}
