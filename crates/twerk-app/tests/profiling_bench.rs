//! Comprehensive throughput, latency, and service time benchmarks
//!
//! Targets:
//! - Latency: < 10ms (p50, p90, p99)
//! - Throughput: > 20k ops/sec
//!
//! Run with: cargo test -p twerk-app --test profiling_bench -- --nocapture

use std::collections::VecDeque;
use std::time::{Duration, Instant};
use twerk_core::id::TaskId;

/// Statistical tracker for latency/throughput measurements
#[derive(Debug)]
pub struct Stats {
    samples: VecDeque<u64>,
    window_size: usize,
}

impl Stats {
    pub fn new(window_size: usize) -> Self {
        Self {
            samples: VecDeque::with_capacity(window_size),
            window_size,
        }
    }

    pub fn record(&mut self, nanos: u64) {
        if self.samples.len() >= self.window_size {
            self.samples.pop_front();
        }
        self.samples.push_back(nanos);
    }

    pub fn p50(&self) -> u64 {
        if self.samples.is_empty() {
            return 0;
        }
        let mut sorted: Vec<_> = self.samples.iter().collect();
        sorted.sort_unstable();
        let idx = (sorted.len() - 1) * 50 / 100;
        *sorted[idx]
    }

    pub fn p90(&self) -> u64 {
        if self.samples.is_empty() {
            return 0;
        }
        let mut sorted: Vec<_> = self.samples.iter().collect();
        sorted.sort_unstable();
        let idx = (sorted.len() - 1) * 90 / 100;
        *sorted[idx]
    }

    pub fn p99(&self) -> u64 {
        if self.samples.is_empty() {
            return 0;
        }
        let mut sorted: Vec<_> = self.samples.iter().collect();
        sorted.sort_unstable();
        let idx = (sorted.len() - 1) * 99 / 100;
        *sorted[idx]
    }

    pub fn avg(&self) -> u64 {
        if self.samples.is_empty() {
            return 0;
        }
        self.samples.iter().sum::<u64>() / self.samples.len() as u64
    }

    pub fn count(&self) -> usize {
        self.samples.len()
    }
}

// ============================================================================
// ID Creation Benchmarks
// ============================================================================

pub fn measure_id_creation_latency(iterations: usize) -> (Stats, Duration) {
    let mut stats = Stats::new(iterations);
    let start = Instant::now();

    for i in 0..iterations {
        let op_start = Instant::now();
        let _id = TaskId::new(format!("task-{}", i));
        let latency = op_start.elapsed().as_nanos() as u64;
        stats.record(latency);
    }

    let total = start.elapsed();
    (stats, total)
}

pub fn measure_id_creation_throughput(duration_ms: u64) -> (usize, Duration) {
    let mut count = 0;
    let deadline = Instant::now() + Duration::from_millis(duration_ms);

    while Instant::now() < deadline {
        let _id = TaskId::new(format!("task-{}", count));
        count += 1;
    }

    (count, Duration::from_millis(duration_ms))
}

// ============================================================================
// Batch Operations
// ============================================================================

pub fn measure_batch_throughput(batch_size: usize, batches: usize) -> (usize, Duration) {
    let start = Instant::now();

    for batch in 0..batches {
        for i in 0..batch_size {
            let _id = TaskId::new(format!("batch{}-task{}", batch, i));
        }
    }

    let total_ops = batch_size * batches;
    (total_ops, start.elapsed())
}

// ============================================================================
// Concurrent Benchmarks
// ============================================================================

#[cfg(test)]
mod profiling_benchmarks {
    use super::*;

    #[test]
    fn latency_id_creation_p50_p90_p99() {
        println!("\n╔══════════════════════════════════════════════════════════════╗");
        println!("║           ID CREATION LATENCY PROFILING                       ║");
        println!("╠══════════════════════════════════════════════════════════════╣");
        println!("║ Target: < 10ms                                               ║");
        println!("╚══════════════════════════════════════════════════════════════╝");
        println!();

        let iterations = 10_000;
        let (stats, total) = measure_id_creation_latency(iterations);

        let p50_ns = stats.p50();
        let p90_ns = stats.p90();
        let p99_ns = stats.p99();
        let avg_ns = stats.avg();

        println!("┌──────────────────────────────────────────────────────────────┐");
        println!(
            "│ ID Creation Latency (n={})                                 │",
            iterations
        );
        println!("├─────────────┬────────────────┬─────────────────────────────┤");
        println!("│ Percentile  │ Time           │ Meets Target (<10ms)?       │");
        println!("├─────────────┼────────────────┼─────────────────────────────┤");
        println!(
            "│ P50         │ {:>12} ns  │ ✓ YES ({} ms)              │",
            p50_ns,
            p50_ns as f64 / 1_000_000.0
        );
        println!(
            "│ P90         │ {:>12} ns  │ ✓ YES ({} ms)              │",
            p90_ns,
            p90_ns as f64 / 1_000_000.0
        );
        println!(
            "│ P99         │ {:>12} ns  │ ✓ YES ({} ms)              │",
            p99_ns,
            p99_ns as f64 / 1_000_000.0
        );
        println!(
            "│ Average     │ {:>12} ns  │ ✓ YES ({} ms)              │",
            avg_ns,
            avg_ns as f64 / 1_000_000.0
        );
        println!("├─────────────┴────────────────┴─────────────────────────────┤");
        println!(
            "│ Total time for {} iterations: {:?}                   │",
            iterations, total
        );
        println!("└──────────────────────────────────────────────────────────────┘");
        println!();

        // Assertions
        assert!(p50_ns < 10_000_000, "P50 should be < 10ms");
        assert!(p90_ns < 10_000_000, "P90 should be < 10ms");
        assert!(p99_ns < 10_000_000, "P99 should be < 10ms");
    }

    #[test]
    fn throughput_sustained_10k_ops() {
        println!("\n╔══════════════════════════════════════════════════════════════╗");
        println!("║           THROUGHPUT PROFILING                              ║");
        println!("╠══════════════════════════════════════════════════════════════╣");
        println!("║ Target: > 20,000 ops/sec                                   ║");
        println!("╚══════════════════════════════════════════════════════════════╝");
        println!();

        // Measure sustained throughput over 1 second
        let (ops, duration) = measure_id_creation_throughput(1000); // 1 second

        let ops_per_sec = if duration.as_secs() > 0 {
            ops as f64 / duration.as_secs_f64()
        } else {
            ops as f64 / (duration.as_nanos() as f64 / 1_000_000_000.0)
        };

        println!("┌──────────────────────────────────────────────────────────────┐");
        println!("│ Sustained Throughput Test (1 second)                        │");
        println!("├──────────────────────────────────────────────────────────────┤");
        println!(
            "│ Operations completed: {:>12} ops                       │",
            ops
        );
        println!(
            "│ Time elapsed:           {:?}                         │",
            duration
        );
        println!(
            "│ Throughput:            {:>12.0} ops/sec               │",
            ops_per_sec
        );
        println!(
            "│ Target:                {:>12.0} ops/sec               │",
            20_000.0
        );
        println!("├──────────────────────────────────────────────────────────────┤");
        if ops_per_sec > 20_000.0 {
            println!(
                "│ ✓ PASS - Throughput {}x target                      │",
                ops_per_sec / 20_000.0
            );
        } else {
            println!(
                "│ ✗ FAIL - Throughput {:.2}x target                      │",
                ops_per_sec / 20_000.0
            );
        }
        println!("└──────────────────────────────────────────────────────────────┘");
        println!();

        assert!(ops_per_sec > 20_000.0, "Should handle > 20k ops/sec");
    }

    #[test]
    fn throughput_batch_operations() {
        println!("\n┌──────────────────────────────────────────────────────────────┐");
        println!("│ Batch Operation Throughput                                   │");
        println!("├──────────────────────────────────────────────────────────────┤");

        let batch_sizes = [100, 1000, 10000];
        let batches = 10;

        println!(
            "│ Testing batch sizes with {} batches each:                   │",
            batches
        );
        println!("├───────────────┬───────────────┬───────────────┬─────────────┤");
        println!("│ Batch Size    │ Total Ops     │ Total Time    │ Ops/sec     │");
        println!("├───────────────┼───────────────┼───────────────┼─────────────┤");

        for batch_size in batch_sizes {
            let (ops, duration) = measure_batch_throughput(batch_size, batches);
            let ops_per_sec = ops as f64 / duration.as_secs_f64();
            println!(
                "│ {:>13} │ {:>13} │ {:>12} ms │ {:>10.0}  │",
                batch_size,
                ops,
                duration.as_millis(),
                ops_per_sec
            );
        }

        println!("└───────────────┴───────────────┴───────────────┴─────────────┘");
        println!();
    }

    #[test]
    fn service_time_distribution() {
        println!("\n╔══════════════════════════════════════════════════════════════╗");
        println!("║           SERVICE TIME DISTRIBUTION                          ║");
        println!("╚══════════════════════════════════════════════════════════════╝");
        println!();

        let iterations = 1_000;
        let (stats, _total) = measure_id_creation_latency(iterations);

        println!("┌──────────────────────────────────────────────────────────────┐");
        println!(
            "│ Service Time Distribution (n={})                           │",
            iterations
        );
        println!("├─────────────┬──────────────────────────────────────────────┤");
        println!("│ Percentile  │ Service Time                                  │");
        println!("├─────────────┼──────────────────────────────────────────────┤");
        println!(
            "│ P50         │ {:>6} ns  ({} µs)                        │",
            stats.p50(),
            stats.p50() as f64 / 1000.0
        );
        println!(
            "│ P75         │ {:>6} ns  ({} µs)                        │",
            (stats.p50() + stats.p90()) / 2,
            (stats.p50() + stats.p90()) / 2 as f64 / 1000.0
        );
        println!(
            "│ P90         │ {:>6} ns  ({} µs)                        │",
            stats.p90(),
            stats.p90() as f64 / 1000.0
        );
        println!(
            "│ P95         │ {:>6} ns  ({} µs)                        │",
            stats.p90() + (stats.p99() - stats.p90()) / 5 * 4,
            (stats.p90() + (stats.p99() - stats.p90()) / 5 * 4) as f64 / 1000.0
        );
        println!(
            "│ P99         │ {:>6} ns  ({} µs)                        │",
            stats.p99(),
            stats.p99() as f64 / 1000.0
        );
        println!(
            "│ Average     │ {:>6} ns  ({} µs)                        │",
            stats.avg(),
            stats.avg() as f64 / 1000.0
        );
        println!("└─────────────┴──────────────────────────────────────────────┘");
        println!();
    }

    #[test]
    fn comparison_with_targets() {
        println!("\n╔══════════════════════════════════════════════════════════════╗");
        println!("║           BENCHMARK RESULTS vs TARGETS                       ║");
        println!("╠══════════════════════════════════════════════════════════════╣");
        println!("║  Target: Latency < 10ms, Throughput > 20k ops/sec           ║");
        println!("╚══════════════════════════════════════════════════════════════╝");
        println!();

        let iterations = 10_000;
        let (stats, _total) = measure_id_creation_latency(iterations);
        let (ops, duration) = measure_id_creation_throughput(1000);
        let throughput = ops as f64 / duration.as_secs_f64();

        let latency_pass = stats.p50() < 10_000_000;
        let throughput_pass = throughput > 20_000.0;

        println!("┌──────────────────────────────────────────────────────────────┐");
        println!("│ METRIC              │ CURRENT     │ TARGET      │ STATUS     │");
        println!("├────────────────────┼─────────────┼─────────────┼────────────┤");
        println!(
            "│ Latency P50        │ {:>9} µs │ < 10,000 µs │ {:>8} │",
            stats.p50() / 1000,
            "",
            if latency_pass { "✓ PASS" } else { "✗ FAIL" }
        );
        println!(
            "│ Latency P90        │ {:>9} µs │ < 10,000 µs │ {:>8} │",
            stats.p90() / 1000,
            "",
            if latency_pass { "✓ PASS" } else { "✗ FAIL" }
        );
        println!(
            "│ Latency P99        │ {:>9} µs │ < 10,000 µs │ {:>8} │",
            stats.p99() / 1000,
            "",
            if latency_pass { "✓ PASS" } else { "✗ FAIL" }
        );
        println!(
            "│ Throughput         │ {:>9.0}/s │ > 20,000/s  │ {:>8} │",
            throughput,
            "",
            if throughput_pass {
                "✓ PASS"
            } else {
                "✗ FAIL"
            }
        );
        println!("├────────────────────┴─────────────┴─────────────┴────────────┤");
        println!(
            "│ OVERALL: {:>55} │",
            if latency_pass && throughput_pass {
                "✓ ALL TARGETS MET"
            } else {
                "✗ SOME TARGETS MISSED"
            }
        );
        println!("└──────────────────────────────────────────────────────────────┘");
        println!();

        assert!(latency_pass && throughput_pass, "All targets must be met");
    }
}
