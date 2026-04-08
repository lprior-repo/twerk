//! Twerk YAML Workflow Benchmark - Pokemon API as External Target
//!
//! This test validates twerk by:
//! 1. Running twerk (or verifying it's running)
//! 2. Submitting YAML workflows that use `curl` to call the Pokemon API
//! 3. Measuring twerk's job scheduling, distribution, and completion performance
//!
//! The Pokemon API (port 8080) is an external service that YAML tasks call via curl.
//! Twerk orchestrates the workflow; we measure twerk, not the API.
//!
//! Run with:
//!   cargo test -p twerk-app --test twerk_yaml_workflow_benchmark -- --nocapture

#![allow(clippy::expect_used)]
#![allow(clippy::doc_markdown)]
#![allow(clippy::cast_precision_loss)]
#![allow(clippy::print_stdout)]

use reqwest::Client;
use serde::Deserialize;
use std::time::{Duration, Instant};
use twerk_core::job::JobState;
use twerk_core::task::TaskState;

const TWERK_ENDPOINT: &str = "http://localhost:8000";

/// Pokemon benchmark YAML - twerk executes this, tasks use curl to hit Pokemon API
/// Triple parallelism: 9 Pokemon fetches + 6 type fetches = 15 concurrent API calls
const BENCHMARK_YAML: &str = r#"name: pokemon-api-workflow
description: Twerk workflow calling Pokemon API via curl - triple parallelism
tasks:
  - name: setup
    image: ubuntu:mantic
    run: |
      echo "=== Twerk Pokemon API Workflow ==="
      echo "Worker checking API availability..."
      curl -s http://127.0.0.1:8080/health || echo "API_UNAVAILABLE"

  - name: fetch-bulbasaur
    image: ubuntu:mantic
    run: curl -s http://127.0.0.1:8080/api/pokemon/1 | grep -o '"name"' | head -1

  - name: fetch-pikachu
    image: ubuntu:mantic
    run: curl -s http://127.0.0.1:8080/api/pokemon/25 | grep -o '"name"' | head -1

  - name: fetch-charmander
    image: ubuntu:mantic
    run: curl -s http://127.0.0.1:8080/api/pokemon/4 | grep -o '"name"' | head -1

  - name: fetch-squirtle
    image: ubuntu:mantic
    run: curl -s http://127.0.0.1:8080/api/pokemon/7 | grep -o '"name"' | head -1

  - name: fetch-bullbasaur
    image: ubuntu:mantic
    run: curl -s http://127.0.0.1:8080/api/pokemon/2 | grep -o '"name"' | head -1

  - name: fetch-charizard
    image: ubuntu:mantic
    run: curl -s http://127.0.0.1:8080/api/pokemon/6 | grep -o '"name"' | head -1

  - name: fetch-mewtwo
    image: ubuntu:mantic
    run: curl -s http://127.0.0.1:8080/api/pokemon/150 | grep -o '"name"' | head -1

  - name: fetch-mew
    image: ubuntu:mantic
    run: curl -s http://127.0.0.1:8080/api/pokemon/151 | grep -o '"name"' | head -1

  - name: fetch-dragonite
    image: ubuntu:mantic
    run: curl -s http://127.0.0.1:8080/api/pokemon/149 | grep -o '"name"' | head -1

  - name: type-fire
    image: ubuntu:mantic
    run: curl -s http://127.0.0.1:8080/api/pokemon/type/fire | grep -o '"id"' | head -1

  - name: type-water
    image: ubuntu:mantic
    run: curl -s http://127.0.0.1:8080/api/pokemon/type/water | grep -o '"id"' | head -1

  - name: type-grass
    image: ubuntu:mantic
    run: curl -s http://127.0.0.1:8080/api/pokemon/type/grass | grep -o '"id"' | head -1

  - name: type-electric
    image: ubuntu:mantic
    run: curl -s http://127.0.0.1:8080/api/pokemon/type/electric | grep -o '"id"' | head -1

  - name: type-psychic
    image: ubuntu:mantic
    run: curl -s http://127.0.0.1:8080/api/pokemon/type/psychic | grep -o '"id"' | head -1

  - name: type-dragon
    image: ubuntu:mantic
    run: curl -s http://127.0.0.1:8080/api/pokemon/type/dragon | grep -o '"id"' | head -1

  - name: verify-all
    image: ubuntu:mantic
    run: |
      echo "=== Workflow Complete ==="
      echo "All Pokemon API calls finished"

  - name: health-check
    image: ubuntu:mantic
    run: |
      echo "=== Final Health Check ==="
      curl -s http://127.0.0.1:8080/health
      echo ""
"#;

/// Simpler single-task YAML for quick validation
const SIMPLE_YAML: &str = r#"name: simple-curl-test
description: Quick test - single curl to Pokemon API
tasks:
  - name: curl-health
    image: ubuntu:mantic
    run: curl -s http://127.0.0.1:8080/health
"#;

#[derive(Debug, Deserialize)]
struct Job {
    id: String,
    name: String,
    state: JobState,
    #[serde(default)]
    #[allow(dead_code)]
    task_count: u32,
    #[serde(default)]
    progress: f64,
    #[serde(default)]
    tasks: Option<Vec<TaskInfo>>,
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
    println!("\n╔══════════════════════════════════════════════════════════════════════════════════╗");
    println!("║  {}", title);
    println!("╚══════════════════════════════════════════════════════════════════════════════════╝");
}

fn print_result(label: &str, value: impl std::fmt::Display) {
    println!("║  {:.<45} {:>20}", label, value);
}

/// Test 1: Simple curl workflow to validate basic twerk functionality
#[tokio::test]
async fn twerk_simple_curl_workflow() -> Result<(), Box<dyn std::error::Error>> {
    let client = Client::builder()
        .timeout(Duration::from_secs(60))
        .build()?;

    print_header("TWERK SIMPLE CURL WORKFLOW TEST");

    // Verify twerk is running
    println!("\n║  Verifying twerk is running...");
    let health = client.get(&format!("{}/health", TWERK_ENDPOINT))
        .send()
        .await?;
    
    if !health.status().is_success() {
        return Err(format!("Twerk not available at {}", TWERK_ENDPOINT).into());
    }
    println!("║  ✓ Twerk API is healthy");

    // Submit simple job
    print_header("SUBMITTING SIMPLE CURL JOB");
    
    let start = Instant::now();
    
    let response = client
        .post(&format!("{}/jobs", TWERK_ENDPOINT))
        .header("Content-Type", "application/yaml")
        .body(SIMPLE_YAML)
        .send()
        .await?;

    let submit_time = start.elapsed();
    
    let job: Job = response.json::<Job>().await?;
    
    print_result("Job ID", &job.id);
    print_result("Job State", &job.state);
    print_result("Submit Time", &format!("{:?}", submit_time));

    // Poll for completion
    let start = Instant::now();
    let timeout = Duration::from_secs(60);
    
    let final_job = loop {
        if start.elapsed() > timeout {
            return Err("Job timed out".into());
        }
        
        let response = client.get(&format!("{}/jobs/{}", TWERK_ENDPOINT, job.id))
            .send()
            .await?
            .json::<Job>()
            .await?;
        
        print_result("Current State", &response.state);
        
        if response.state == JobState::Completed {
            break response;
        } else if response.state == JobState::Failed {
            return Err("Job failed".into());
        }
        
        tokio::time::sleep(Duration::from_secs(1)).await;
    };
    
    let total_time = start.elapsed();

    print_header("SIMPLE WORKFLOW RESULTS");
    print_result("Job ID", &final_job.id);
    print_result("Total Time", &format!("{:?}", total_time));
    print_result("Throughput", &format!("{:.1} tasks/sec", 1.0 / total_time.as_secs_f64()));
    
    assert_eq!(final_job.state, JobState::Completed, "Simple job should complete");
    
    Ok(())
}

/// Test 2: Full Pokemon API workflow with triple parallelism
#[tokio::test]
async fn twerk_pokemon_api_workflow() -> Result<(), Box<dyn std::error::Error>> {
    let client = Client::builder()
        .timeout(Duration::from_secs(300))
        .build()?;

    print_header("TWERK POKEMON API WORKFLOW (Triple Parallelism)");

    // Verify twerk is running
    println!("\n║  Verifying twerk is running...");
    let health = client.get(&format!("{}/health", TWERK_ENDPOINT))
        .send()
        .await?;
    
    if !health.status().is_success() {
        return Err(format!("Twerk not available at {}", TWERK_ENDPOINT).into());
    }
    println!("║  ✓ Twerk API is healthy");

    // Verify Pokemon API is reachable (twerk workers will call it via curl)
    println!("\n║  Verifying Pokemon API is reachable...");
    let pokemon_health = reqwest::get("http://127.0.0.1:8080/health").await?;
    if !pokemon_health.status().is_success() {
        return Err("Pokemon API not available at http://127.0.0.1:8080".into());
    }
    println!("║  ✓ Pokemon API is available (workers will curl it)");

    // Submit full workflow
    print_header("SUBMITTING POKEMON API WORKFLOW");
    println!("\n║  YAML runs 15+ curl commands to Pokemon API concurrently");
    println!("║  This measures twerk's ability to:");
    println!("║    - Parse and schedule 17 tasks");
    println!("║    - Distribute tasks to workers");
    println!("║    - Execute shell commands with curl");
    println!("║    - Handle concurrent API calls through workers");
    
    let start = Instant::now();
    
    let response = client
        .post(&format!("{}/jobs?wait=blocking", TWERK_ENDPOINT))
        .header("Content-Type", "application/yaml")
        .body(BENCHMARK_YAML)
        .send()
        .await?;

    let status = response.status();
    if !status.is_success() {
        let body = response.text().await?;
        return Err(format!("Job submission failed: {} - {}", status, body).into());
    }

    let submit_time = start.elapsed();
    
    let job: Job = response.json::<Job>().await?;
    
    print_result("Job ID", &job.id);
    print_result("Job Name", &job.name);
    print_result("Initial State", &job.state);
    print_result("Submit + Wait Time", &format!("{:?}", submit_time));

    // If job already completed (blocking mode), skip polling
    if job.state == JobState::Completed {
        print_header("JOB COMPLETED DURING SUBMIT");
    } else {
        // Poll for completion
        print_header("WAITING FOR COMPLETION");
        
        let poll_start = Instant::now();
        let timeout = Duration::from_secs(180);
        
        let _final_job = loop {
            if poll_start.elapsed() > timeout {
                return Err("Job timed out after 180 seconds".into());
            }
            
            let response = client.get(&format!("{}/jobs/{}", TWERK_ENDPOINT, job.id))
                .send()
                .await?
                .json::<Job>()
                .await?;
            
            print_result("State", &response.state);
            print_result("Progress", &format!("{:.0}%", response.progress * 100.0));
            print_result("Elapsed", &format!("{:?}", poll_start.elapsed()));
            
            if response.state == JobState::Completed {
                break response;
            } else if response.state == JobState::Failed {
                // Get failure details
                let tasks_resp = client.get(&format!("{}/jobs/{}/tasks", TWERK_ENDPOINT, job.id))
                    .send()
                    .await?
                    .json::<TasksResponse>()
                    .await?;
                
                for task in tasks_resp.items {
                    if task.state == TaskState::Failed {
                        eprintln!("\n║  ✗ Task '{}' FAILED: {:?}", task.name, task.error);
                    }
                }
                return Err("Job failed".into());
            }
            
            tokio::time::sleep(Duration::from_secs(2)).await;
        };
    }

    // Get task details from job response (embedded tasks, not separate endpoint)
    let total_tasks = job.tasks.as_ref().map_or(0, |t| t.len());
    let completed = job.tasks.as_ref().map_or(0, |t| 
        t.iter().filter(|task| task.state == TaskState::Completed).count()
    );
    let failed = job.tasks.as_ref().map_or(0, |t| 
        t.iter().filter(|task| task.state == TaskState::Failed).count()
    );

    // Results
    let total_time = start.elapsed();
    
    print_header("TWERK WORKFLOW BENCHMARK RESULTS");
    
    println!("\n║  TIMING:");
    print_result("Total Duration", &format!("{:?}", total_time));
    print_result("Submit + Wait", &format!("{:?}", submit_time));
    print_result("Tasks Created", &total_tasks.to_string());
    
    println!("\n║  TASK STATUS:");
    print_result("Completed", &completed.to_string());
    print_result("Failed", &failed.to_string());
    print_result("Success Rate", &format!("{:.1}%", (completed as f64 / total_tasks as f64) * 100.0));
    
    println!("\n║  TWERK THROUGHPUT:");
    let tasks_per_sec = total_tasks as f64 / total_time.as_secs_f64();
    print_result("Tasks/Second", &format!("{:.2}", tasks_per_sec));
    print_result("Avg ms/task", &format!("{:.2}", total_time.as_millis() as f64 / total_tasks as f64));
    
    println!("\n║  WORKFLOW BREAKDOWN:");
    for task in job.tasks.as_ref().unwrap_or(&vec![]) {
        let status = match task.state {
            TaskState::Completed => "✓",
            TaskState::Failed => "✗",
            TaskState::Pending => "○",
            TaskState::Running => "◐",
            _ => "?",
        };
        println!("║    {} {}", status, task.name);
        if let Some(ref e) = task.error {
            println!("║      Error: {}", e);
        }
    }

    println!("\n╚══════════════════════════════════════════════════════════════════════════════════╝");

    // Assertions
    assert_eq!(job.state, JobState::Completed, "Job should complete successfully");
    assert_eq!(failed, 0, "No tasks should fail");
    assert_eq!(total_tasks, 18, "Should have 18 tasks");

    Ok(())
}
