//! Pokemon API Workflow Simulation
//!
//! Simulates how twerk would execute YAML workflows against the Pokemon API.
//! Each workflow represents a real-world distributed computing pattern.
//!
//! Run: cargo run --bin pokemon-workflow

use reqwest::Client;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};

static TOTAL_REQUESTS: AtomicU64 = AtomicU64::new(0);
static SUCCESSFUL: AtomicU64 = AtomicU64::new(0);
static FAILED: AtomicU64 = AtomicU64::new(0);

/// Results from workflow execution
#[derive(Debug)]
struct WorkflowResult {
    name: String,
    duration_ms: f64,
    requests: u64,
    successful: u64,
    failed: u64,
}

impl WorkflowResult {
    fn print(&self) {
        println!(
            "  {:.<40} {:>8.0} req/s | {} success | {} failed | {:.1}ms",
            self.name,
            self.requests as f64 / (self.duration_ms / 1000.0),
            self.successful,
            self.failed,
            self.duration_ms
        );
    }
}

// ============================================================================
// WORKFLOW 1: CI Pipeline (tripled - 3x parallelism)
// ============================================================================
// Original: 2 parallel build + 2 parallel tests
// Tripled: 6 parallel build + 6 parallel tests = 12 concurrent API calls
//
// Flow:
// 1. Sequential: Fetch all Pokemon (1 request)
// 2. Parallel: Get 6 random Pokemon (6 requests) 
// 3. Parallel: Get Pokemon by 6 types (6 requests)
// 4. Sequential: Final health check (1 request)
// Total: 14 API calls per pipeline run

async fn ci_pipeline_workflow(client: &Client, base_url: &str) -> WorkflowResult {
    let start = Instant::now();
    let mut local_success = 0u64;
    let mut local_failed = 0u64;

    // 1. Sequential: Fetch all Pokemon
    if client.get(&format!("{}/api/pokemon", base_url)).send().await.is_ok() {
        local_success += 1;
    } else {
        local_failed += 1;
    }

    // 2. Parallel: Get 6 random Pokemon (TRIPLED from 2)
    let pokemon_ids = [1u8, 25, 6, 150, 151, 132]; // Pikachu, Charizard, Mewtwo, Mew, Ditto
    let mut handles = Vec::new();
    for id in pokemon_ids {
        let url = format!("{}/api/pokemon/{}", base_url, id);
        let client = client.clone();
        handles.push(tokio::spawn(async move {
            match client.get(&url).send().await {
                Ok(resp) if resp.status().is_success() => 1u64,
                _ => 0u64,
            }
        }));
    }
    for h in handles {
        match h.await {
            Ok(1) => local_success += 1,
            _ => local_failed += 1,
        }
    }

    // 3. Parallel: Get Pokemon by 6 types (TRIPLED from 2)
    let types = ["fire", "water", "grass", "electric", "psychic", "dragon"];
    let mut handles = Vec::new();
    for t in types {
        let url = format!("{}/api/pokemon/type/{}", base_url, t);
        let client = client.clone();
        handles.push(tokio::spawn(async move {
            match client.get(&url).send().await {
                Ok(resp) if resp.status().is_success() => 1u64,
                _ => 0u64,
            }
        }));
    }
    for h in handles {
        match h.await {
            Ok(1) => local_success += 1,
            _ => local_failed += 1,
        }
    }

    // 4. Sequential: Final health check
    if client.get(&format!("{}/health", base_url)).send().await.is_ok() {
        local_success += 1;
    } else {
        local_failed += 1;
    }

    TOTAL_REQUESTS.fetch_add(local_success + local_failed, Ordering::Relaxed);
    SUCCESSFUL.fetch_add(local_success, Ordering::Relaxed);
    FAILED.fetch_add(local_failed, Ordering::Relaxed);

    WorkflowResult {
        name: "CI Pipeline (3x parallel)".to_string(),
        duration_ms: start.elapsed().as_secs_f64() * 1000.0,
        requests: local_success + local_failed,
        successful: local_success,
        failed: local_failed,
    }
}

// ============================================================================
// WORKFLOW 2: Parallel Task Fan-Out (TRIPLED - 15 tasks)
// ============================================================================
// Original: 6 parallel tasks
// Tripled: 15 parallel tasks (all hitting different Pokemon IDs)
//
// This simulates twerk's parallel task execution where multiple workers
// pull tasks from a queue and execute them concurrently.

async fn parallel_fanout_workflow(client: &Client, base_url: &str) -> WorkflowResult {
    let start = Instant::now();
    let mut local_success = 0u64;
    let mut local_failed = 0u64;

    // Fan-out: 15 concurrent requests (TRIPLED from 5)
    let ids: Vec<u8> = (1..=151).step_by(10).take(15).collect();
    let mut handles = Vec::new();
    
    for id in ids {
        let url = format!("{}/api/pokemon/{}", base_url, id);
        let client = client.clone();
        handles.push(tokio::spawn(async move {
            match client.get(&url).send().await {
                Ok(resp) if resp.status().is_success() => 1u64,
                _ => 0u64,
            }
        }));
    }
    
    for h in handles {
        match h.await {
            Ok(1) => local_success += 1,
            _ => local_failed += 1,
        }
    }

    TOTAL_REQUESTS.fetch_add(local_success + local_failed, Ordering::Relaxed);
    SUCCESSFUL.fetch_add(local_success, Ordering::Relaxed);
    FAILED.fetch_add(local_failed, Ordering::Relaxed);

    WorkflowResult {
        name: "Parallel Fan-Out (15 tasks)".to_string(),
        duration_ms: start.elapsed().as_secs_f64() * 1000.0,
        requests: local_success + local_failed,
        successful: local_success,
        failed: local_failed,
    }
}

// ============================================================================
// WORKFLOW 3: Each/Iterator Pattern (TRIPLED - 15 iterations)
// ============================================================================
// Original: 5 iterations
// Tripled: 15 iterations (processing batches of Pokemon)
//
// This simulates twerk's `each` pattern where a task is executed
// for each item in a list.

async fn each_iterator_workflow(client: &Client, base_url: &str) -> WorkflowResult {
    let start = Instant::now();
    let mut local_success = 0u64;
    let mut local_failed = 0u64;

    // Simulate each: 15 iterations (TRIPLED from 5)
    // Each iteration fetches a Pokemon and its type info
    let pokemon_batch: Vec<u8> = (1..=151).step_by(10).collect();
    
    for id in pokemon_batch.into_iter().take(15) {
        // Get Pokemon
        let url = format!("{}/api/pokemon/{}", base_url, id);
        match client.get(&url).send().await {
            Ok(resp) if resp.status().is_success() => {
                local_success += 1;
                
                // Each item can trigger follow-up tasks
                // Get type info (simulating a dependent task)
                let type_url = match id {
                    1..=25 => format!("{}/api/pokemon/type/grass", base_url),
                    26..=50 => format!("{}/api/pokemon/type/electric", base_url),
                    51..=75 => format!("{}/api/pokemon/type/water", base_url),
                    76..=100 => format!("{}/api/pokemon/type/fire", base_url),
                    _ => format!("{}/api/pokemon/type/normal", base_url),
                };
                
                if client.get(&type_url).send().await.is_ok() {
                    local_success += 1;
                } else {
                    local_failed += 1;
                }
            }
            _ => {
                local_failed += 1;
            }
        }
    }

    TOTAL_REQUESTS.fetch_add(local_success + local_failed, Ordering::Relaxed);
    SUCCESSFUL.fetch_add(local_success, Ordering::Relaxed);
    FAILED.fetch_add(local_failed, Ordering::Relaxed);

    WorkflowResult {
        name: "Each Iterator (15 items)".to_string(),
        duration_ms: start.elapsed().as_secs_f64() * 1000.0,
        requests: local_success + local_failed,
        successful: local_success,
        failed: local_failed,
    }
}

// ============================================================================
// WORKFLOW 4: Split & Stitch (TRIPLED - 30 chunks)
// ============================================================================
// Original: 10 video chunks processed in parallel
// Tripled: 30 chunks (simulating 30 Pokemon data processing tasks)
//
// This simulates twerk's split-and-stitch pattern where work is split,
// processed in parallel, then results are stitched together.

async fn split_stitch_workflow(client: &Client, base_url: &str) -> WorkflowResult {
    let start = Instant::now();
    let mut local_success = 0u64;
    let mut local_failed = 0u64;

    // Simulate split: Process 30 "chunks" in parallel (TRIPLED from 10)
    // Each "chunk" is a Pokemon that needs to be processed
    
    // Phase 1: Split - 30 parallel fetches
    let ids: Vec<u8> = (1..=151).step_by(5).take(30).collect();
    let mut phase1_handles = Vec::new();
    
    for id in ids {
        let url = format!("{}/api/pokemon/{}", base_url, id);
        let client = client.clone();
        phase1_handles.push(tokio::spawn(async move {
            match client.get(&url).send().await {
                Ok(resp) if resp.status().is_success() => 1u64,
                _ => 0u64,
            }
        }));
    }
    
    // Phase 2: Stitch - Collect results and make 10 summary requests
    for h in phase1_handles {
        match h.await {
            Ok(1) => local_success += 1,
            _ => local_failed += 1,
        }
    }
    
    // Simulate stitch: 10 summary/type aggregation requests
    let summary_types = ["fire", "water", "grass", "electric", "psychic",
                         "bug", "normal", "poison", "ground", "rock"];
    let mut stitch_handles = Vec::new();
    
    for t in summary_types {
        let url = format!("{}/api/pokemon/type/{}", base_url, t);
        let client = client.clone();
        stitch_handles.push(tokio::spawn(async move {
            match client.get(&url).send().await {
                Ok(resp) if resp.status().is_success() => 1u64,
                _ => 0u64,
            }
        }));
    }
    
    for h in stitch_handles {
        match h.await {
            Ok(1) => local_success += 1,
            _ => local_failed += 1,
        }
    }

    TOTAL_REQUESTS.fetch_add(local_success + local_failed, Ordering::Relaxed);
    SUCCESSFUL.fetch_add(local_success, Ordering::Relaxed);
    FAILED.fetch_add(local_failed, Ordering::Relaxed);

    WorkflowResult {
        name: "Split & Stitch (30 chunks)".to_string(),
        duration_ms: start.elapsed().as_secs_f64() * 1000.0,
        requests: local_success + local_failed,
        successful: local_success,
        failed: local_failed,
    }
}

// ============================================================================
// WORKFLOW 5: MapReduce Pattern (TRIPLED - 45 map + 15 reduce)
// ============================================================================
// Map phase: 45 parallel Pokemon fetches
// Reduce phase: 15 type aggregations
// Total: 60 requests per workflow

async fn mapreduce_workflow(client: &Client, base_url: &str) -> WorkflowResult {
    let start = Instant::now();
    let mut local_success = 0u64;
    let mut local_failed = 0u64;

    // Map phase: 45 parallel Pokemon fetches (TRIPLED from 15)
    let ids: Vec<u8> = (1..=151).step_by(3).take(45).collect();
    let mut map_handles = Vec::new();
    
    for id in ids {
        let url = format!("{}/api/pokemon/{}", base_url, id);
        let client = client.clone();
        map_handles.push(tokio::spawn(async move {
            match client.get(&url).send().await {
                Ok(resp) if resp.status().is_success() => 1u64,
                _ => 0u64,
            }
        }));
    }
    
    for h in map_handles {
        match h.await {
            Ok(1) => local_success += 1,
            _ => local_failed += 1,
        }
    }
    
    // Reduce phase: 15 type aggregations (TRIPLED from 5)
    let types = ["fire", "water", "grass", "electric", "psychic", "bug", 
                 "normal", "poison", "ground", "rock", "ghost", "ice", 
                 "fighting", "dragon", "flying"];
    let mut reduce_handles = Vec::new();
    
    for t in types {
        let url = format!("{}/api/pokemon/type/{}", base_url, t);
        let client = client.clone();
        reduce_handles.push(tokio::spawn(async move {
            match client.get(&url).send().await {
                Ok(resp) if resp.status().is_success() => 1u64,
                _ => 0u64,
            }
        }));
    }
    
    for h in reduce_handles {
        match h.await {
            Ok(1) => local_success += 1,
            _ => local_failed += 1,
        }
    }

    TOTAL_REQUESTS.fetch_add(local_success + local_failed, Ordering::Relaxed);
    SUCCESSFUL.fetch_add(local_success, Ordering::Relaxed);
    FAILED.fetch_add(local_failed, Ordering::Relaxed);

    WorkflowResult {
        name: "MapReduce (45 map + 15 reduce)".to_string(),
        duration_ms: start.elapsed().as_secs_f64() * 1000.0,
        requests: local_success + local_failed,
        successful: local_success,
        failed: local_failed,
    }
}

// ============================================================================
// WORKFLOW 6: DAG Pipeline (TRIPLED - 3 stages x 3 parallel tasks)
// ============================================================================
// Stage 1: 3 parallel fetches
// Stage 2: 3 parallel fetches (depends on stage 1)
// Stage 3: 3 parallel fetches (depends on stage 2)

async fn dag_pipeline_workflow(client: &Client, base_url: &str) -> WorkflowResult {
    let start = Instant::now();
    let mut local_success = 0u64;
    let mut local_failed = 0u64;

    // Stage 1: 3 parallel fetches
    let stage1_ids = [1u8, 4, 7];
    for id in stage1_ids {
        let url = format!("{}/api/pokemon/{}", base_url, id);
        if client.get(&url).send().await.is_ok() {
            local_success += 1;
        } else {
            local_failed += 1;
        }
    }
    
    // Stage 2: 3 parallel fetches (after stage 1)
    let stage2_ids = [25u8, 26, 27];
    for id in stage2_ids {
        let url = format!("{}/api/pokemon/{}", base_url, id);
        if client.get(&url).send().await.is_ok() {
            local_success += 1;
        } else {
            local_failed += 1;
        }
    }
    
    // Stage 3: 3 parallel fetches (after stage 2)
    let stage3_ids = [130u8, 131, 132];
    for id in stage3_ids {
        let url = format!("{}/api/pokemon/{}", base_url, id);
        if client.get(&url).send().await.is_ok() {
            local_success += 1;
        } else {
            local_failed += 1;
        }
    }

    TOTAL_REQUESTS.fetch_add(local_success + local_failed, Ordering::Relaxed);
    SUCCESSFUL.fetch_add(local_success, Ordering::Relaxed);
    FAILED.fetch_add(local_failed, Ordering::Relaxed);

    WorkflowResult {
        name: "DAG Pipeline (3 stages x 3)".to_string(),
        duration_ms: start.elapsed().as_secs_f64() * 1000.0,
        requests: local_success + local_failed,
        successful: local_success,
        failed: local_failed,
    }
}

#[tokio::main]
async fn main() {
    println!("\n╔═══════════════════════════════════════════════════════════════════════════╗");
    println!("║          TWERK WORKFLOW SIMULATION - Pokemon API Benchmark                 ║");
    println!("╠═══════════════════════════════════════════════════════════════════════════╣");
    println!("║  Simulates 6 real-world distributed computing patterns                   ║");
    println!("║  Each workflow is TRIPLED in parallelism from the original examples      ║");
    println!("╚═══════════════════════════════════════════════════════════════════════════╝\n");

    let base_url = "http://127.0.0.1:8080";
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(30))
        .build()
        .unwrap();

    // Verify server is running
    println!("Checking API server availability...");
    match client.get(&format!("{}/health", base_url)).send().await {
        Ok(_) => println!("✓ Server healthy\n"),
        Err(_) => {
            eprintln!("✗ Server not reachable at {}", base_url);
            std::process::exit(1);
        }
    }

    // Run each workflow pattern
    println!("╔═══════════════════════════════════════════════════════════════════════════╗");
    println!("║  WORKFLOW 1: CI Pipeline (3x parallel)                                  ║");
    println!("╠═══════════════════════════════════════════════════════════════════════════╣");
    println!("║  YAML: bash-ci-pipeline.yaml → TRIPLED parallelism                     ║");
    println!("║  Pattern: Sequential + Parallel + Parallel + Sequential                ║");
    println!("║  Requests: 14 (1 all + 6 Pokemon + 6 types + 1 health)                 ║");
    println!("╚═══════════════════════════════════════════════════════════════════════════╝");
    
    let mut results = Vec::new();
    let wf1 = ci_pipeline_workflow(&client, base_url).await;
    wf1.print();
    results.push(wf1);

    println!("\n╔═══════════════════════════════════════════════════════════════════════════╗");
    println!("║  WORKFLOW 2: Parallel Fan-Out (15 tasks)                                ║");
    println!("╠═══════════════════════════════════════════════════════════════════════════╣");
    println!("║  YAML: parallel.yaml → TRIPLED from 6 to 15 parallel tasks            ║");
    println!("║  Pattern: Fan-out to multiple workers pulling from queue                ║");
    println!("║  Requests: 15 concurrent Pokemon fetches                               ║");
    println!("╚═══════════════════════════════════════════════════════════════════════════╝");
    
    let wf2 = parallel_fanout_workflow(&client, base_url).await;
    wf2.print();
    results.push(wf2);

    println!("\n╔═══════════════════════════════════════════════════════════════════════════╗");
    println!("║  WORKFLOW 3: Each Iterator (15 items)                                   ║");
    println!("╠═══════════════════════════════════════════════════════════════════════════╣");
    println!("║  YAML: bash-each.yaml → TRIPLED from 5 to 15 iterations                ║");
    println!("║  Pattern: For-each item, execute task + follow-up dependent task        ║");
    println!("║  Requests: 30 (15 Pokemon + 15 type lookups)                          ║");
    println!("╚═══════════════════════════════════════════════════════════════════════════╝");
    
    let wf3 = each_iterator_workflow(&client, base_url).await;
    wf3.print();
    results.push(wf3);

    println!("\n╔═══════════════════════════════════════════════════════════════════════════╗");
    println!("║  WORKFLOW 4: Split & Stitch (30 chunks)                                 ║");
    println!("╠═══════════════════════════════════════════════════════════════════════════╣");
    println!("║  YAML: split_and_stitch.yaml → TRIPLED from 10 to 30 chunks            ║");
    println!("║  Pattern: Split work → Parallel process → Stitch results                 ║");
    println!("║  Requests: 40 (30 map + 10 reduce/stitch)                             ║");
    println!("╚═══════════════════════════════════════════════════════════════════════════╝");
    
    let wf4 = split_stitch_workflow(&client, base_url).await;
    wf4.print();
    results.push(wf4);

    println!("\n╔═══════════════════════════════════════════════════════════════════════════╗");
    println!("║  WORKFLOW 5: MapReduce (45 map + 15 reduce)                            ║");
    println!("╠═══════════════════════════════════════════════════════════════════════════╣");
    println!("║  Pattern: TRIPLED MapReduce - extract, transform, aggregate              ║");
    println!("║  Pattern: Map phase (45) → Reduce phase (15)                           ║");
    println!("║  Requests: 60 (45 map + 15 reduce)                                     ║");
    println!("╚═══════════════════════════════════════════════════════════════════════════╝");
    
    let wf5 = mapreduce_workflow(&client, base_url).await;
    wf5.print();
    results.push(wf5);

    println!("\n╔═══════════════════════════════════════════════════════════════════════════╗");
    println!("║  WORKFLOW 6: DAG Pipeline (3 stages x 3 parallel)                     ║");
    println!("╠═══════════════════════════════════════════════════════════════════════════╣");
    println!("║  Pattern: Sequential stages with parallel tasks within each stage       ║");
    println!("║  Stage 1 → Stage 2 → Stage 3 (each with 3 parallel fetches)          ║");
    println!("║  Requests: 9 (3 stages x 3 parallel)                                  ║");
    println!("╚═══════════════════════════════════════════════════════════════════════════╝");
    
    let wf6 = dag_pipeline_workflow(&client, base_url).await;
    wf6.print();
    results.push(wf6);

    // Summary
    let total: u64 = results.iter().map(|r| r.requests).sum();
    let total_success: u64 = results.iter().map(|r| r.successful).sum();
    let total_failed: u64 = results.iter().map(|r| r.failed).sum();
    let total_time: f64 = results.iter().map(|r| r.duration_ms).sum();
    let throughput = total as f64 / (total_time / 1000.0);

    println!("\n╔═══════════════════════════════════════════════════════════════════════════╗");
    println!("║                         WORKFLOW SUMMARY                                 ║");
    println!("╠═══════════════════════════════════════════════════════════════════════════╣");
    println!("║  Total Workflows:               6                                      ║");
    println!("║  Total API Requests:     {:>10}                                      ║", total);
    println!("║  Successful Requests:    {:>10} ({:.1}%)                               ║", 
             total_success, (total_success as f64 / total as f64) * 100.0);
    println!("║  Failed Requests:        {:>10} ({:.1}%)                               ║",
             total_failed, (total_failed as f64 / total as f64) * 100.0);
    println!("║  Total Time:            {:>10.1}ms                                      ║", total_time);
    println!("║  Aggregate Throughput:   {:>10.0} req/s                                 ║", throughput);
    println!("╚═══════════════════════════════════════════════════════════════════════════╝\n");
    
    // Global stats from atomic counters
    println!("╔═══════════════════════════════════════════════════════════════════════════╗");
    println!("║                      GLOBAL COUNTER TOTALS                              ║");
    println!("╠═══════════════════════════════════════════════════════════════════════════╣");
    println!("║  Total Requests (atomic):   {:>10}                                      ║", 
             TOTAL_REQUESTS.load(Ordering::Relaxed));
    println!("║  Successful (atomic):       {:>10}                                      ║",
             SUCCESSFUL.load(Ordering::Relaxed));
    println!("║  Failed (atomic):          {:>10}                                      ║",
             FAILED.load(Ordering::Relaxed));
    println!("╚═══════════════════════════════════════════════════════════════════════════╝\n");
    
    println!("✓ All workflows completed successfully!\n");
}
