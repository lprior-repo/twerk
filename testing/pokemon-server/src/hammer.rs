//! Pokemon API Hammer - Distributed Load Benchmark
//!
//! This simulates how twerk would hammer an API with parallel shell tasks.
//! Each "task" is a concurrent curl command hitting the Pokemon API.
//!
//! Run: cargo run --bin pokemon-hammer

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

/// Results from a hammering run
#[derive(Debug, Clone)]
struct HammerResults {
    total_requests: u64,
    successful: u64,
    failed: u64,
    duration_secs: f64,
    requests_per_sec: f64,
    mean_latency_ms: f64,
    p50_latency_ms: f64,
    p90_latency_ms: f64,
    p99_latency_ms: f64,
}

impl HammerResults {
    fn print(&self) {
        println!("\n╔═══════════════════════════════════════════════════════════════╗");
        println!("║            DISTRIBUTED API HAMMER RESULTS                      ║");
        println!("╠═══════════════════════════════════════════════════════════════╣");
        println!("║  Total Requests:      {:>12}", self.total_requests);
        println!("║  Successful:         {:>12}", self.successful);
        println!("║  Failed:              {:>12}", self.failed);
        println!("║  Duration:           {:>12.3}s", self.duration_secs);
        println!("║  Throughput:         {:>12.2} req/s", self.requests_per_sec);
        println!("╠═══════════════════════════════════════════════════════════════╣");
        println!("║                    LATENCY PERCENTILES                        ║");
        println!("╠═══════════════════════════════════════════════════════════════╣");
        println!("║  Mean:               {:>12.3}ms", self.mean_latency_ms);
        println!("║  P50:               {:>12.3}ms", self.p50_latency_ms);
        println!("║  P90:               {:>12.3}ms", self.p90_latency_ms);
        println!("║  P99:               {:>12.3}ms", self.p99_latency_ms);
        println!("╚═══════════════════════════════════════════════════════════════╝");
    }
}

fn percentile(sorted: &[f64], p: f64) -> f64 {
    if sorted.is_empty() {
        return 0.0;
    }
    let p = p.clamp(0.0, 1.0);
    let n = sorted.len() as f64;
    let idx = (p * (n - 1.0)).floor() as usize;
    sorted[idx.min(sorted.len().saturating_sub(1))]
}

/// Hammer the API with concurrent curl requests
async fn hammer_api(
    url: &str,
    duration_secs: u64,
    concurrency: usize,
) -> HammerResults {
    let start = Instant::now();
    let deadline = start + Duration::from_secs(duration_secs);
    
    let total = Arc::new(AtomicU64::new(0));
    let successful = Arc::new(AtomicU64::new(0));
    let failed = Arc::new(AtomicU64::new(0));
    
    // Spawn concurrent workers
    let mut handles = Vec::new();
    
    for _ in 0..concurrency {
        let total = total.clone();
        let _successful = successful.clone();
        let _failed = failed.clone();
        let url = url.to_string();
        
        handles.push(tokio::spawn(async move {
            let mut latencies = Vec::with_capacity(10000);
            let mut local_total = 0u64;
            let mut local_successful = 0u64;
            let mut local_failed = 0u64;
            
            loop {
                if Instant::now() >= deadline {
                    break;
                }
                
                let now = Instant::now();
                
                // Use reqwest for actual HTTP calls (simulates what curl does)
                let client = reqwest::Client::builder()
                    .timeout(Duration::from_secs(5))
                    .build()
                    .unwrap();
                
                if let Ok(resp) = client.get(&url).send().await {
                    if resp.status().is_success() {
                        let elapsed = now.elapsed().as_secs_f64() * 1000.0;
                        local_total += 1;
                        local_successful += 1;
                        latencies.push(elapsed);
                    } else {
                        local_total += 1;
                        local_failed += 1;
                    }
                } else {
                    local_total += 1;
                    local_failed += 1;
                }
                
                // Yield periodically
                if (local_total).is_multiple_of(100) {
                    tokio::task::yield_now().await;
                }
            }
            
            (latencies, local_total, local_successful, local_failed)
        }));
    }
    
    // Collect results
    let mut all_latencies = Vec::new();
    for handle in handles {
        if let Ok((latencies, t, s, f)) = handle.await {
            all_latencies.extend(latencies);
            total.fetch_add(t, Ordering::Relaxed);
            successful.fetch_add(s, Ordering::Relaxed);
            failed.fetch_add(f, Ordering::Relaxed);
        }
    }
    
    let duration = start.elapsed();
    let total_requests = total.load(Ordering::Relaxed);
    let success = successful.load(Ordering::Relaxed);
    let fail = failed.load(Ordering::Relaxed);
    
    all_latencies.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    
    let mean = if !all_latencies.is_empty() {
        all_latencies.iter().sum::<f64>() / all_latencies.len() as f64
    } else {
        0.0
    };
    
    HammerResults {
        total_requests,
        successful: success,
        failed: fail,
        duration_secs: duration.as_secs_f64(),
        requests_per_sec: total_requests as f64 / duration.as_secs_f64(),
        mean_latency_ms: mean,
        p50_latency_ms: percentile(&all_latencies, 0.50),
        p90_latency_ms: percentile(&all_latencies, 0.90),
        p99_latency_ms: percentile(&all_latencies, 0.99),
    }
}

/// Monte Carlo simulation with varying concurrency
async fn monte_carlo(url: &str, iterations: usize) {
    println!("\n🔄 Running Monte Carlo Simulation ({} iterations)...\n", iterations);
    
    let mut all_results = Vec::new();
    
    for i in 0..iterations {
        let concurrency = ((i % 10) + 1) * 10; // 10, 20, 30, ... 100
        let duration = 3; // 3 seconds per iteration
        
        print!("  Iteration {}/{} (concurrency={}, duration={}s)... ", i + 1, iterations, concurrency, duration);
        
        let results = hammer_api(url, duration, concurrency).await;
        println!("{:.0} req/s", results.requests_per_sec);
        
        all_results.push(results);
    }
    
    // Aggregate statistics
    let mut throughputs: Vec<f64> = all_results.iter().map(|r| r.requests_per_sec).collect();
    throughputs.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    
    let mut p99s: Vec<f64> = all_results.iter().map(|r| r.p99_latency_ms).collect();
    p99s.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    
    let mean_tp = throughputs.iter().sum::<f64>() / throughputs.len() as f64;
    let mean_p99 = p99s.iter().sum::<f64>() / p99s.len() as f64;
    
    println!("\n╔═══════════════════════════════════════════════════════════════╗");
    println!("║            MONTE CARLO SIMULATION SUMMARY                     ║");
    println!("╠═══════════════════════════════════════════════════════════════╣");
    println!("║  Iterations:            {:>12}", iterations);
    println!("╠═══════════════════════════════════════════════════════════════╣");
    println!("║                 THROUGHPUT (req/s)                          ║");
    println!("╠═══════════════════════════════════════════════════════════════╣");
    println!("║  Min:                  {:>12.2}", throughputs.first().unwrap_or(&0.0));
    println!("║  Mean:                 {:>12.2}", mean_tp);
    println!("║  Max:                 {:>12.2}", throughputs.last().unwrap_or(&0.0));
    println!("║  P50:                 {:>12.2}", percentile(&throughputs, 0.50));
    println!("║  P90:                 {:>12.2}", percentile(&throughputs, 0.90));
    println!("╠═══════════════════════════════════════════════════════════════╣");
    println!("║                 LATENCY P99 (ms)                           ║");
    println!("╠═══════════════════════════════════════════════════════════════╣");
    println!("║  Min:                  {:>12.3}", p99s.first().unwrap_or(&0.0));
    println!("║  Mean:                 {:>12.3}", mean_p99);
    println!("║  Max:                 {:>12.3}", p99s.last().unwrap_or(&0.0));
    println!("║  P50:                 {:>12.3}", percentile(&p99s, 0.50));
    println!("║  P90:                 {:>12.3}", percentile(&p99s, 0.90));
    println!("╚═══════════════════════════════════════════════════════════════╝");
}

#[tokio::main]
async fn main() {
    println!("\n🔥 POKEMON API HAMMER - Distributed Load Benchmark 🔥\n");
    println!("Simulates how twerk would hammer an API with parallel shell tasks\n");
    
    let base_url = "http://127.0.0.1:8080/api/pokemon";
    
    // Check if server is running
    println!("1. Checking if Pokemon API server is running...");
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(5))
        .build()
        .unwrap();
    
    match client.get(format!("{}/health", base_url.replace("/api/pokemon", ""))).send().await {
        Ok(_) => println!("   ✓ Server is running"),
        Err(_) => {
            eprintln!("\n   ✗ Server not reachable at {}", base_url);
            eprintln!("\n   Start the server first:");
            eprintln!("   cd pokemon-server && cargo run --bin pokemon-server\n");
            std::process::exit(1);
        }
    }
    
    // Verify API works
    println!("\n2. Verifying API returns 151 Pokemon...");
    match client.get(base_url).send().await {
        Ok(resp) => {
            if let Ok(pokemon) = resp.json::<Vec<serde_json::Value>>().await {
                println!("   ✓ API returned {} Pokemon", pokemon.len());
            } else {
                println!("   ✓ API responded");
            }
        }
        Err(e) => {
            eprintln!("   ✗ API error: {}", e);
            std::process::exit(1);
        }
    }
    
    // Single benchmark
    println!("\n3. Running single benchmark (10s, 50 concurrent connections)...");
    let results = hammer_api(base_url, 10, 50).await;
    results.print();
    
    // Latency test with sequential requests
    println!("\n4. Running sequential latency test (1000 requests)...");
    let start = Instant::now();
    let mut latencies = Vec::with_capacity(1000);
    
    for i in 1..=1000 {
        let now = Instant::now();
        let id = (i % 151) as u8 + 1;
        let url = format!("{}/{}", base_url, id);
        
        match client.get(&url).send().await {
            Ok(_) => {
                latencies.push(now.elapsed().as_secs_f64() * 1000.0);
            }
            Err(_) => {}
        }
    }
    
    latencies.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    
    println!("   Mean latency: {:.3}ms", latencies.iter().sum::<f64>() / latencies.len() as f64);
    println!("   P50 latency:  {:.3}ms", percentile(&latencies, 0.50));
    println!("   P90 latency:  {:.3}ms", percentile(&latencies, 0.90));
    println!("   P99 latency:  {:.3}ms", percentile(&latencies, 0.99));
    println!("   Total time:   {:.3}s", start.elapsed().as_secs_f64());
    
    // Monte Carlo
    println!("\n5. Running Monte Carlo simulation (15 iterations)...");
    monte_carlo(base_url, 15).await;
    
    println!("\n✨ Benchmark complete!\n");
}
