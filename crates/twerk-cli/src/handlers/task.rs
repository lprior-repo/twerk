//! Task command handlers
//!
//! HTTP client functions for task API operations.

use serde::Deserialize;

use crate::error::CliError;

#[derive(Debug, Deserialize)]
pub struct TaskResponse {
    #[serde(default)]
    pub id: Option<String>,
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub state: Option<String>,
    #[serde(default)]
    pub job_id: Option<String>,
    #[serde(default)]
    pub queue: Option<String>,
    #[serde(default)]
    pub error: Option<String>,
    #[serde(default)]
    pub created_at: Option<String>,
    #[serde(default)]
    pub started_at: Option<String>,
    #[serde(default)]
    pub completed_at: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct TaskLogPage {
    pub items: Vec<TaskLogEntry>,
    #[serde(default)]
    pub number: i64,
    #[serde(default)]
    pub size: i64,
    #[serde(default)]
    pub total_pages: i64,
    #[serde(default)]
    pub total_items: i64,
}

#[derive(Debug, Deserialize)]
pub struct TaskLogEntry {
    #[serde(default)]
    pub id: Option<String>,
    #[serde(default)]
    pub task_id: Option<String>,
    #[serde(default)]
    pub number: i64,
    #[serde(default)]
    pub contents: Option<String>,
    #[serde(default)]
    pub created_at: Option<String>,
}

pub async fn task_get(endpoint: &str, task_id: &str, json_mode: bool) -> Result<String, CliError> {
    let url = format!("{}/tasks/{}", endpoint.trim_end_matches('/'), task_id);

    let response = reqwest::get(&url).await.map_err(CliError::Http)?;

    let status = response.status();

    if status == reqwest::StatusCode::NOT_FOUND {
        return Err(CliError::NotFound(format!("task {} not found", task_id)));
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

    let task: TaskResponse =
        serde_json::from_str(&body).map_err(|e| CliError::InvalidBody(e.to_string()))?;

    if json_mode {
        println!("{}", body);
    } else {
        println!("Task: {}", task.id.as_deref().unwrap_or("unknown"));
        if let Some(name) = &task.name {
            println!("Name: {}", name);
        }
        if let Some(state) = &task.state {
            println!("State: {}", state);
        }
        if let Some(job_id) = &task.job_id {
            println!("Job: {}", job_id);
        }
        if let Some(queue) = &task.queue {
            println!("Queue: {}", queue);
        }
        if let Some(error) = &task.error {
            println!("Error: {}", error);
        }
        if let Some(created) = &task.created_at {
            println!("Created: {}", created);
        }
        if let Some(started) = &task.started_at {
            println!("Started: {}", started);
        }
        if let Some(completed) = &task.completed_at {
            println!("Completed: {}", completed);
        }
    }

    Ok(body)
}

pub async fn task_log(
    endpoint: &str,
    task_id: &str,
    page: Option<i64>,
    size: Option<i64>,
    json_mode: bool,
) -> Result<String, CliError> {
    let mut url = format!("{}/tasks/{}/log", endpoint.trim_end_matches('/'), task_id);
    let mut params = Vec::new();
    if let Some(p) = page {
        params.push(format!("page={}", p));
    }
    if let Some(s) = size {
        params.push(format!("size={}", s));
    }
    if !params.is_empty() {
        url.push('?');
        url.push_str(&params.join("&"));
    }

    let response = reqwest::get(&url).await.map_err(CliError::Http)?;

    let status = response.status();

    if status == reqwest::StatusCode::NOT_FOUND {
        return Err(CliError::NotFound(format!("task {} not found", task_id)));
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

    let log_page: TaskLogPage =
        serde_json::from_str(&body).map_err(|e| CliError::InvalidBody(e.to_string()))?;

    if json_mode {
        println!("{}", body);
    } else {
        println!("Task: {}", task_id);
        println!("---");
        for entry in &log_page.items {
            if let Some(contents) = &entry.contents {
                println!("[{}] {}", entry.number, contents);
            }
        }
        println!("---");
        println!(
            "Page {}/{} ({} items total)",
            log_page.number, log_page.total_pages, log_page.total_items
        );
    }

    Ok(body)
}