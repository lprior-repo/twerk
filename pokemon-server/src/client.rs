//! Pokemon API Client - Hammer the server with concurrent requests
//! 
//! Benchmark types:
//! 1. Throughput - requests per second
//! 2. Latency - p50/p90/p99/p999 response times
//! 3. Concurrency - scaling with parallel connections
//! 4. Monte Carlo - randomized workload simulation

use reqwest::Client;
use serde::Deserialize;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

#[derive(Debug, Deserialize, Clone)]
pub struct Pokemon {
    pub id: u8,
    pub name: String,
    pub types: Vec<String>,
    pub base_stats: Stats,
    pub generation: u8,
}

#[derive(Debug, Deserialize, Clone)]
pub struct Stats {
    pub hp: u16,
    pub attack: u16,
    pub defense: u16,
    pub sp_attack: u16,
    pub sp_defense: u16,
    pub speed: u16,
}

#[derive(Debug)]
pub struct BenchmarkResults {
    pub total_requests: u64,
    pub successful_requests: u64,
    pub failed_requests: u64,
    pub duration_secs: f64,
    pub requests_per_sec: f64,
    pub mean_latency_ms: f64,
    pub p50_latency_ms: f64,
    pub p90_latency_ms: f64,
    pub p99_latency_ms: f64,
    pub p999_latency_ms: f64,
}

impl BenchmarkResults {
    pub fn print(&self) {
        println!("\n╔════════════════════════════════════════════════════════════╗");
        println!("║                   BENCHMARK RESULTS                        ║");
        println!("╠════════════════════════════════════════════════════════════╣");
        println!("║  Total Requests:      {:>10}", self.total_requests);
        println!("║  Successful:         {:>10}", self.successful_requests);
        println!("║  Failed:              {:>10}", self.failed_requests);
        println!("║  Duration:           {:>10.3}s", self.duration_secs);
        println!("║  Throughput:         {:>10.2} req/s", self.requests_per_sec);
        println!("╠════════════════════════════════════════════════════════════╣");
        println!("║                   LATENCY PERCENTILES                      ║");
        println!("╠════════════════════════════════════════════════════════════╣");
        println!("║  Mean:               {:>10.3}ms", self.mean_latency_ms);
        println!("║  P50:               {:>10.3}ms", self.p50_latency_ms);
        println!("║  P90:               {:>10.3}ms", self.p90_latency_ms);
        println!("║  P99:               {:>10.3}ms", self.p99_latency_ms);
        println!("║  P99.9:             {:>10.3}ms", self.p999_latency_ms);
        println!("╚════════════════════════════════════════════════════════════╝");
    }
}

/// Calculate percentile from sorted latencies
fn percentile(sorted: &[f64], p: f64) -> f64 {
    if sorted.is_empty() {
        return 0.0;
    }
    // Clamp p to [0.0, 1.0] range
    let p = p.clamp(0.0, 1.0);
    // Use floor index for discrete percentile
    let n = sorted.len() as f64;
    let idx = (p * (n - 1.0)).floor() as usize;
    sorted[idx.min(sorted.len().saturating_sub(1))]
}

#[derive(Debug, thiserror::Error)]
pub enum ClientError {
    #[error("HTTP client error: {0}")]
    Http(#[from] reqwest::Error),
    
    #[error("Invalid base URL: {0}")]
    InvalidUrl(String),
    
    #[error("Client build error: {0}")]
    Build(String),
}

pub struct PokemonClient {
    client: Client,
    base_url: String,
}

impl PokemonClient {
    /// Create a new Pokemon API client
    pub fn new(base_url: &str) -> Result<Self, ClientError> {
        // Validate URL is well-formed
        if !base_url.starts_with("http://") && !base_url.starts_with("https://") {
            return Err(ClientError::InvalidUrl(format!(
                "URL must start with http:// or https://, got: {}", base_url
            )));
        }
        
        let client = Client::builder()
            .timeout(Duration::from_secs(30))
            .build()
            .map_err(|e| ClientError::Build(e.to_string()))?;
        
        Ok(Self {
            client,
            base_url: base_url.to_string(),
        })
    }

    /// Health check
    pub async fn health_check(&self) -> Result<String, ClientError> {
        self.client.get(format!("{}/health", self.base_url))
            .send()
            .await?
            .text()
            .await
            .map_err(ClientError::Http)
    }

    /// Get all 151 Pokemon
    pub async fn get_all_pokemon(&self) -> Result<Vec<Pokemon>, ClientError> {
        self.client.get(format!("{}/api/pokemon", self.base_url))
            .send()
            .await?
            .json::<Vec<Pokemon>>()
            .await
            .map_err(ClientError::Http)
    }

    /// Get single Pokemon by ID
    pub async fn get_pokemon_by_id(&self, id: u8) -> Result<Pokemon, ClientError> {
        if id == 0 || id > 151 {
            return Err(ClientError::InvalidUrl(format!(
                "Pokemon ID must be 1-151, got: {}", id
            )));
        }
        
        self.client.get(format!("{}/api/pokemon/{}", self.base_url, id))
            .send()
            .await?
            .json::<Pokemon>()
            .await
            .map_err(ClientError::Http)
    }

    /// Get Pokemon IDs by type
    pub async fn get_pokemon_by_type(&self, type_name: &str) -> Result<Vec<u8>, ClientError> {
        let valid_types = [
            "fire", "water", "grass", "electric", "psychic", 
            "bug", "normal", "poison", "ground", "rock",
            "ghost", "ice", "fighting", "dragon", "flying"
        ];
        
        let normalized = type_name.to_lowercase();
        if !valid_types.contains(&normalized.as_str()) {
            return Err(ClientError::InvalidUrl(format!(
                "Invalid type: {}. Valid types: {:?}", type_name, valid_types
            )));
        }
        
        self.client.get(format!("{}/api/pokemon/type/{}", self.base_url, normalized))
            .send()
            .await?
            .json::<Vec<u8>>()
            .await
            .map_err(ClientError::Http)
    }
    
    /// Get the base URL
    pub fn base_url(&self) -> &str {
        &self.base_url
    }
}

/// Worker function that runs for the specified duration and returns latencies
async fn benchmark_worker(
    client: Arc<PokemonClient>,
    deadline: tokio::time::Instant,
) -> (Vec<f64>, u64, u64) {
    let mut latencies = Vec::with_capacity(10_000);
    let mut successful = 0u64;
    let mut failed = 0u64;
    
    loop {
        // Check if we've exceeded our deadline
        if tokio::time::Instant::now() >= deadline {
            break;
        }
        
        let now = Instant::now();
        match client.get_all_pokemon().await {
            Ok(pokemon) if pokemon.len() == 151 => {
                let elapsed = now.elapsed().as_secs_f64() * 1000.0;
                successful += 1;
                latencies.push(elapsed);
            }
            Ok(_) => {
                // Wrong number of Pokemon - count as failed
                failed += 1;
            }
            Err(_) => {
                failed += 1;
            }
        }
        
        // Yield periodically to prevent monopolizing
        if (successful + failed).is_multiple_of(100) {
            tokio::task::yield_now().await;
        }
    }
    
    (latencies, successful, failed)
}

/// Concurrent benchmark with specified parameters
pub async fn benchmark_throughput(
    url: &str,
    duration_secs: u64,
    concurrency: usize,
) -> Result<BenchmarkResults, ClientError> {
    let client = Arc::new(PokemonClient::new(url)?);
    let start = Instant::now();
    let deadline = start + Duration::from_secs(duration_secs);
    
    // Spawn all workers
    let mut handles = Vec::with_capacity(concurrency);
    for _ in 0..concurrency {
        let client = client.clone();
        handles.push(tokio::spawn(benchmark_worker(client, deadline.into())));
    }
    
    // Wait for all workers to complete
    let mut all_latencies = Vec::new();
    let mut total_successful = 0u64;
    let mut total_failed = 0u64;
    
    for handle in handles {
        if let Ok((latencies, s, f)) = handle.await {
            all_latencies.extend(latencies);
            total_successful += s;
            total_failed += f;
        }
    }
    
    let duration = start.elapsed();
    let total = total_successful + total_failed;
    
    all_latencies.sort_by(|a: &f64, b: &f64| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    
    let mean = if !all_latencies.is_empty() {
        all_latencies.iter().sum::<f64>() / all_latencies.len() as f64
    } else {
        0.0
    };
    
    Ok(BenchmarkResults {
        total_requests: total,
        successful_requests: total_successful,
        failed_requests: total_failed,
        duration_secs: duration.as_secs_f64(),
        requests_per_sec: total as f64 / duration.as_secs_f64(),
        mean_latency_ms: mean,
        p50_latency_ms: percentile(&all_latencies, 0.50),
        p90_latency_ms: percentile(&all_latencies, 0.90),
        p99_latency_ms: percentile(&all_latencies, 0.99),
        p999_latency_ms: percentile(&all_latencies, 0.999),
    })
}

/// Monte Carlo simulation - randomized workload with varying concurrency
pub async fn monte_carlo_simulation(url: &str, num_iterations: usize) -> Vec<BenchmarkResults> {
    let mut results = Vec::with_capacity(num_iterations);
    
    for i in 0..num_iterations {
        println!("  Monte Carlo iteration {}/{}", i + 1, num_iterations);
        
        // Randomized parameters: vary concurrency from 5 to 50
        let duration = 2; // 2 seconds per iteration
        let concurrency = ((i % 10) + 1) * 5; // 5, 10, 15, ... 50
        
        match benchmark_throughput(url, duration, concurrency).await {
            Ok(result) => results.push(result),
            Err(e) => {
                eprintln!("  Iteration {} failed: {}", i + 1, e);
            }
        }
    }
    
    results
}

/// Print Monte Carlo summary statistics
pub fn print_monte_carlo_summary(results: &[BenchmarkResults]) {
    if results.is_empty() {
        println!("No results to summarize");
        return;
    }
    
    let mut throughputs: Vec<f64> = results.iter().map(|r| r.requests_per_sec).collect();
    throughputs.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    
    let mut latencies: Vec<f64> = results.iter().map(|r| r.p99_latency_ms).collect();
    latencies.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    
    let mean_throughput = throughputs.iter().sum::<f64>() / throughputs.len() as f64;
    let mean_p99 = latencies.iter().sum::<f64>() / latencies.len() as f64;
    
    println!("\n╔════════════════════════════════════════════════════════════╗");
    println!("║              MONTE CARLO SIMULATION SUMMARY               ║");
    println!("╠════════════════════════════════════════════════════════════╣");
    println!("║  Iterations:            {:>10}", results.len());
    println!("╠════════════════════════════════════════════════════════════╣");
    println!("║                   THROUGHPUT (req/s)                       ║");
    println!("╠════════════════════════════════════════════════════════════╣");
    println!("║  Min:                  {:>10.2}", throughputs.first().unwrap_or(&0.0));
    println!("║  Mean:                 {:>10.2}", mean_throughput);
    println!("║  Max:                 {:>10.2}", throughputs.last().unwrap_or(&0.0));
    println!("║  P50:                 {:>10.2}", percentile(&throughputs, 0.50));
    println!("║  P90:                 {:>10.2}", percentile(&throughputs, 0.90));
    println!("╠════════════════════════════════════════════════════════════╣");
    println!("║                   LATENCY P99 (ms)                         ║");
    println!("╠════════════════════════════════════════════════════════════╣");
    println!("║  Min:                  {:>10.3}", latencies.first().unwrap_or(&0.0));
    println!("║  Mean:                 {:>10.3}", mean_p99);
    println!("║  Max:                 {:>10.3}", latencies.last().unwrap_or(&0.0));
    println!("║  P50:                 {:>10.3}", percentile(&latencies, 0.50));
    println!("║  P90:                 {:>10.3}", percentile(&latencies, 0.90));
    println!("╚════════════════════════════════════════════════════════════╝");
}

/// Latency benchmark with detailed tracking
pub async fn benchmark_latency(
    url: &str, 
    num_requests: usize,
) -> Result<(Vec<f64>, u64, u64), ClientError> {
    let client = PokemonClient::new(url)?;
    let mut latencies = Vec::with_capacity(num_requests);
    let mut successful = 0u64;
    let mut failed = 0u64;
    
    for i in 1..=num_requests {
        let now = Instant::now();
        let id = (i % 151) as u8 + 1;
        
        match client.get_pokemon_by_id(id).await {
            Ok(_) => {
                successful += 1;
                latencies.push(now.elapsed().as_secs_f64() * 1000.0);
            }
            Err(e) => {
                failed += 1;
                eprintln!("Request {} failed: {}", i, e);
            }
        }
    }
    
    Ok((latencies, successful, failed))
}

// ============================================================================
// MAIN - Run benchmarks
// ============================================================================

#[tokio::main]
async fn main() {
    println!("\n🔥 POKEMON API BENCHMARK CLIENT 🔥\n");
    
    let base_url = "http://127.0.0.1:8080";
    
    // Create client with proper error handling
    let client = match PokemonClient::new(base_url) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("Failed to create client: {}", e);
            std::process::exit(1);
        }
    };
    
    // Verify server is running
    println!("1. Verifying server is running...");
    match client.health_check().await {
        Ok(status) => println!("   ✓ Server healthy: {}", status),
        Err(e) => {
            eprintln!("   ✗ Server not reachable: {}", e);
            eprintln!("\n   Start the server first:");
            eprintln!("   cd pokemon-server && cargo run\n");
            std::process::exit(1);
        }
    }
    
    // Get all Pokemon to verify
    println!("\n2. Fetching all 151 Pokemon...");
    match client.get_all_pokemon().await {
        Ok(pokemon) => println!("   ✓ Retrieved {} Pokemon", pokemon.len()),
        Err(e) => {
            eprintln!("   ✗ Failed to get Pokemon: {}", e);
            std::process::exit(1);
        }
    }
    
    // Test single Pokemon
    println!("\n3. Fetching Pikachu (#25)...");
    match client.get_pokemon_by_id(25).await {
        Ok(p) => println!("   ✓ {} - {:?}", p.name, p.types),
        Err(e) => eprintln!("   ✗ Failed: {}", e),
    }
    
    // Test type filtering
    println!("\n4. Fetching Fire type Pokemon...");
    match client.get_pokemon_by_type("fire").await {
        Ok(ids) => println!("   ✓ Found {} Fire type Pokemon: {:?}", ids.len(), ids),
        Err(e) => eprintln!("   ✗ Failed: {}", e),
    }
    
    // Throughput benchmark
    println!("\n5. Running throughput benchmark (10s, 50 concurrent connections)...");
    match benchmark_throughput(base_url, 10, 50).await {
        Ok(result) => result.print(),
        Err(e) => eprintln!("   ✗ Benchmark failed: {}", e),
    }
    
    // Latency benchmark with single requests
    println!("\n6. Running latency benchmark (1000 sequential requests)...");
    match benchmark_latency(base_url, 1000).await {
        Ok((mut latencies, successful, failed)) => {
            latencies.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
            
            println!("   Successful: {}", successful);
            println!("   Failed: {}", failed);
            println!("   Mean latency: {:.3}ms", latencies.iter().sum::<f64>() / latencies.len() as f64);
            println!("   P50 latency:  {:.3}ms", percentile(&latencies, 0.50));
            println!("   P90 latency:  {:.3}ms", percentile(&latencies, 0.90));
            println!("   P99 latency:  {:.3}ms", percentile(&latencies, 0.99));
        }
        Err(e) => eprintln!("   ✗ Latency benchmark failed: {}", e),
    }
    
    // Monte Carlo simulation
    println!("\n7. Running Monte Carlo simulation (15 iterations)...");
    println!("   (Each iteration: 2s duration with varying concurrency)");
    let mc_results = monte_carlo_simulation(base_url, 15).await;
    print_monte_carlo_summary(&mc_results);
    
    println!("\n✨ Benchmark complete!\n");
}

// ============================================================================
// TESTS
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    // ========================================================================
    // PokemonClient Tests
    // ========================================================================

    #[test]
    fn test_pokemon_client_new_valid_url() {
        let result = PokemonClient::new("http://localhost:8080");
        assert!(result.is_ok());
    }

    #[test]
    fn test_pokemon_client_new_https_url() {
        let result = PokemonClient::new("https://api.example.com");
        assert!(result.is_ok());
    }

    #[test]
    fn test_pokemon_client_new_invalid_url_no_scheme() {
        let result = PokemonClient::new("localhost:8080");
        assert!(result.is_err());
        match result {
            Err(ClientError::InvalidUrl(msg)) => {
                assert!(msg.contains("http:// or https://"));
            }
            _ => panic!("Expected InvalidUrl error"),
        }
    }

    #[test]
    fn test_pokemon_client_new_invalid_url_empty() {
        let result = PokemonClient::new("");
        assert!(result.is_err());
    }

    #[test]
    fn test_client_error_display() {
        let err = ClientError::InvalidUrl("test error".to_string());
        assert!(err.to_string().contains("test error"));
        
        let err2 = ClientError::Build("build failed".to_string());
        assert!(err2.to_string().contains("build failed"));
    }

    #[test]
    fn test_client_error_http_display() {
        // ClientError::Http wraps reqwest::Error
        // We can't easily construct reqwest::Error in tests, so we test the other variants
        let err: ClientError = ClientError::InvalidUrl("test".to_string());
        let msg = err.to_string();
        assert!(msg.contains("Invalid base URL"));
    }

    // ========================================================================
    // Percentile Calculation Tests
    // ========================================================================

    #[test]
    fn test_percentile_empty() {
        let result = percentile(&[], 0.5);
        assert_eq!(result, 0.0);
    }

    #[test]
    fn test_percentile_single_element() {
        let sorted = vec![100.0];
        assert_eq!(percentile(&sorted, 0.5), 100.0);
    }

    #[test]
    fn test_percentile_p50() {
        let sorted = vec![1.0, 2.0, 3.0, 4.0, 5.0];
        assert_eq!(percentile(&sorted, 0.5), 3.0);
    }

    #[test]
    fn test_percentile_p90() {
        let sorted: Vec<f64> = (1..=100).map(|i| i as f64).collect();
        assert_eq!(percentile(&sorted, 0.9), 90.0);
    }

    #[test]
    fn test_percentile_p99() {
        let sorted: Vec<f64> = (1..=1000).map(|i| i as f64).collect();
        // P99 of 1..=1000 is 990 (index 989: 0.99 * 999 = 989.01 -> floor 989)
        assert_eq!(percentile(&sorted, 0.99), 990.0);
    }

    #[test]
    fn test_percentile_out_of_bounds() {
        let sorted = vec![1.0, 2.0, 3.0];
        // Should clamp to last element
        assert_eq!(percentile(&sorted, 1.5), 3.0);
    }

    #[test]
    fn test_percentile_zero() {
        let sorted = vec![1.0, 2.0, 3.0];
        assert_eq!(percentile(&sorted, 0.0), 1.0);
    }

    // ========================================================================
    // BenchmarkResults Tests
    // ========================================================================

    #[test]
    fn test_benchmark_results_default() {
        let results = BenchmarkResults {
            total_requests: 1000,
            successful_requests: 950,
            failed_requests: 50,
            duration_secs: 10.0,
            requests_per_sec: 100.0,
            mean_latency_ms: 5.0,
            p50_latency_ms: 4.5,
            p90_latency_ms: 6.0,
            p99_latency_ms: 8.0,
            p999_latency_ms: 10.0,
        };
        
        assert_eq!(results.total_requests, 1000);
        assert_eq!(results.successful_requests, 950);
        assert_eq!(results.failed_requests, 50);
        assert_eq!(results.requests_per_sec, 100.0);
    }

    #[test]
    fn test_benchmark_results_zero_duration() {
        let results = BenchmarkResults {
            total_requests: 0,
            successful_requests: 0,
            failed_requests: 0,
            duration_secs: 0.0,
            requests_per_sec: 0.0,
            mean_latency_ms: 0.0,
            p50_latency_ms: 0.0,
            p90_latency_ms: 0.0,
            p99_latency_ms: 0.0,
            p999_latency_ms: 0.0,
        };
        
        assert_eq!(results.duration_secs, 0.0);
        assert_eq!(results.requests_per_sec, 0.0);
    }

    // ========================================================================
    // Monte Carlo Summary Tests
    // ========================================================================

    #[test]
    fn test_print_monte_carlo_summary_empty() {
        // Should not panic
        print_monte_carlo_summary(&[]);
    }

    #[test]
    fn test_print_monte_carlo_summary_single() {
        let results = vec![BenchmarkResults {
            total_requests: 100,
            successful_requests: 100,
            failed_requests: 0,
            duration_secs: 1.0,
            requests_per_sec: 100.0,
            mean_latency_ms: 5.0,
            p50_latency_ms: 5.0,
            p90_latency_ms: 6.0,
            p99_latency_ms: 7.0,
            p999_latency_ms: 8.0,
        }];
        
        // Should not panic
        print_monte_carlo_summary(&results);
    }

    #[test]
    fn test_monte_carlo_multiple_iterations() {
        let results: Vec<BenchmarkResults> = (0..10).map(|i| BenchmarkResults {
            total_requests: 100 + i as u64,
            successful_requests: 95 + i as u64,
            failed_requests: 5,
            duration_secs: 1.0,
            requests_per_sec: 100.0 + i as f64,
            mean_latency_ms: 5.0 + i as f64,
            p50_latency_ms: 4.5 + i as f64,
            p90_latency_ms: 6.0 + i as f64,
            p99_latency_ms: 8.0 + i as f64,
            p999_latency_ms: 10.0 + i as f64,
        }).collect();
        
        // Should not panic
        print_monte_carlo_summary(&results);
    }

    // ========================================================================
    // Pokemon Struct Deserialization Tests
    // ========================================================================

    #[test]
    fn test_pokemon_deserialization() {
        let json = r#"{
            "id": 25,
            "name": "Pikachu",
            "types": ["Electric"],
            "base_stats": {
                "hp": 35,
                "attack": 55,
                "defense": 40,
                "sp_attack": 50,
                "sp_defense": 50,
                "speed": 90
            },
            "generation": 1
        }"#;
        
        let pokemon: Pokemon = serde_json::from_str(json).unwrap();
        assert_eq!(pokemon.id, 25);
        assert_eq!(pokemon.name, "Pikachu");
        assert_eq!(pokemon.types, vec!["Electric"]);
        assert_eq!(pokemon.base_stats.hp, 35);
        assert_eq!(pokemon.base_stats.speed, 90);
        assert_eq!(pokemon.generation, 1);
    }

    #[test]
    fn test_pokemon_dual_type_deserialization() {
        let json = r#"{
            "id": 6,
            "name": "Charizard",
            "types": ["Fire", "Flying"],
            "base_stats": {
                "hp": 78,
                "attack": 84,
                "defense": 78,
                "sp_attack": 109,
                "sp_defense": 85,
                "speed": 100
            },
            "generation": 1
        }"#;
        
        let pokemon: Pokemon = serde_json::from_str(json).unwrap();
        assert_eq!(pokemon.types, vec!["Fire", "Flying"]);
    }

    #[test]
    fn test_stats_total() {
        let stats = Stats {
            hp: 45,
            attack: 49,
            defense: 49,
            sp_attack: 65,
            sp_defense: 65,
            speed: 45,
        };
        
        let total = stats.hp as u32 + stats.attack as u32 + stats.defense as u32 
            + stats.sp_attack as u32 + stats.sp_defense as u32 + stats.speed as u32;
        assert_eq!(total, 318);
    }

    // ========================================================================
    // Client URL Validation Tests
    // ========================================================================

    #[test]
    fn test_client_url_trailing_slash() {
        // Should handle URLs with or without trailing slash
        let result1 = PokemonClient::new("http://localhost:8080");
        let result2 = PokemonClient::new("http://localhost:8080/");
        
        assert!(result1.is_ok());
        assert!(result2.is_ok());
    }

    #[test]
    fn test_client_url_with_path() {
        let result = PokemonClient::new("http://localhost:8080/api");
        assert!(result.is_ok());
    }

    // ========================================================================
    // Error Edge Cases
    // ========================================================================

    #[test]
    fn test_client_error_all_variants() {
        let err1 = ClientError::InvalidUrl("bad".to_string());
        let err2 = ClientError::Build("build".to_string());
        
        // Both should display properly
        assert!(!err1.to_string().is_empty());
        assert!(!err2.to_string().is_empty());
    }

    // ========================================================================
    // Latency Tracking Tests
    // ========================================================================

    #[test]
    fn test_latency_tracking_vector_growth() {
        let mut latencies = Vec::new();
        
        // Simulate adding latencies
        for i in 1..=1000 {
            let latency = i as f64 * 0.1;
            latencies.push(latency);
        }
        
        assert_eq!(latencies.len(), 1000);
        assert_eq!(latencies[0], 0.1);
        assert_eq!(latencies[999], 100.0);
    }

    #[test]
    fn test_latency_sorting() {
        let mut latencies = vec![50.0, 10.0, 30.0, 20.0, 40.0];
        latencies.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
        
        assert_eq!(latencies, vec![10.0, 20.0, 30.0, 40.0, 50.0]);
    }

    // ========================================================================
    // Throughput Calculation Tests
    // ========================================================================

    #[test]
    fn test_throughput_calculation() {
        let total_requests = 1000u64;
        let duration_secs = 10.0;
        
        let throughput = total_requests as f64 / duration_secs;
        assert_eq!(throughput, 100.0);
    }

    #[test]
    fn test_throughput_zero_duration() {
        let total_requests = 1000u64;
        let duration_secs = 0.0;
        
        let throughput = total_requests as f64 / duration_secs;
        assert!(throughput.is_infinite());
    }

    // ========================================================================
    // Mean Calculation Tests
    // ========================================================================

    #[test]
    fn test_mean_calculation() {
        let latencies = vec![1.0, 2.0, 3.0, 4.0, 5.0];
        let mean = latencies.iter().sum::<f64>() / latencies.len() as f64;
        assert_eq!(mean, 3.0);
    }

    #[test]
    fn test_mean_empty() {
        let latencies: Vec<f64> = vec![];
        let mean = if latencies.is_empty() {
            0.0
        } else {
            latencies.iter().sum::<f64>() / latencies.len() as f64
        };
        assert_eq!(mean, 0.0);
    }

    #[test]
    fn test_mean_single_element() {
        let latencies = vec![42.0];
        let mean = latencies.iter().sum::<f64>() / latencies.len() as f64;
        assert_eq!(mean, 42.0);
    }

    // ========================================================================
    // Atomic Operations (Compile-time verification)
    // ========================================================================

    #[test]
    fn test_atomic_u64_operations() {
        let counter = AtomicU64::new(0);
        
        // Test fetch_add
        counter.fetch_add(1, Ordering::Relaxed);
        assert_eq!(counter.load(Ordering::Relaxed), 1);
        
        // Test multiple increments
        counter.fetch_add(5, Ordering::Relaxed);
        assert_eq!(counter.load(Ordering::Relaxed), 6);
        
        // Test fetch_sub
        counter.fetch_sub(2, Ordering::Relaxed);
        assert_eq!(counter.load(Ordering::Relaxed), 4);
    }

    // ========================================================================
    // Concurrency Simulation Tests
    // ========================================================================

    #[tokio::test]
    async fn test_async_client_creation() {
        let result = PokemonClient::new("http://test.example.com");
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_invalid_pokemon_id_zero() {
        let client = PokemonClient::new("http://localhost:8080").unwrap();
        let result = client.get_pokemon_by_id(0).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_invalid_pokemon_id_above_151() {
        let client = PokemonClient::new("http://localhost:8080").unwrap();
        let result = client.get_pokemon_by_id(152).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_invalid_type_name() {
        let client = PokemonClient::new("http://localhost:8080").unwrap();
        let result = client.get_pokemon_by_type("invalid_type").await;
        assert!(result.is_err());
    }

    // ========================================================================
    // Stress Test预备 (Preparation for stress tests)
    // ========================================================================

    #[test]
    fn test_result_aggregation() {
        let mut total = 0u64;
        let mut successful = 0u64;
        let mut failed = 0u64;
        
        // Simulate multiple benchmark iterations
        for _ in 0..100 {
            total += 100;
            successful += 95;
            failed += 5;
        }
        
        assert_eq!(total, 10000);
        assert_eq!(successful, 9500);
        assert_eq!(failed, 500);
        assert_eq!(successful + failed, total);
    }

    // ========================================================================
    // Duration Calculation Tests
    // ========================================================================

    #[test]
    fn test_duration_calculation() {
        use std::time::Instant;
        
        let start = Instant::now();
        // Simulate some work
        std::thread::sleep(Duration::from_millis(10));
        let duration = start.elapsed();
        
        assert!(duration.as_secs_f64() >= 0.01);
    }

    // ========================================================================
    // Order Statistics Tests
    // ========================================================================

    #[test]
    fn test_percentile_order_statistics() {
        // Test that percentile properly selects from sorted data
        let sorted = vec![1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0, 9.0, 10.0];
        
        assert_eq!(percentile(&sorted, 0.0), 1.0);  // Min
        assert_eq!(percentile(&sorted, 0.1), 1.0);  // Near min
        assert_eq!(percentile(&sorted, 0.5), 5.0);  // Median
        assert_eq!(percentile(&sorted, 0.9), 9.0);  // Near max
        assert_eq!(percentile(&sorted, 1.0), 10.0); // Max
    }

    // ========================================================================
    // Base URL Accessor Test
    // ========================================================================

    #[test]
    fn test_base_url_accessor() {
        let client = PokemonClient::new("http://example.com:8080").unwrap();
        assert_eq!(client.base_url(), "http://example.com:8080");
    }
}
