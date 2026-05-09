//! Real workflow benchmark for Twerk engine.
//!
//! Measures end-to-end job submission and completion throughput
//! using the actual Twerk Engine (in-memory broker + datastore).

use std::time::{Duration, Instant};
use twerk_app::engine::{Config, Engine, Mode};
use twerk_core::job::{Job, JobState};
use twerk_core::task::Task;
use uuid::Uuid;

#[derive(Debug, Clone)]
struct BenchResult {
    job_id: String,
    latency_ms: f64,
}

fn make_shell_job(id: String) -> Job {
    Job {
        id: Some(id.parse().expect("valid uuid")),
        name: Some(format!("bench-job-{}", id)),
        state: JobState::Pending,
        tasks: Some(vec![Task {
            name: Some("bench-task".to_string()),
            run: Some("echo 'hello from benchmark'".to_string()),
            ..Default::default()
        }]),
        task_count: 1,
        ..Default::default()
    }
}

#[tokio::main]
async fn main() {
    println!("========================================");
    println!("  TWERK WORKFLOW BENCHMARK");
    println!("========================================");
    println!();

    // Set up in-memory engine
    std::env::set_var("TWERK_DATASTORE_TYPE", "inmemory");
    std::env::set_var("TWERK_BROKER_TYPE", "inmemory");
    std::env::set_var("TWERK_RUNTIME_TYPE", "shell");
    std::env::set_var("TWERK_RUNTIME_SHELL_CMD", "bash,-c");

    let mut config = Config::default();
    config.mode = Mode::Standalone;
    let mut engine = Engine::new(config);

    engine.start().await.expect("engine should start");

    let job_count = 1_000;
    let mut results = Vec::with_capacity(job_count);

    println!("Submitting {} jobs...", job_count);
    let start = Instant::now();

    for i in 0..job_count {
        let job_id = Uuid::new_v4().to_string();
        let job = make_shell_job(job_id.clone());
        let job_start = Instant::now();

        engine
            .submit_job(job, vec![])
            .await
            .expect("job submission should succeed");

        // Poll for completion
        let mut completed = false;
        for _ in 0..100 {
            if let Ok(j) = engine.datastore().get_job_by_id(&job_id).await {
                if j.state == JobState::Completed || j.state == JobState::Failed {
                    let elapsed = job_start.elapsed().as_secs_f64() * 1000.0;
                    results.push(BenchResult { job_id, latency_ms: elapsed });
                    completed = true;
                    break;
                }
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }

        if !completed {
            println!("  Job {} did not complete in time", i);
        }

        if (i + 1) % 100 == 0 {
            println!("  ...{} jobs submitted", i + 1);
        }
    }

    let total_elapsed = start.elapsed();

    // Compute stats
    let mut latencies: Vec<f64> = results.iter().map(|r| r.latency_ms).collect();
    latencies.sort_by(|a, b| a.partial_cmp(b).unwrap());

    let n = latencies.len() as f64;
    let mean = latencies.iter().sum::<f64>() / n;
    let p50 = latencies[(n * 0.5) as usize.min(latencies.len() - 1)];
    let p90 = latencies[(n * 0.9) as usize.min(latencies.len() - 1)];
    let p99 = latencies[(n * 0.99) as usize.min(latencies.len() - 1)];

    let throughput = results.len() as f64 / total_elapsed.as_secs_f64();

    println!();
    println!("Results ({}/{} jobs completed):", results.len(), job_count);
    println!("  Total time: {:.2}s", total_elapsed.as_secs_f64());
    println!("  Throughput: {:.1} jobs/sec", throughput);
    println!("  Mean latency: {:.2}ms", mean);
    println!("  P50 latency: {:.2}ms", p50);
    println!("  P90 latency: {:.2}ms", p90);
    println!("  P99 latency: {:.2}ms", p99);
    println!();

    if throughput >= 50.0 {
        println!("  [PASS] Exceeds 50 jobs/sec target");
    } else {
        println!("  [FAIL] Below 50 jobs/sec target");
    }

    engine.terminate().await.expect("engine should terminate");
}
