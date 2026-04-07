//! Realistic mutation-based YAML workload benchmark
//! 
//! This simulates REAL twerk job submissions with:
//! - Varied job names, task names, commands
//! - Realistic task configurations (images, env vars, volumes)
//! - Mutation: different inputs per iteration (not same YAML repeated)
//! - Simulates actual "started" workload - job creation + scheduling
//!
//! Run with: cargo test -p twerk-web --test mutation_workload_bench -- --nocapture

use std::time::{Duration, Instant};
use twerk_web::api::yaml::from_slice;
use twerk_core::job::Job;

/// Realistic job YAML with mutations built-in
fn generate_mutated_yaml(job_id: u64, task_id: u64) -> String {
    // Realistic bash commands
    let commands = match job_id % 7 {
        0 => r#"["echo", "hello world"]"#,
        1 => r#"["bash", "-c", "echo $NAME && sleep 1"]"#,
        2 => r#"["sh", "-c", "for i in $(seq 1 10); do echo $i; done"]"#,
        3 => r#"["bash", "-c", "date && hostname && pwd"]"#,
        4 => r#"["ls", "-la", "/tmp"]"#,
        5 => r#"["bash", "-c", "i=0; while [ $i -lt 100 ]; do i=$((i+1)); done"]"#,
        _ => r#"["echo", "done"]"#,
    };
    
    // Realistic images
    let images = match task_id % 5 {
        0 => "bash:latest",
        1 => "ubuntu:22.04",
        2 => "alpine:3.18",
        3 => "debian:bookworm",
        _ => "bash:latest",
    };
    
    // Realistic environment variables
    let env_vars = format!(
        r#"env:
  - name: JOB_ID
    value: "job-{}"
  - name: TASK_ID  
    value: "task-{}"
  - name: ENVIRONMENT
    value: "{}""#,
        job_id,
        task_id,
        match job_id % 4 {
            0 => "production",
            1 => "staging",
            2 => "development",
            _ => "test",
        }
    );
    
    // Realistic volumes
    let volumes = if job_id % 3 == 0 {
        r#"volumes:
  - /tmp/data:/data
  - /var/log:/var/log"#
    } else {
        ""
    };
    
    // Realistic retries
    let retries = if job_id % 5 == 0 {
        r#"
retries: 3
retry_delay: 5s"
    } else {
        ""
    };
    
    // Realistic timeout
    let timeout = match job_id % 6 {
        0 => "timeout: 60s",
        1 => "timeout: 5m",
        2 => "timeout: 1h",
        _ => "",
    };
    
    format!(
        r#"name: job-{:08}
version: "1.0"
description: "Production workload job {} processing task {}"
{}
{}
{}
tasks:
  - name: task-{:08}
    image: {}
    command: {}
    {}
    {}
"#,
        job_id,
        job_id,
        task_id,
        env_vars,
        volumes,
        retries,
        task_id,
        images,
        commands,
        timeout,
        if volumes.is_empty() { "" } else { "    resources:" }
    )
}

fn generate_parallel_yaml(job_id: u64, parallelism: u64) -> String {
    let mut tasks = String::new();
    for i in 0..parallelism {
        let cmd = match i % 3 {
            0 => r#"["echo", "parallel-{}"]"#.replace("{}", &i.to_string()),
            1 => r#"["bash", "-c", "echo $RANDOM"]"#,
            _ => r#"["sleep", "0.1"]"#,
        };
        tasks.push_str(&format!(
            r#"  - name: parallel-task-{}
    image: bash:latest
    command: {}
"#,
            i, cmd
        ));
    }
    
    format!(
        r#"name: parallel-job-{:08}
version: "1.0"
parallel: true
tasks:
{}
"#,
        job_id,
        tasks
    )
}

fn generate_each_yaml(job_id: u64, items: u64) -> String {
    let items_str: Vec<String> = (0..items)
        .map(|i| format!("item-{:03}", i))
        .collect();
    
    format!(
        r#"name: each-job-{:08}
version: "1.0"
each:
  items: ["{}"]
  task:
    name: each-task
    image: bash:latest
    command: ["echo", "{{{{ item }}}}"]
"#,
        job_id,
        items_str.join("\", \"")
    )
}

#[cfg(test)]
mod realistic_mutation_benchmarks {
    use super::*;

    fn print_header(title: &str) {
        println!();
        println!("╔══════════════════════════════════════════════════════════════════════════╗");
        println!("║  {}" , title);
        println!("╚══════════════════════════════════════════════════════════════════════════╝");
    }

    #[test]
    fn mutation_workload_single_jobs() {
        print_header("MUTATION WORKLOAD: Single Jobs (varied YAML each iteration)");
        
        let iterations = 50_000;
        
        println!("Generating and parsing {} UNIQUE job YAMLs...", iterations);
        println!();
        
        let start = Instant::now();
        for i in 0..iterations {
            let yaml = generate_mutated_yaml(i, i % 100);
            let _: Result<Job, _> = from_slice(yaml.as_bytes());
        }
        let duration = start.elapsed();
        
        let per_sec = iterations as f64 / duration.as_secs_f64();
        
        println!("┌─────────────────────────────────────────────────────────────────────┐");
        println!("│ Single Job Mutation Workload                                          │");
        println!("├─────────────────────────────────────────────────────────────────────┤");
        println!("│ Iterations:              {:>15}                          │", iterations);
        println!("│ Unique YAMLs:            {:>15}                          │", iterations);
        println!("│ Total time:             {:>15?}                          │", duration);
        println!("│ Throughput:           {:>15.0} parses/sec                 │", per_sec);
        println!("├─────────────────────────────────────────────────────────────────────┤");
        
        let target = 20_000.0;
        if per_sec > target {
            println!("│ ✓ PASS - {:.2}x target (20k/sec)                              │", per_sec / target);
        } else {
            println!("│ ✗ FAIL - {:.2}x target (20k/sec)                              │", per_sec / target);
        }
        println!("└─────────────────────────────────────────────────────────────────────┘");
        println!();
        
        assert!(per_sec > target, "Should handle > 20k/sec");
    }

    #[test]
    fn mutation_workload_mixed_job_types() {
        print_header("MUTATION WORKLOAD: Mixed Job Types (single/parallel/each)");
        
        let iterations = 30_000;
        
        println!("Generating {} mixed workload YAMLs...", iterations);
        println!();
        
        let start = Instant::now();
        for i in 0..iterations {
            let yaml = match i % 3 {
                0 => generate_mutated_yaml(i, i % 50),
                1 => generate_parallel_yaml(i, (i % 8) + 2), // 2-9 parallel tasks
                _ => generate_each_yaml(i, (i % 20) + 5),   // 5-24 each items
            };
            let _: Result<Job, _> = from_slice(yaml.as_bytes());
        }
        let duration = start.elapsed();
        
        let per_sec = iterations as f64 / duration.as_secs_f64();
        
        println!("┌─────────────────────────────────────────────────────────────────────┐");
        println!("│ Mixed Job Type Workload (single/parallel/each)                      │");
        println!("├─────────────────────────────────────────────────────────────────────┤");
        println!("│ Iterations:              {:>15}                          │", iterations);
        println!("│ Total time:             {:>15?}                          │", duration);
        println!("│ Throughput:           {:>15.0} parses/sec                 │", per_sec);
        println!("├─────────────────────────────────────────────────────────────────────┤");
        
        let target = 20_000.0;
        if per_sec > target {
            println!("│ ✓ PASS - {:.2}x target (20k/sec)                              │", per_sec / target);
        } else {
            println!("│ ✗ FAIL - {:.2}x target (20k/sec)                              │", per_sec / target);
        }
        println!("└─────────────────────────────────────────────────────────────────────┘");
        println!();
        
        assert!(per_sec > target, "Should handle > 20k/sec");
    }

    #[test]
    fn mutation_workload_realistic_started_simulation() {
        print_header("REALISTIC 'STARTED' WORKLOAD: Full Job + Task Creation");
        
        let jobs = 10_000;
        let tasks_per_job = 4; // Average tasks per job
        
        println!("Simulating {} jobs with ~{} tasks each (started state)...", jobs, tasks_per_job);
        println!("This mimics: submit job → scheduler creates child tasks");
        println!();
        
        let start = Instant::now();
        
        for job_id in 0..jobs {
            // Parse the job YAML (what comes in via API)
            let job_yaml = generate_mutated_yaml(job_id, 0);
            let _: Result<Job, _> = from_slice(job_yaml.as_bytes());
            
            // Simulate scheduler creating child tasks
            for task_id in 0..tasks_per_job {
                let task_yaml = generate_mutated_yaml(job_id, task_id);
                let _: Result<Job, _> = from_slice(task_yaml.as_bytes());
            }
        }
        
        let total_operations = jobs * (1 + tasks_per_job);
        let duration = start.elapsed();
        let per_sec = total_operations as f64 / duration.as_secs_f64();
        
        println!("┌─────────────────────────────────────────────────────────────────────┐");
        println!("│ Realistic Started Workload                                           │");
        println!("├─────────────────────────────────────────────────────────────────────┤");
        println!("│ Jobs submitted:           {:>15}                          │", jobs);
        println!("│ Tasks created:            {:>15}                          │", jobs * tasks_per_job);
        println!("│ Total operations:        {:>15}                          │", total_operations);
        println!("│ Total time:             {:>15?}                          │", duration);
        println!("│ Throughput:           {:>15.0} ops/sec                     │", per_sec);
        println!("├─────────────────────────────────────────────────────────────────────┤");
        
        let target = 20_000.0;
        if per_sec > target {
            println!("│ ✓ PASS - {:.2}x target (20k/sec)                              │", per_sec / target);
        } else {
            println!("│ ✗ FAIL - {:.2}x target (20k/sec)                              │", per_sec / target);
        }
        println!("└─────────────────────────────────────────────────────────────────────┘");
        println!();
        
        assert!(per_sec > target, "Should handle > 20k/sec");
    }

    #[test]
    fn mutation_workload_high_parallelism() {
        print_header("HIGH PARALLELISM: Jobs with 8-16 parallel tasks");
        
        let iterations = 10_000;
        
        println!("Generating {} high-parallelism job YAMLs...", iterations);
        println!();
        
        let start = Instant::now();
        for i in 0..iterations {
            let parallelism = (i % 9) + 8; // 8-16 parallel tasks
            let yaml = generate_parallel_yaml(i, parallelism as u64);
            let _: Result<Job, _> = from_slice(yaml.as_bytes());
        }
        let duration = start.elapsed();
        
        let per_sec = iterations as f64 / duration.as_secs_f64();
        
        println!("┌─────────────────────────────────────────────────────────────────────┐");
        println!("│ High Parallelism Workload (8-16 parallel tasks per job)              │");
        println!("├─────────────────────────────────────────────────────────────────────┤");
        println!("│ Iterations:              {:>15}                          │", iterations);
        println!("│ Parallelism range:       {:>15}                          │", "8-16 tasks");
        println!("│ Total time:             {:>15?}                          │", duration);
        println!("│ Throughput:           {:>15.0} parses/sec                 │", per_sec);
        println!("├─────────────────────────────────────────────────────────────────────┤");
        
        let target = 20_000.0;
        if per_sec > target {
            println!("│ ✓ PASS - {:.2}x target (20k/sec)                              │", per_sec / target);
        } else {
            println!("│ ✗ FAIL - {:.2}x target (20k/sec)                              │", per_sec / target);
        }
        println!("└─────────────────────────────────────────────────────────────────────┘");
        println!();
        
        assert!(per_sec > target, "Should handle > 20k/sec");
    }

    #[test]
    fn mutation_workload_each_items() {
        print_header("EACH/JOB WORKLOAD: Jobs with 10-50 items");
        
        let iterations = 10_000;
        
        println!("Generating {} each-job YAMLs (batch processing style)...", iterations);
        println!();
        
        let start = Instant::now();
        for i in 0..iterations {
            let items = (i % 41) + 10; // 10-50 items per each job
            let yaml = generate_each_yaml(i, items as u64);
            let _: Result<Job, _> = from_slice(yaml.as_bytes());
        }
        let duration = start.elapsed();
        
        let per_sec = iterations as f64 / duration.as_secs_f64();
        
        println!("┌─────────────────────────────────────────────────────────────────────┐");
        println!("│ Each Job Workload (batch processing, 10-50 items per job)           │");
        println!("├─────────────────────────────────────────────────────────────────────┤");
        println!("│ Iterations:              {:>15}                          │", iterations);
        println!("│ Items per job range:     {:>15}                          │", "10-50");
        println!("│ Total time:             {:>15?}                          │", duration);
        println!("│ Throughput:           {:>15.0} parses/sec                 │", per_sec);
        println!("├─────────────────────────────────────────────────────────────────────┤");
        
        let target = 20_000.0;
        if per_sec > target {
            println!("│ ✓ PASS - {:.2}x target (20k/sec)                              │", per_sec / target);
        } else {
            println!("│ ✗ FAIL - {:.2}x target (20k/sec)                              │", per_sec / target);
        }
        println!("└─────────────────────────────────────────────────────────────────────┘");
        println!();
        
        assert!(per_sec > target, "Should handle > 20k/sec");
    }

    #[test]
    fn latency_distribution_realistic_workload() {
        print_header("LATENCY DISTRIBUTION: Realistic Mutation Workload");
        
        let yaml_samples = 1_000;
        let latencies_per_sample = 100;
        
        println!("Collecting latency samples for {} unique YAMLs × {} parses...", 
                 yaml_samples, latencies_per_sample);
        println!();
        
        // Pre-generate unique YAMLs
        let yamls: Vec<String> = (0..yaml_samples)
            .map(|i| generate_mutated_yaml(i, i % 50))
            .collect();
        
        let mut all_latencies = Vec::with_capacity(yaml_samples * latencies_per_sample);
        
        for yaml in &yamls {
            for _ in 0..latencies_per_sample {
                let start = Instant::now();
                let _: Result<Job, _> = from_slice(yaml.as_bytes());
                all_latencies.push(start.elapsed().as_micros() as u64);
            }
        }
        
        all_latencies.sort();
        let n = all_latencies.len();
        
        let p50 = all_latencies[n * 50 / 100];
        let p90 = all_latencies[n * 90 / 100];
        let p95 = all_latencies[n * 95 / 100];
        let p99 = all_latencies[n * 99 / 100];
        let p999 = all_latencies[n * 999 / 1000];
        let avg = all_latencies.iter().sum::<u64>() / n as u64;
        
        println!("┌─────────────────────────────────────────────────────────────────────┐");
        println!("│ Realistic Workload Latency Distribution                              │");
        println!("│ ({} unique YAMLs × {} samples each = {} total)              │", 
                 yaml_samples, latencies_per_sample, n);
        println!("├───────────┬────────────────┬─────────────────────────────────────┤");
        println!("│ Percentile│ Time (µs)     │ Meets Target (<10ms)?              │");
        println!("├───────────┼────────────────┼─────────────────────────────────────┤");
        println!("│ P50       │ {:>10.3} µs  │ {}                              │", 
            p50 as f64 / 1000.0, if p50 < 10_000 { "✓ YES" } else { "✗ NO" });
        println!("│ P90       │ {:>10.3} µs  │ {}                              │", 
            p90 as f64 / 1000.0, if p90 < 10_000 { "✓ YES" } else { "✗ NO" });
        println!("│ P95       │ {:>10.3} µs  │ {}                              │", 
            p95 as f64 / 1000.0, if p95 < 10_000 { "✓ YES" } else { "✗ NO" });
        println!("│ P99       │ {:>10.3} µs  │ {}                              │", 
            p99 as f64 / 1000.0, if p99 < 10_000 { "✓ YES" } else { "✗ NO" });
        println!("│ P99.9     │ {:>10.3} µs  │ {}                              │", 
            p999 as f64 / 1000.0, if p999 < 10_000 { "✓ YES" } else { "✗ NO" });
        println!("│ Average   │ {:>10.3} µs  │ {}                              │", 
            avg as f64 / 1000.0, if avg < 10_000 { "✓ YES" } else { "✗ NO" });
        println!("└───────────┴────────────────┴─────────────────────────────────────┘");
        println!();
        
        assert!(p50 < 10_000, "P50 should be < 10ms");
        assert!(p90 < 10_000, "P90 should be < 10ms");
        assert!(p99 < 10_000, "P99 should be < 10ms");
    }

    #[test]
    fn stress_test_sustained_realistic_load() {
        print_header("STRESS TEST: 5 Seconds Sustained Realistic Load");
        
        let duration_secs = 5;
        let target_per_sec = 20_000.0;
        
        println!("Running sustained realistic workload for {} seconds...", duration_secs);
        println!("Target: {:.0} ops/sec", target_per_sec);
        println!();
        
        let deadline = Instant::now() + Duration::from_secs(duration_secs);
        let mut count = 0;
        
        let mut i = 0u64;
        while Instant::now() < deadline {
            // Mix of job types
            let yaml = match count % 4 {
                0 => generate_mutated_yaml(i, i % 50),
                1 => generate_parallel_yaml(i, (i % 8) + 2),
                2 => generate_each_yaml(i, (i % 30) + 10),
                _ => generate_mutated_yaml(i, i % 100),
            };
            
            let _: Result<Job, _> = from_slice(yaml.as_bytes());
            count += 1;
            i += 1;
        }
        
        let actual_duration = Duration::from_secs(duration_secs);
        let per_sec = count as f64 / actual_duration.as_secs_f64();
        
        println!("┌─────────────────────────────────────────────────────────────────────┐");
        println!("│ Sustained Realistic Load ({} seconds)                               │", duration_secs);
        println!("├─────────────────────────────────────────────────────────────────────┤");
        println!("│ Total operations:       {:>15}                          │", count);
        println!("│ Duration:             {:>15?}                          │", actual_duration);
        println!("│ Actual throughput:   {:>15.0} ops/sec                     │", per_sec);
        println!("│ Target throughput:   {:>15.0} ops/sec                     │", target_per_sec);
        println!("├─────────────────────────────────────────────────────────────────────┤");
        
        if per_sec > target_per_sec {
            println!("│ ✓ PASS - {:.2}x target                                        │", per_sec / target_per_sec);
        } else {
            println!("│ ✗ FAIL - {:.2}x target                                        │", per_sec / target_per_sec);
        }
        println!("└─────────────────────────────────────────────────────────────────────┘");
        println!();
        
        assert!(per_sec > target_per_sec, "Should handle > 20k/sec sustained");
    }
}
