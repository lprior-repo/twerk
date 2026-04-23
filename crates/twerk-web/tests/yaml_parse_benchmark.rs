//! TRUE YAML Parsing Benchmark - using twerk-web's actual YAML parser
//!
//! This PROVES actual YAML parsing throughput through the twerk-web API

use std::time::{Duration, Instant};
use twerk_core::job::Job;
use twerk_web::api::yaml::from_slice;

/// TRUE YAML parsing benchmark using twerk-web's actual YAML parser
fn benchmark_yaml_parse_real(iterations: usize) -> Duration {
    let yaml = r#"
name: benchmark-job
version: "1.0"
tasks:
  - name: test-task
    image: bash:latest
    command: ["echo", "hello world"]
"#;

    (0..1_000).for_each(|_| {
        let _: Result<Job, _> = from_slice(yaml.as_bytes());
    });

    let start = Instant::now();
    (0..iterations).for_each(|_| {
        // This calls the actual twerk-web YAML parser, deserializing to Job
        let _: Result<Job, _> = from_slice(yaml.as_bytes());
    });
    start.elapsed()
}

#[cfg(test)]
mod true_yaml_benchmark {
    use super::*;

    #[test]
    fn true_yaml_parse_throughput_100k() {
        let iterations = 100_000;
        let duration = benchmark_yaml_parse_real(iterations);
        let per_sec = iterations as f64 / duration.as_secs_f64();

        println!();
        println!("╔══════════════════════════════════════════════════════════════════════╗");
        println!("║  TRUE YAML PARSING BENCHMARK (twerk-web::api::yaml::from_slice)    ║");
        println!("╠══════════════════════════════════════════════════════════════════════╣");
        println!("║  This uses the ACTUAL twerk YAML parser → Job deserialization     ║");
        println!("╚══════════════════════════════════════════════════════════════════════╝");
        println!();
        println!("┌─────────────────────────────────────────────────────────────────────┐");
        println!("│ YAML Parsing Throughput (Job deserialization)                       │");
        println!("├─────────────────────────────────────────────────────────────────────┤");
        println!(
            "│ Iterations:              {:>15}                          │",
            iterations
        );
        println!(
            "│ Total time:              {:>15?}                          │",
            duration
        );
        println!(
            "│ Throughput:             {:>15.0} parses/sec                 │",
            per_sec
        );
        println!("├─────────────────────────────────────────────────────────────────────┤");

        let target = 5_000.0;
        if per_sec > target {
            println!(
                "│ ✓ PASS - {:.2}x target (5k/sec)                               │",
                per_sec / target
            );
        } else {
            println!(
                "│ ✗ FAIL - {:.2}x target (5k/sec)                               │",
                per_sec / target
            );
        }
        println!("└─────────────────────────────────────────────────────────────────────┘");
        println!();

        assert!(per_sec > target, "YAML parsing should handle > 5k/sec");
    }

    #[test]
    fn true_yaml_parse_latency_p50_p90_p99() {
        let yaml = r#"
name: benchmark-job
version: "1.0"
tasks:
  - name: test-task
    image: bash:latest
    command: ["echo", "hello world"]
"#;

        let iterations = 10_000;
        let mut latencies: Vec<u64> = (0..iterations)
            .map(|_| {
                let start = Instant::now();
                let _: Result<Job, _> = from_slice(yaml.as_bytes());
                start.elapsed().as_nanos() as u64
            })
            .collect();

        latencies.sort();
        let p50 = latencies[iterations * 50 / 100];
        let p90 = latencies[iterations * 90 / 100];
        let p99 = latencies[iterations * 99 / 100];
        let avg: u64 = latencies.iter().sum::<u64>() / iterations as u64;

        println!();
        println!("┌─────────────────────────────────────────────────────────────────────┐");
        println!(
            "│ YAML Parsing Latency Distribution (n={})                         │",
            iterations
        );
        println!("├───────────┬────────────────┬─────────────────────────────────────┤");
        println!("│ Percentile│ Time (µs)     │ Meets Target (<10ms)?              │");
        println!("├───────────┼────────────────┼─────────────────────────────────────┤");
        println!(
            "│ P50       │ {:>10.3} µs  │ {}                              │",
            p50 as f64 / 1000.0,
            if p50 < 10_000 { "✓ YES" } else { "✗ NO" }
        );
        println!(
            "│ P90       │ {:>10.3} µs  │ {}                              │",
            p90 as f64 / 1000.0,
            if p90 < 10_000 { "✓ YES" } else { "✗ NO" }
        );
        println!(
            "│ P99       │ {:>10.3} µs  │ {}                              │",
            p99 as f64 / 1000.0,
            if p99 < 10_000 { "✓ YES" } else { "✗ NO" }
        );
        println!(
            "│ Average   │ {:>10.3} µs  │ {}                              │",
            avg as f64 / 1000.0,
            if avg < 10_000 { "✓ YES" } else { "✗ NO" }
        );
        println!("└───────────┴────────────────┴─────────────────────────────────────┘");
        println!();

        assert!(p50 < 10_000_000, "P50 should be < 10ms");
        assert!(p90 < 10_000_000, "P90 should be < 10ms");
        assert!(p99 < 10_000_000, "P99 should be < 10ms");
    }
}
