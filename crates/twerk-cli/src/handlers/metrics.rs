//! Metrics command handlers
//!
//! HTTP client functions for metrics API operations.

use serde::Deserialize;

use crate::error::CliError;
use crate::handlers::common::TriggerErrorResponse;

#[derive(Debug, Deserialize)]
pub struct JobMetrics {
    pub running: i32,
}

#[derive(Debug, Deserialize)]
pub struct TaskMetrics {
    pub running: i32,
}

#[derive(Debug, Deserialize)]
pub struct NodeMetrics {
    #[serde(rename = "online")]
    pub running: i32,
    #[serde(rename = "cpuPercent")]
    pub cpu_percent: f64,
}

#[derive(Debug, Deserialize)]
pub struct Metrics {
    pub jobs: JobMetrics,
    pub tasks: TaskMetrics,
    pub nodes: NodeMetrics,
}

pub async fn metrics_get(endpoint: &str, json_mode: bool) -> Result<String, CliError> {
    let url = format!("{}/metrics", endpoint.trim_end_matches('/'));

    let response = reqwest::get(&url).await.map_err(CliError::Http)?;

    let status = response.status();

    if !status.is_success() {
        let body = response
            .text()
            .await
            .map_err(|e| CliError::InvalidBody(e.to_string()))?;
        if let Ok(err_resp) = serde_json::from_str::<TriggerErrorResponse>(&body) {
            return Err(CliError::ApiError {
                code: status.as_u16(),
                message: err_resp.message,
            });
        }
        return Err(CliError::HttpStatus {
            status: status.as_u16(),
            reason: status.canonical_reason().unwrap_or("Unknown").to_string(),
        });
    }

    let body = response
        .text()
        .await
        .map_err(|e| CliError::InvalidBody(e.to_string()))?;

    if json_mode {
        println!("{}", body);
    } else {
        let metrics: Metrics =
            serde_json::from_str(&body).map_err(|e| CliError::InvalidBody(e.to_string()))?;
        println!("Metrics:");
        println!("  Jobs Running: {}", metrics.jobs.running);
        println!("  Tasks Running: {}", metrics.tasks.running);
        println!("  Nodes Online: {}", metrics.nodes.running);
        println!("  CPU Usage: {:.2}%", metrics.nodes.cpu_percent);
    }

    Ok(body)
}
