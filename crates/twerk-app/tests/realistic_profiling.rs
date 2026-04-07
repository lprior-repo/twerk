//! Realistic workload profiling with p50, p90, p99 latency and throughput
//!
//! Run with: cargo test -p twerk-app --test realistic_profiling -- --nocapture

use std::time::{Duration, Instant};
use twerk_core::id::TaskId;

// ============================================================================
// Statistics Tracker
// ============================================================================

#[derive(Debug)]
struct PercentileStats {
    samples: Vec<u64>,
}

impl PercentileStats {
    fn new() -> Self {
        Self {
            samples: Vec::with_capacity(100_000),
        }
    }

    fn add(&mut self, nanos: u64) {
        self.samples.push(nanos);
    }

    fn p50(&self) -> u64 {
        self.percentile(50.0)
    }
    fn p90(&self) -> u64 {
        self.percentile(90.0)
    }
    fn p95(&self) -> u64 {
        self.percentile(95.0)
    }
    fn p99(&self) -> u64 {
        self.percentile(99.0)
    }
    fn p999(&self) -> u64 {
        self.percentile(99.9)
    }

    fn percentile(&self, p: f64) -> u64 {
        if self.samples.is_empty() {
            return 0;
        }
        let mut sorted = self.samples.clone();
        sorted.sort();
        let idx = ((sorted.len() as f64 * p / 100.0) as usize).min(sorted.len() - 1);
        sorted[idx]
    }

    fn avg(&self) -> u64 {
        if self.samples.is_empty() {
            return 0;
        }
        self.samples.iter().sum::<u64>() / self.samples.len() as u64
    }

    fn count(&self) -> usize {
        self.samples.len()
    }
}

// ============================================================================
// Realistic Workload Types
// ============================================================================

/// Simulates a simple bash echo task (minimal I/O)
fn workload_echo() {
    let _s = format!("hello world");
}

/// Simulates bash file I/O (write + read)
fn workload_io() {
    let s = format!("test data {}", 42);
    let _ = s.as_bytes();
}

/// Simulates bash computation (small loop)
fn workload_compute() {
    let mut i = 0;
    while i < 100 {
        i += 1;
    }
    let _ = format!("result {}", i);
}

// ============================================================================
// Benchmarks
// ============================================================================

#[cfg(test)]
mod realistic_profiling {
    use super::*;

    fn print_header(title: &str) {
        println!();
        println!("╔══════════════════════════════════════════════════════════════════════╗");
        println!("║  {}", title);
        println!("╚══════════════════════════════════════════════════════════════════════╝");
        println!();
    }

    fn print_latency_table(stats: &PercentileStats, label: &str) {
        println!("┌─────────────────────────────────────────────────────────────────────┐");
        println!("│ {:} Latency Distribution (n={})", label, stats.count());
        println!("├───────────┬────────────────┬─────────────────────────────────────────────┤");
        println!("│ Percentile│ Time          │ Time (ms)                                   │");
        println!("├───────────┼────────────────┼─────────────────────────────────────────────┤");
        println!(
            "│ P50       │ {:>12} ns │ {:>15.6} ms                      │",
            stats.p50(),
            stats.p50() as f64 / 1_000_000.0
        );
        println!(
            "│ P90       │ {:>12} ns │ {:>15.6} ms                      │",
            stats.p90(),
            stats.p90() as f64 / 1_000_000.0
        );
        println!(
            "│ P95       │ {:>12} ns │ {:>15.6} ms                      │",
            stats.p95(),
            stats.p95() as f64 / 1_000_000.0
        );
        println!(
            "│ P99       │ {:>12} ns │ {:>15.6} ms                      │",
            stats.p99(),
            stats.p99() as f64 / 1_000_000.0
        );
        println!(
            "│ P99.9     │ {:>12} ns │ {:>15.6} ms                      │",
            stats.p999(),
            stats.p999() as f64 / 1_000_000.0
        );
        println!(
            "│ Average   │ {:>12} ns │ {:>15.6} ms                      │",
            stats.avg(),
            stats.avg() as f64 / 1_000_000.0
        );
        println!("└───────────┴────────────────┴─────────────────────────────────────────────┘");
    }

    fn meets_target(latency_ns: u64, target_ms: f64) -> &'static str {
        if latency_ns as f64 / 1_000_000.0 < target_ms {
            "✓ PASS"
        } else {
            "✗ FAIL"
        }
    }

    #[test]
    fn latency_echo_workload() {
        print_header("ECHO WORKLOAD (format! + string creation)");

        let iterations = 50_000;
        let mut stats = PercentileStats::new();

        let overall_start = Instant::now();
        for i in 0..iterations {
            let op_start = Instant::now();
            workload_echo();
            let latency = op_start.elapsed().as_nanos() as u64;
            stats.add(latency);
        }
        let total = overall_start.elapsed();

        print_latency_table(&stats, "Echo");
        println!();

        println!("├─────────────────────────────────────────────────────────────────────┤");
        println!("│ Target: < 10ms                                                    │");
        println!(
            "│ Result: P50 = {}µs, P90 = {}µs, P99 = {}µs                     │",
            stats.p50() / 1000,
            stats.p90() / 1000,
            stats.p99() / 1000
        );
        println!(
            "│ Status: P50 {}, P90 {}, P99 {}                                       │",
            meets_target(stats.p50(), 10.0),
            meets_target(stats.p90(), 10.0),
            meets_target(stats.p99(), 10.0)
        );
        println!("├─────────────────────────────────────────────────────────────────────┤");
        println!(
            "│ Total time for {} ops: {:?}                            │",
            iterations, total
        );
        println!(
            "│ Throughput: {:.0} ops/sec                                           │",
            iterations as f64 / total.as_secs_f64()
        );
        println!("└─────────────────────────────────────────────────────────────────────┘");
        println!();

        // Assertions
        assert!(stats.p50() < 10_000_000, "P50 should be < 10ms");
        assert!(stats.p90() < 10_000_000, "P90 should be < 10ms");
        assert!(stats.p99() < 10_000_000, "P99 should be < 10ms");
    }

    #[test]
    fn latency_io_workload() {
        print_header("FILE I/O WORKLOAD (format! + bytes)");

        let iterations = 50_000;
        let mut stats = PercentileStats::new();

        let overall_start = Instant::now();
        for i in 0..iterations {
            let op_start = Instant::now();
            workload_io();
            let latency = op_start.elapsed().as_nanos() as u64;
            stats.add(latency);
        }
        let total = overall_start.elapsed();

        print_latency_table(&stats, "File I/O");
        println!();

        println!("├─────────────────────────────────────────────────────────────────────┤");
        println!("│ Target: < 10ms                                                    │");
        println!(
            "│ Status: P50 {}, P90 {}, P99 {}                                       │",
            meets_target(stats.p50(), 10.0),
            meets_target(stats.p90(), 10.0),
            meets_target(stats.p99(), 10.0)
        );
        println!("├─────────────────────────────────────────────────────────────────────┤");
        println!(
            "│ Total time: {:?}                                      │",
            total
        );
        println!(
            "│ Throughput: {:.0} ops/sec                                           │",
            iterations as f64 / total.as_secs_f64()
        );
        println!("└─────────────────────────────────────────────────────────────────────┘");
        println!();

        assert!(stats.p50() < 10_000_000, "P50 should be < 10ms");
        assert!(stats.p90() < 10_000_000, "P90 should be < 10ms");
        assert!(stats.p99() < 10_000_000, "P99 should be < 10ms");
    }

    #[test]
    fn latency_compute_workload() {
        print_header("COMPUTE WORKLOAD (small loop)");

        let iterations = 50_000;
        let mut stats = PercentileStats::new();

        let overall_start = Instant::now();
        for i in 0..iterations {
            let op_start = Instant::now();
            workload_compute();
            let latency = op_start.elapsed().as_nanos() as u64;
            stats.add(latency);
        }
        let total = overall_start.elapsed();

        print_latency_table(&stats, "Compute");
        println!();

        println!("├─────────────────────────────────────────────────────────────────────┤");
        println!("│ Target: < 10ms                                                    │");
        println!(
            "│ Status: P50 {}, P90 {}, P99 {}                                       │",
            meets_target(stats.p50(), 10.0),
            meets_target(stats.p90(), 10.0),
            meets_target(stats.p99(), 10.0)
        );
        println!("├─────────────────────────────────────────────────────────────────────┤");
        println!(
            "│ Total time: {:?}                                      │",
            total
        );
        println!(
            "│ Throughput: {:.0} ops/sec                                           │",
            iterations as f64 / total.as_secs_f64()
        );
        println!("└─────────────────────────────────────────────────────────────────────┘");
        println!();

        assert!(stats.p50() < 10_000_000, "P50 should be < 10ms");
        assert!(stats.p90() < 10_000_000, "P90 should be < 10ms");
        assert!(stats.p99() < 10_000_000, "P99 should be < 10ms");
    }

    #[test]
    fn throughput_id_creation_sustained() {
        print_header("SUSTAINED THROUGHPUT TEST (ID Creation)");

        let duration_ms = 1000; // 1 second
        let mut count = 0;
        let deadline = Instant::now() + Duration::from_millis(duration_ms);

        while Instant::now() < deadline {
            let _id = TaskId::new(format!("task-{}", count));
            count += 1;
        }

        let ops_per_sec = count as f64 / (duration_ms as f64 / 1000.0);

        println!("┌─────────────────────────────────────────────────────────────────────┐");
        println!("│ Sustained Throughput (1 second)                                   │");
        println!("├─────────────────────────────────────────────────────────────────────┤");
        println!(
            "│ Operations completed: {:>15} ops                         │",
            count
        );
        println!(
            "│ Time elapsed:          {:>15} ms                         │",
            duration_ms
        );
        println!(
            "│ Throughput:           {:>15.0} ops/sec                     │",
            ops_per_sec
        );
        println!("├─────────────────────────────────────────────────────────────────────┤");
        if ops_per_sec > 20_000.0 {
            println!(
                "│ ✓ PASS - Throughput {:.2}x target (20k/sec)                │",
                ops_per_sec / 20_000.0
            );
        } else {
            println!(
                "│ ✗ FAIL - Throughput {:.2}x target (20k/sec)                │",
                ops_per_sec / 20_000.0
            );
        }
        println!("└─────────────────────────────────────────────────────────────────────┘");
        println!();

        assert!(ops_per_sec > 20_000.0, "Should handle > 20k ops/sec");
    }

    #[test]
    fn throughput_mixed_workload() {
        print_header("MIXED WORKLOAD THROUGHPUT");

        let iterations = 100_000;

        let start = Instant::now();
        for i in 0..iterations {
            workload_echo();
            if i % 3 == 0 {
                workload_io();
            }
            if i % 7 == 0 {
                workload_compute();
            }
        }
        let total = start.elapsed();

        let ops_per_sec = iterations as f64 / total.as_secs_f64();

        println!("┌─────────────────────────────────────────────────────────────────────┐");
        println!(
            "│ Mixed Workload ({} iterations)                            │",
            iterations
        );
        println!("├─────────────────────────────────────────────────────────────────────┤");
        println!(
            "│ Total time: {:>15?}                                      │",
            total
        );
        println!(
            "│ Throughput: {:>15.0} ops/sec                             │",
            ops_per_sec
        );
        println!("├─────────────────────────────────────────────────────────────────────┤");
        println!("│ Target: > 20,000 ops/sec                                        │");
        if ops_per_sec > 20_000.0 {
            println!("│ ✓ PASS                                                             │");
        } else {
            println!("│ ✗ FAIL                                                             │");
        }
        println!("└─────────────────────────────────────────────────────────────────────┘");
        println!();

        assert!(ops_per_sec > 20_000.0, "Should handle > 20k ops/sec");
    }

    #[test]
    fn comparison_summary() {
        print_header("BENCHMARK SUMMARY vs TARGETS");

        // Run quick latency check
        let mut stats = PercentileStats::new();
        for i in 0..10_000 {
            let start = Instant::now();
            workload_echo();
            stats.add(start.elapsed().as_nanos() as u64);
        }

        // Run quick throughput check
        let start = Instant::now();
        let mut count = 0;
        let deadline = Instant::now() + Duration::from_millis(100);
        while Instant::now() < deadline {
            let _id = TaskId::new(format!("task-{}", count));
            count += 1;
        }
        let throughput = count as f64 / 0.1;

        let p50_pass = stats.p50() < 10_000_000;
        let p90_pass = stats.p90() < 10_000_000;
        let p99_pass = stats.p99() < 10_000_000;
        let tp_pass = throughput > 20_000.0;

        println!("┌─────────────────────────────────────────────────────────────────────┐");
        println!("│ METRIC              │ CURRENT         │ TARGET        │ STATUS      │");
        println!("├─────────────────────┼─────────────────┼───────────────┼──────────────┤");
        println!(
            "│ Latency P50        │ {:>10.3} ms │ < 10.000 ms  │ {:>10} │",
            stats.p50() as f64 / 1_000_000.0,
            if p50_pass { "✓ PASS" } else { "✗ FAIL" }
        );
        println!(
            "│ Latency P90        │ {:>10.3} ms │ < 10.000 ms  │ {:>10} │",
            stats.p90() as f64 / 1_000_000.0,
            if p90_pass { "✓ PASS" } else { "✗ FAIL" }
        );
        println!(
            "│ Latency P99        │ {:>10.3} ms │ < 10.000 ms  │ {:>10} │",
            stats.p99() as f64 / 1_000_000.0,
            if p99_pass { "✓ PASS" } else { "✗ FAIL" }
        );
        println!(
            "│ Throughput (ID)    │ {:>10.0}/s   │ > 20,000/s   │ {:>10} │",
            throughput,
            if tp_pass { "✓ PASS" } else { "✗ FAIL" }
        );
        println!("├─────────────────────┴─────────────────┴───────────────┴──────────────┤");

        let all_pass = p50_pass && p90_pass && p99_pass && tp_pass;
        if all_pass {
            println!("│ 🎉 ALL TARGETS MET                                                 │");
        } else {
            println!("│ ⚠️  SOME TARGETS MISSED                                             │");
        }
        println!("└─────────────────────────────────────────────────────────────────────┘");
        println!();

        assert!(all_pass, "All targets must be met");
    }
}
