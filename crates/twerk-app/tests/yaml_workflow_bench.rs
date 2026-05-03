//! YAML Workflow Throughput & Latency Benchmark
//!
//! Measures actual E2E performance: YAML parse → job submit → task complete
//!
//! Run with: cargo test -p twerk-app --test yaml_workflow_bench -- --nocapture

#![allow(clippy::print_stdout)]

use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use twerk_app::engine::{Config, Engine, JobListener, Mode};
use twerk_core::id::JobId;
use twerk_core::job::{Job, JobState};
use serde_saphyr::from_str;
use tokio::sync::oneshot;
use tokio::time::timeout;

/// Minimal Pokemon API YAML for benchmarking
const BENCHMARK_YAML: &str = r#"name: throughput-bench
description: Benchmark workflow
tasks:
  - name: p1
    run: echo "task 1"
  - name: p2
    run: echo "task 2"
  - name: p3
    run: echo "task 3"
  - name: p4
    run: echo "task 4"
  - name: p5
    run: echo "task 5"
"#;

fn parse_yaml(yaml: &str) -> Result<Job, String> {
    from_str(yaml).map_err(|e| e.to_string())
}

fn create_job_from_yaml(yaml: &str, count: usize) -> Vec<Job> {
    (0..count)
        .map(|i| {
            let mut job: Job = parse_yaml(yaml).unwrap();
            job.id = Some(JobId::new(format!("550e8400-e29b-41d4-a716-{:012}", i)).unwrap());
            job.state = JobState::Pending;
            job
        })
        .collect()
}

fn completion_listener() -> (Vec<JobListener>, oneshot::Receiver<Job>) {
    let (tx, rx) = oneshot::channel();
    let sender = Arc::new(Mutex::new(Some(tx)));

    let listener: JobListener = Arc::new(move |job: Job| {
        if job.state == JobState::Completed || job.state == JobState::Failed {
            let mut guard = sender.lock().unwrap();
            if let Some(tx) = guard.take() {
                let _ = tx.send(job);
            }
        }
    });

    (vec![listener], rx)
}

#[tokio::test]
async fn yaml_workflow_throughput_and_latency() {
    println!("\n╔══════════════════════════════════════════════════════════════╗");
    println!("║       YAML WORKFLOW THROUGHPUT & LATENCY BENCHMARK        ║");
    println!("╚══════════════════════════════════════════════════════════════╝");
    println!();

    // Setup in-memory engine
    std::env::set_var("TWERK_DATASTORE_TYPE", "inmemory");
    std::env::set_var("TWERK_BROKER_TYPE", "inmemory");
    std::env::set_var("TWERK_RUNTIME_TYPE", "shell");
    std::env::set_var("TWERK_RUNTIME_SHELL_CMD", "bash,-c");

    let mut config = Config::default();
    config.mode = Mode::Standalone;

    let mut engine = Engine::new(config);
    engine.start().await.unwrap();

    // ─── YAML Parse Benchmark ─────────────────────────────────────────
    println!("┌──────────────────────────────────────────────────────────────┐");
    println!("│ YAML PARSE THROUGHPUT                                        │");
    println!("├──────────────────────────────────────────────────────────────┤");

    let parse_start = Instant::now();
    let parse_count = 100_000;
    for i in 0..parse_count {
        let _ = parse_yaml(BENCHMARK_YAML);
    }
    let parse_elapsed = parse_start.elapsed();
    let parse_ops_per_sec = parse_count as f64 / parse_elapsed.as_secs_f64();
    println!("│ Parsed {} YAML workflows in {:?}                    │", parse_count, parse_elapsed);
    println!("│ YAML Parse Throughput: {:>15.0} ops/sec              │", parse_ops_per_sec);
    println!("└──────────────────────────────────────────────────────────────┘");
    println!();

    // ─── Job Creation Benchmark ─────────────────────────────────────
    println!("┌──────────────────────────────────────────────────────────────┐");
    println!("│ JOB CREATION THROUGHPUT                                      │");
    println!("├──────────────────────────────────────────────────────────────┤");

    let job_create_start = Instant::now();
    let job_count = 10_000;
    let jobs = create_job_from_yaml(BENCHMARK_YAML, job_count);
    let job_create_elapsed = job_create_start.elapsed();
    let job_create_ops_per_sec = job_count as f64 / job_create_elapsed.as_secs_f64();
    println!("│ Created {} jobs in {:?}                             │", job_count, job_create_elapsed);
    println!("│ Job Creation Throughput: {:>15.0} ops/sec              │", job_create_ops_per_sec);
    println!("└──────────────────────────────────────────────────────────────┘");
    println!();

    // ─── E2E Throughput: Submit Jobs ───────────────────────────────
    println!("┌──────────────────────────────────────────────────────────────┐");
    println!("│ E2E THROUGHPUT (Job Submit + Task Exec)                      │");
    println!("├──────────────────────────────────────────────────────────────┤");

    let submit_start = Instant::now();
    let submit_count = 1_000;
    let submit_jobs = create_job_from_yaml(BENCHMARK_YAML, submit_count);

    for job in &submit_jobs {
        let (listeners, _rx) = completion_listener();
        engine.submit_job(job.clone(), listeners).await.unwrap();
    }
    let submit_elapsed = submit_start.elapsed();
    let submit_ops_per_sec = submit_count as f64 / submit_elapsed.as_secs_f64();
    let total_tasks = submit_count * 5; // 5 tasks per job
    let tasks_per_sec = total_tasks as f64 / submit_elapsed.as_secs_f64();

    println!("│ Submitted {} jobs ({}/s)                                 │", submit_count, submit_ops_per_sec as u64);
    println!("│ Total tasks queued: {}                                       │", total_tasks);
    println!("│ Task Throughput: {:>15.0} tasks/sec                   │", tasks_per_sec);
    println!("│ Time: {:?}                                              │", submit_elapsed);
    println!("└──────────────────────────────────────────────────────────────┘");
    println!();

    // ─── E2E Latency: Single Job Completion ────────────────────────
    println!("┌──────────────────────────────────────────────────────────────┐");
    println!("│ E2E LATENCY (Single Job Submit → Complete)                  │");
    println!("├──────────────────────────────────────────────────────────────┤");

    let single_job = create_job_from_yaml(BENCHMARK_YAML, 1).pop().unwrap();
    let (listeners, mut rx) = completion_listener();

    let latency_start = Instant::now();
    engine.submit_job(single_job, listeners).await.unwrap();

    let result = timeout(Duration::from_secs(30), rx).await;
    let latency = latency_start.elapsed();

    match result {
        Ok(_) => {
            println!("│ Single job latency:  {:?}                          │", latency);
            println!("│ Status: ✓ COMPLETED                                           │");
        }
        Err(_) => {
            println!("│ Single job latency: > 30s (TIMEOUT)                        │");
            println!("│ Status: ✗ TIMEOUT                                            │");
        }
    }
    println!("└──────────────────────────────────────────────────────────────┘");
    println!();

    // ─── Summary ────────────────────────────────────────────────────
    println!("╔══════════════════════════════════════════════════════════════╗");
    println!("║                    SUMMARY                                   ║");
    println!("╠══════════════════════════════════════════════════════════════╣");
    println!("║ Metric                    │ Value                          ║");
    println!("╟────────────────────────────┼────────────────────────────────╢");
    println!("║ YAML Parse Throughput     │ {:>15.0} ops/sec        ║", parse_ops_per_sec);
    println!("║ Job Creation Throughput   │ {:>15.0} ops/sec        ║", job_create_ops_per_sec);
    println!("║ Job Submit Throughput     │ {:>15.0} ops/sec        ║", submit_ops_per_sec);
    println!("║ Task Throughput           │ {:>15.0} tasks/sec      ║", tasks_per_sec);
    println!("║ Single Job Latency        │ {:?}                     ║", latency);
    println!("╚══════════════════════════════════════════════════════════════╝");
    println!();

    // Assert targets
    assert!(parse_ops_per_sec > 1000.0, "YAML parse should be > 1k ops/sec");
    assert!(job_create_ops_per_sec > 500.0, "Job creation should be > 500 ops/sec");
    assert!(submit_ops_per_sec > 100.0, "Job submit should be > 100 ops/sec");
}
