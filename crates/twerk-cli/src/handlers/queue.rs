//! Queue command handlers
//!
//! HTTP client functions for queue API operations.

use serde::Deserialize;

use crate::error::CliError;

#[derive(Debug, Deserialize)]
pub struct QueueInfo {
    pub name: String,
    pub size: i32,
    pub subscribers: i32,
    pub unacked: i32,
}

pub async fn queue_list(endpoint: &str, json_mode: bool) -> Result<String, CliError> {
    let url = format!("{}/queues", endpoint.trim_end_matches('/'));

    let response = reqwest::get(&url).await.map_err(CliError::Http)?;

    let status = response.status();

    if !status.is_success() {
        return Err(CliError::HttpStatus {
            status: status.as_u16(),
            reason: status.canonical_reason().unwrap_or("Unknown").to_string(),
        });
    }

    let body = response
        .text()
        .await
        .map_err(|e| CliError::InvalidBody(e.to_string()))?;

    let queues: Vec<QueueInfo> =
        serde_json::from_str(&body).map_err(|e| CliError::InvalidBody(e.to_string()))?;

    if json_mode {
        println!("{}", body);
    } else {
        if queues.is_empty() {
            println!("No queues found.");
        } else {
            println!(
                "{:<30} {:>10} {:>12} {:>10}",
                "NAME", "SIZE", "SUBSCRIBERS", "UNACKED"
            );
            println!("{}", "-".repeat(65));
            for q in &queues {
                println!(
                    "{:<30} {:>10} {:>12} {:>10}",
                    q.name, q.size, q.subscribers, q.unacked
                );
            }
        }
    }

    Ok(body)
}

pub async fn queue_get(endpoint: &str, name: &str, json_mode: bool) -> Result<String, CliError> {
    let url = format!("{}/queues/{}", endpoint.trim_end_matches('/'), name);

    let response = reqwest::get(&url).await.map_err(CliError::Http)?;

    let status = response.status();

    if status == reqwest::StatusCode::NOT_FOUND {
        return Err(CliError::NotFound(format!("queue {} not found", name)));
    }

    if !status.is_success() {
        return Err(CliError::HttpStatus {
            status: status.as_u16(),
            reason: status.canonical_reason().unwrap_or("Unknown").to_string(),
        });
    }

    let body = response
        .text()
        .await
        .map_err(|e| CliError::InvalidBody(e.to_string()))?;

    let queue: QueueInfo =
        serde_json::from_str(&body).map_err(|e| CliError::InvalidBody(e.to_string()))?;

    if json_mode {
        println!("{}", body);
    } else {
        println!("Queue: {}", queue.name);
        println!("Size: {}", queue.size);
        println!("Subscribers: {}", queue.subscribers);
        println!("Unacked: {}", queue.unacked);
    }

    Ok(body)
}

pub async fn queue_delete(endpoint: &str, name: &str, json_mode: bool) -> Result<String, CliError> {
    let url = format!("{}/queues/{}", endpoint.trim_end_matches('/'), name);

    let client = reqwest::Client::new();
    let response = client.delete(&url).send().await.map_err(CliError::Http)?;

    let status = response.status();

    if status == reqwest::StatusCode::NOT_FOUND {
        return Err(CliError::NotFound(format!("queue {} not found", name)));
    }

    if !status.is_success() {
        return Err(CliError::HttpStatus {
            status: status.as_u16(),
            reason: status.canonical_reason().unwrap_or("Unknown").to_string(),
        });
    }

    if json_mode {
        println!(r#"{{"type":"queue","name":"{}","deleted":true}}"#, name);
    } else {
        println!("Queue '{}' deleted.", name);
    }

    Ok(format!(r#"{{"deleted":true,"name":"{}"}}"#, name))
}
