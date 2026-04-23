//! Metrics command handlers
//!
//! HTTP client functions for metrics API operations.

use serde::Deserialize;

use crate::error::CliError;

#[derive(Debug, Deserialize)]
pub struct Metrics {
    #[serde(default)]
    pub uptime_seconds: Option<u64>,
    #[serde(default)]
    pub total_tasks: Option<u64>,
    #[serde(default)]
    pub queued_tasks: Option<u64>,
    #[serde(default)]
    pub running_tasks: Option<u64>,
    #[serde(default)]
    pub completed_tasks: Option<u64>,
    #[serde(default)]
    pub failed_tasks: Option<u64>,
    #[serde(default)]
    pub total_nodes: Option<u64>,
    #[serde(default)]
    pub active_nodes: Option<u64>,
    #[serde(default)]
    pub memory_usage_bytes: Option<u64>,
    #[serde(default)]
    pub cpu_usage_percent: Option<f64>,
}

pub async fn metrics_get(endpoint: &str, json_mode: bool) -> Result<String, CliError> {
    let url = format!("{}/metrics", endpoint.trim_end_matches('/'));

    let response = reqwest::get(&url).await.map_err(CliError::Http)?;

    let status = response.status();

    if !status.is_success() {
        return Err(CliError::HttpStatus {
            status: status.as_u16(),
            reason: status
                .canonical_reason()
                .map_or_else(|| "Unknown".to_string(), |s| s.to_string()),
        });
    }

    let body = response
        .text()
        .await
        .map_err(|e| CliError::InvalidBody(e.to_string()))?;

    let metrics: Metrics =
        serde_json::from_str(&body).map_err(|e| CliError::InvalidBody(e.to_string()))?;

    if json_mode {
        println!("{}", body);
    } else {
        println!("Metrics:");
        if let Some(uptime) = metrics.uptime_seconds {
            println!("  Uptime: {}s", uptime);
        }
        if let Some(total) = metrics.total_tasks {
            println!("  Total Tasks: {}", total);
        }
        if let Some(queued) = metrics.queued_tasks {
            println!("  Queued Tasks: {}", queued);
        }
        if let Some(running) = metrics.running_tasks {
            println!("  Running Tasks: {}", running);
        }
        if let Some(completed) = metrics.completed_tasks {
            println!("  Completed Tasks: {}", completed);
        }
        if let Some(failed) = metrics.failed_tasks {
            println!("  Failed Tasks: {}", failed);
        }
        if let Some(total_nodes) = metrics.total_nodes {
            println!("  Total Nodes: {}", total_nodes);
        }
        if let Some(active) = metrics.active_nodes {
            println!("  Active Nodes: {}", active);
        }
        if let Some(mem) = metrics.memory_usage_bytes {
            println!("  Memory Usage: {} bytes", mem);
        }
        if let Some(cpu) = metrics.cpu_usage_percent {
            println!("  CPU Usage: {:.2}%", cpu);
        }
    }

    Ok(body)
}
