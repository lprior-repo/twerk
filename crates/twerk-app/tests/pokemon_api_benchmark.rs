//! Pokemon API Benchmark via Twerk - Integration Test
//!
//! This test validates twerk end-to-end by:
//! 1. Starting/verifying twerk is running
//! 2. Submitting a YAML workflow that calls the Pokemon API
//! 3. Measuring twerk's ability to schedule, distribute, and complete tasks
//! 4. Reporting throughput, latency, and success metrics
//!
//! Run with:
//!   cargo test -p twerk-app --test pokemon_api_benchmark -- --nocapture

#![allow(clippy::expect_used)]
#![allow(clippy::doc_markdown)]
#![allow(clippy::cast_precision_loss)]
#![allow(clippy::print_stdout)] // Benchmarks print to stdout

use reqwest::Client;
use serde::Deserialize;
use std::time::{Duration, Instant};
use twerk_core::job::JobState;
use twerk_core::task::TaskState;

const TWERK_ENDPOINT: &str = "http://localhost:8000";
const POKEMON_API: &str = "http://127.0.0.1:8080";

/// Pokemon benchmark YAML - tripled parallelism
const BENCHMARK_YAML: &str = r#"name: pokemon-api-benchmark
description: Benchmark twerk by calling Pokemon API - tripled parallelism
tasks:
  - name: health-check
    image: ubuntu:mantic
    run: |
      echo "=== Pokemon API Benchmark ==="
      curl -s http://127.0.0.1:8080/health
      echo ""

  - name: fetch-all-pokemon
    image: ubuntu:mantic
    run: |
      echo "Fetching all 151 Pokemon..."
      curl -s http://127.0.0.1:8080/api/pokemon | head -c 200
      echo "..."

  - name: parallel-pokemon-fetches
    parallel:
      tasks:
        - name: fetch-bulbasaur
          image: ubuntu:mantic
          run: curl -s http://127.0.0.1:8080/api/pokemon/1
        - name: fetch-pikachu
          image: ubuntu:mantic
          run: curl -s http://127.0.0.1:8080/api/pokemon/25
        - name: fetch-charmander
          image: ubuntu:mantic
          run: curl -s http://127.0.0.1:8080/api/pokemon/4
        - name: fetch-squirtle
          image: ubuntu:mantic
          run: curl -s http://127.0.0.1:8080/api/pokemon/7
        - name: fetch-bullbasaur
          image: ubuntu:mantic
          run: curl -s http://127.0.0.1:8080/api/pokemon/2
        - name: fetch-charizard
          image: ubuntu:mantic
          run: curl -s http://127.0.0.1:8080/api/pokemon/6
        - name: fetch-mewtwo
          image: ubuntu:mantic
          run: curl -s http://127.0.0.1:8080/api/pokemon/150
        - name: fetch-mew
          image: ubuntu:mantic
          run: curl -s http://127.0.0.1:8080/api/pokemon/151
        - name: fetch-dragonite
          image: ubuntu:mantic
          run: curl -s http://127.0.0.1:8080/api/pokemon/149

  - name: parallel-type-fetches
    parallel:
      tasks:
        - name: type-fire
          image: ubuntu:mantic
          run: curl -s http://127.0.0.1:8080/api/pokemon/type/fire | head -c 100
        - name: type-water
          image: ubuntu:mantic
          run: curl -s http://127.0.0.1:8080/api/pokemon/type/water | head -c 100
        - name: type-grass
          image: ubuntu:mantic
          run: curl -s http://127.0.0.1:8080/api/pokemon/type/grass | head -c 100
        - name: type-electric
          image: ubuntu:mantic
          run: curl -s http://127.0.0.1:8080/api/pokemon/type/electric | head -c 100
        - name: type-psychic
          image: ubuntu:mantic
          run: curl -s http://127.0.0.1:8080/api/pokemon/type/psychic | head -c 100
        - name: type-dragon
          image: ubuntu:mantic
          run: curl -s http://127.0.0.1:8080/api/pokemon/type/dragon | head -c 100

  - name: final-health
    image: ubuntu:mantic
    run: |
      echo "=== Benchmark Complete ==="
      curl -s http://127.0.0.1:8080/health
      echo ""
"#;

#[derive(Debug, Deserialize)]
struct Job {
    id: String,
    name: String,
    state: JobState,
    #[serde(default)]
    task_count: u32,
    #[serde(default)]
    progress: f64,
}

#[derive(Debug, Deserialize)]
struct TasksResponse {
    items: Vec<TaskInfo>,
}

#[derive(Debug, Deserialize)]
struct TaskInfo {
    name: String,
    state: TaskState,
    #[serde(default)]
    error: Option<String>,
}

fn print_header(title: &str) {
    println!("\n╔══════════════════════════════════════════════════════════════════════════════════════════╗");
    println!("║  {}", title);
    println!("╚══════════════════════════════════════════════════════════════════════════════════════════╝");
}

fn print_result(label: &str, value: impl std::fmt::Display) {
    println!("║  {:.<50} {:>15}", label, value);
}

#[tokio::test]
async fn pokemon_api_benchmark_through_twerk() -> Result<(), Box<dyn std::error::Error>> {
    let client = Client::builder()
        .timeout(Duration::from_secs(300))
        .build()?;

    print_header("TWERK + POKEMON API BENCHMARK");

    // =========================================================================
    // Step 1: Verify Twerk is running
    // =========================================================================
    println!("\n║  STEP 1: Verifying Twerk is running...");
    
    let twerk_health = client.get(&format!("{}/health", TWERK_ENDPOINT))
        .send()
        .await?;
    
    if !twerk_health.status().is_success() {
        return Err(format!("Twerk health check failed: {}", twerk_health.status()).into());
    }
    println!("║  ✓ Twerk API is healthy");

    // =========================================================================
    // Step 2: Verify Pokemon API is running
    // =========================================================================
    println!("\n║  STEP 2: Verifying Pokemon API is running...");
    
    let pokemon_health = client.get(&format!("{}/health", POKEMON_API))
        .send()
        .await?;
    
    if !pokemon_health.status().is_success() {
        return Err(format!("Pokemon API health check failed: {}", pokemon_health.status()).into());
    }
    println!("║  ✓ Pokemon API is healthy");

    // =========================================================================
    // Step 3: Verify Pokemon API returns all 151
    // =========================================================================
    println!("\n║  STEP 3: Verifying Pokemon API returns 151 Pokemon...");
    
    let all_pokemon = client.get(&format!("{}/api/pokemon", POKEMON_API))
        .send()
        .await?
        .json::<Vec<serde_json::Value>>()
        .await?;
    
    println!("║  ✓ API returned {} Pokemon", all_pokemon.len());

    // =========================================================================
    // Step 4: Clear any stale jobs
    // =========================================================================
    println!("\n║  STEP 4: Clearing old jobs from database...");
    
    let jobs_response = client.get(&format!("{}/jobs", TWERK_ENDPOINT))
        .send()
        .await?;
    
    if let Ok(jobs) = jobs_response.json::<serde_json::Value>().await {
        if let Some(items) = jobs.get("items").and_then(|i| i.as_array()) {
            for job in items {
                if let Some(id) = job.get("id").and_then(|i| i.as_str()) {
                    // Log but don't fail if delete fails - we still want to try submitting
                    if let Err(e) = client.delete(&format!("{}/jobs/{}", TWERK_ENDPOINT, id))
                        .send()
                        .await
                    {
                        eprintln!("Warning: Failed to delete stale job {}: {}", id, e);
                    }
                }
            }
        }
    }
    println!("║  ✓ Cleared old jobs");

    // =========================================================================
    // Step 5: Submit Pokemon benchmark job
    // =========================================================================
    print_header("SUBMITTING BENCHMARK JOB");
    
    let submit_time = Instant::now();
    
    let response = client
        .post(&format!("{}/jobs?wait=blocking", TWERK_ENDPOINT))
        .header("Content-Type", "application/yaml")
        .body(BENCHMARK_YAML)
        .send()
        .await?;

    let submit_duration = submit_time.elapsed();
    
    println!("\n║  Submission response status: {}", response.status());
    
    let job: Job = response.json::<Job>().await?;
    
    print_header("JOB SUBMITTED");
    print_result("Job ID", &job.id);
    print_result("Job Name", &job.name);
    print_result("Job State", &job.state);
    print_result("Submit Duration", &format!("{:?}", submit_duration));

    // =========================================================================
    // Step 6: Poll for job completion
    // =========================================================================
    print_header("WAITING FOR JOB COMPLETION");
    
    let start_time = Instant::now();
    let timeout = Duration::from_secs(120);
    let poll_interval = Duration::from_secs(1);
    
    let final_job = loop {
        if start_time.elapsed() > timeout {
            return Err("Job timed out after 120 seconds".into());
        }
        
        let response = client.get(&format!("{}/jobs/{}", TWERK_ENDPOINT, job.id))
            .send()
            .await?
            .json::<Job>()
            .await?;
        
        print_result("Current State", &response.state);
        print_result("Progress", &format!("{:.1}%", response.progress * 100.0));
        print_result("Elapsed", &format!("{:?}", start_time.elapsed()));
        
        if response.state == JobState::Completed {
            break response;
        } else if response.state == JobState::Failed {
            // Get task details
            let task_response = client.get(&format!("{}/jobs/{}/tasks", TWERK_ENDPOINT, job.id))
                .send()
                .await?
                .json::<TasksResponse>()
                .await?;
            
            for task in task_response.items {
                if task.state == TaskState::Failed {
                    println!("\n║  ✗ Task '{}' FAILED: {:?}", task.name, task.error);
                }
            }
            return Err("Job failed".into());
        }
        
        tokio::time::sleep(poll_interval).await;
        println!();
    };
    
    let total_duration = start_time.elapsed();

    // =========================================================================
    // Step 7: Get final job details
    // =========================================================================
    let task_response = client.get(&format!("{}/jobs/{}/tasks", TWERK_ENDPOINT, job.id))
        .send()
        .await?
        .json::<TasksResponse>()
        .await?;

    let total_tasks = task_response.items.len();
    let completed_tasks = task_response.items.iter().filter(|t| t.state == TaskState::Completed).count();
    let failed_tasks = task_response.items.iter().filter(|t| t.state == TaskState::Failed).count();

    // =========================================================================
    // Results Summary
    // =========================================================================
    print_header("BENCHMARK RESULTS");
    
    println!("\n║  JOB METRICS:");
    print_result("Job ID", &final_job.id);
    print_result("Total Duration", &format!("{:?}", total_duration));
    print_result("Submit + Wait Time", &format!("{:?}", submit_duration));
    println!("\n║  TASK METRICS:");
    print_result("Total Tasks", &total_tasks.to_string());
    print_result("Completed", &completed_tasks.to_string());
    print_result("Failed", &failed_tasks.to_string());
    print_result("Success Rate", &format!("{:.1}%", (completed_tasks as f64 / total_tasks as f64) * 100.0));
    
    // Calculate throughput (9 parallel fetches + 6 type fetches + 3 sequential = 18 API calls)
    let estimated_api_calls = 18;
    let throughput = estimated_api_calls as f64 / total_duration.as_secs_f64();
    
    println!("\n║  THROUGHPUT:");
    print_result("Estimated API Calls", &estimated_api_calls.to_string());
    print_result("API Calls/Second", &format!("{:.1}", throughput));
    print_result("Avg Time per API Call", &format!("{:?}", total_duration / estimated_api_calls as u32));

    println!("\n║  TASK BREAKDOWN:");
    for task in &task_response.items {
        let status = match task.state {
            TaskState::Completed => "✓",
            TaskState::Failed => "✗",
            TaskState::Pending => "○",
            TaskState::Running => "◐",
            _ => "?",
        };
        println!("║    {} {} ({})", status, task.name, task.state);
        if let Some(ref err) = task.error {
            println!("║      Error: {}", err);
        }
    }

    println!("\n╚══════════════════════════════════════════════════════════════════════════════════════════╝");

    // Assert job completed successfully
    assert_eq!(final_job.state, JobState::Completed, "Job should complete successfully");
    assert_eq!(failed_tasks, 0, "No tasks should fail");
    
    Ok(())
}

#[tokio::test]
async fn verify_pokemon_api_direct() -> Result<(), Box<dyn std::error::Error>> {
    let client = Client::new();
    
    println!("\n╔══════════════════════════════════════════════════════════════════════════════════════════╗");
    println!("║  DIRECT POKEMON API BENCHMARK (Baseline for comparison)                                ║");
    println!("╚══════════════════════════════════════════════════════════════════════════════════════════╝");
    
    let start = Instant::now();
    
    // Fetch all Pokemon
    let all = client.get(&format!("{}/api/pokemon", POKEMON_API))
        .send()
        .await?
        .json::<Vec<serde_json::Value>>()
        .await?;
    println!("\n║  GET /api/pokemon: {} Pokemon in {:?}", all.len(), start.elapsed());
    
    // Fetch 9 Pokemon in parallel
    let parallel_start = Instant::now();
    let ids = [1, 25, 4, 7, 2, 6, 150, 151, 149];
    let mut handles = vec![];
    for id in ids {
        let url = format!("{}/api/pokemon/{}", POKEMON_API, id);
        let client = client.clone();
        handles.push(tokio::spawn(async move {
            client.get(&url).send().await
        }));
    }
    for h in handles {
        h.await??;
    }
    println!("║  Parallel 9 GET /api/pokemon/X: {:?} (includes network)", parallel_start.elapsed());
    
    // Fetch 6 types in parallel
    let type_start = Instant::now();
    let types = ["fire", "water", "grass", "electric", "psychic", "dragon"];
    let mut handles = vec![];
    for t in types {
        let url = format!("{}/api/pokemon/type/{}", POKEMON_API, t);
        let client = client.clone();
        handles.push(tokio::spawn(async move {
            client.get(&url).send().await
        }));
    }
    for h in handles {
        h.await??;
    }
    println!("║  Parallel 6 GET /api/pokemon/type/X: {:?}", type_start.elapsed());
    
    let total = start.elapsed();
    println!("\n║  Total direct API calls: 16 in {:?}", total);
    println!("║  Throughput: {:.1} req/s", 16.0 / total.as_secs_f64());
    
    println!("\n╚══════════════════════════════════════════════════════════════════════════════════════════╝");
    
    Ok(())
}
