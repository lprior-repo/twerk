//! Job command handlers
//!
//! HTTP client functions for job API operations.

use serde::Deserialize;

use crate::error::CliError;

#[derive(Debug, Deserialize)]
pub struct JobPage {
    pub items: Vec<JobSummaryItem>,
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
#[serde(rename_all = "camelCase")]
pub struct JobSummaryItem {
    #[serde(default)]
    pub id: Option<String>,
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub state: Option<String>,
    #[serde(default)]
    pub created_at: Option<String>,
    #[serde(default)]
    pub started_at: Option<String>,
    #[serde(default)]
    pub completed_at: Option<String>,
    #[serde(default)]
    pub task_count: i64,
    #[serde(default)]
    pub progress: f64,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct JobDetail {
    #[serde(default)]
    pub id: Option<String>,
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub state: Option<String>,
    #[serde(default)]
    pub created_at: Option<String>,
    #[serde(default)]
    pub created_by: Option<serde_json::Value>,
    #[serde(default)]
    pub started_at: Option<String>,
    #[serde(default)]
    pub completed_at: Option<String>,
    #[serde(default)]
    pub tasks: Option<Vec<serde_json::Value>>,
    #[serde(default)]
    pub task_count: i64,
    #[serde(default)]
    pub position: i64,
    #[serde(default)]
    pub inputs: Option<serde_json::Value>,
    #[serde(default)]
    pub output: Option<String>,
    #[serde(default)]
    pub result: Option<String>,
    #[serde(default)]
    pub error: Option<String>,
    #[serde(default)]
    pub progress: f64,
}

#[derive(Debug, Deserialize)]
pub struct JobLogPage {
    pub items: Vec<JobLogEntry>,
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
pub struct JobLogEntry {
    #[serde(default)]
    pub id: Option<String>,
    #[serde(default)]
    pub job_id: Option<String>,
    #[serde(default)]
    pub number: i64,
    #[serde(default)]
    pub contents: Option<String>,
    #[serde(default)]
    pub created_at: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct StatusMessage {
    pub status: String,
}

pub async fn job_list(
    endpoint: &str,
    page: Option<i64>,
    size: Option<i64>,
    json_mode: bool,
) -> Result<String, CliError> {
    let mut url = format!("{}/jobs", endpoint.trim_end_matches('/'));
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

    let job_page: JobPage =
        serde_json::from_str(&body).map_err(|e| CliError::InvalidBody(e.to_string()))?;

    if json_mode {
        println!("{}", body);
    } else {
        if job_page.items.is_empty() {
            println!("No jobs found.");
        } else {
            println!(
                "{:<40} {:>10} {:>15} {:>10}",
                "ID", "STATE", "CREATED", "TASKS"
            );
            println!("{}", "-".repeat(80));
            for job in &job_page.items {
                let id = job.id.as_deref().unwrap_or("unknown");
                let state = job.state.as_deref().unwrap_or("unknown");
                let created = job.created_at.as_deref().map(|s| &s[..10]).unwrap_or("-");
                println!(
                    "{:<40} {:>10} {:>15} {:>10}",
                    id, state, created, job.task_count
                );
            }
            println!("---");
            println!(
                "Page {}/{} ({} jobs total)",
                job_page.number, job_page.total_pages, job_page.total_items
            );
        }
    }

    Ok(body)
}

pub async fn job_create(endpoint: &str, body_json: &str, json_mode: bool) -> Result<String, CliError> {
    let url = format!("{}/jobs", endpoint.trim_end_matches('/'));

    let client = reqwest::Client::new();
    let response = client
        .post(&url)
        .header("Content-Type", "application/json")
        .body(body_json.to_string())
        .send()
        .await
        .map_err(CliError::Http)?;

    let status = response.status();

    let body = response
        .text()
        .await
        .map_err(|e| CliError::InvalidBody(e.to_string()))?;

    if status == reqwest::StatusCode::BAD_REQUEST {
        return Err(CliError::HttpStatus {
            status: status.as_u16(),
            reason: status
                .canonical_reason()
                .unwrap_or("Bad Request")
                .to_string(),
        });
    }

    if !status.is_success() {
        return Err(CliError::HttpStatus {
            status: status.as_u16(),
            reason: status.canonical_reason().unwrap_or("Unknown").to_string(),
        });
    }

    if json_mode {
        println!("{}", body);
    } else {
        let job: JobSummaryItem =
            serde_json::from_str(&body).map_err(|e| CliError::InvalidBody(e.to_string()))?;
        println!(
            "Job created: {} ({})",
            job.id.as_deref().unwrap_or("unknown"),
            job.name.as_deref().unwrap_or("unnamed")
        );
    }

    Ok(body)
}

pub async fn job_get(endpoint: &str, job_id: &str, json_mode: bool) -> Result<String, CliError> {
    let url = format!("{}/jobs/{}", endpoint.trim_end_matches('/'), job_id);

    let response = reqwest::get(&url).await.map_err(CliError::Http)?;

    let status = response.status();

    if status == reqwest::StatusCode::NOT_FOUND {
        return Err(CliError::NotFound(format!("job {} not found", job_id)));
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

    let job: JobDetail =
        serde_json::from_str(&body).map_err(|e| CliError::InvalidBody(e.to_string()))?;

    if json_mode {
        println!("{}", body);
    } else {
        println!("Job: {}", job.id.as_deref().unwrap_or("unknown"));
        if let Some(name) = &job.name {
            println!("Name: {}", name);
        }
        if let Some(desc) = &job.description {
            println!("Description: {}", desc);
        }
        if let Some(state) = &job.state {
            println!("State: {}", state);
        }
        if let Some(created) = &job.created_at {
            println!("Created: {}", created);
        }
        if let Some(started) = &job.started_at {
            println!("Started: {}", started);
        }
        if let Some(completed) = &job.completed_at {
            println!("Completed: {}", completed);
        }
        println!("Task Count: {}", job.task_count);
        println!("Position: {}", job.position);
        println!("Progress: {:.1}%", job.progress * 100.0);
        if let Some(output) = &job.output {
            println!("Output: {}", output);
        }
        if let Some(result) = &job.result {
            println!("Result: {}", result);
        }
        if let Some(error) = &job.error {
            println!("Error: {}", error);
        }
    }

    Ok(body)
}

pub async fn job_log(
    endpoint: &str,
    job_id: &str,
    page: Option<i64>,
    size: Option<i64>,
    json_mode: bool,
) -> Result<String, CliError> {
    let mut url = format!("{}/jobs/{}/log", endpoint.trim_end_matches('/'), job_id);
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
        return Err(CliError::NotFound(format!("job {} not found", job_id)));
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

    let log_page: JobLogPage =
        serde_json::from_str(&body).map_err(|e| CliError::InvalidBody(e.to_string()))?;

    if json_mode {
        println!("{}", body);
    } else {
        println!("Job: {}", job_id);
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

pub async fn job_cancel(endpoint: &str, job_id: &str, json_mode: bool) -> Result<String, CliError> {
    let url = format!("{}/jobs/{}/cancel", endpoint.trim_end_matches('/'), job_id);

    let client = reqwest::Client::new();
    let response = client
        .put(&url)
        .send()
        .await
        .map_err(CliError::Http)?;

    let status = response.status();

    if status == reqwest::StatusCode::NOT_FOUND {
        return Err(CliError::NotFound(format!("job {} not found", job_id)));
    }

    if status == reqwest::StatusCode::BAD_REQUEST {
        let body = response
            .text()
            .await
            .map_err(|e| CliError::InvalidBody(e.to_string()))?;
        return Err(CliError::ApiError {
            code: status.as_u16(),
            message: body,
        });
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

    if json_mode {
        println!("{}", body);
    } else {
        println!("Job '{}' cancelled.", job_id);
    }

    Ok(body)
}

pub async fn job_restart(endpoint: &str, job_id: &str, json_mode: bool) -> Result<String, CliError> {
    let url = format!("{}/jobs/{}/restart", endpoint.trim_end_matches('/'), job_id);

    let client = reqwest::Client::new();
    let response = client
        .put(&url)
        .send()
        .await
        .map_err(CliError::Http)?;

    let status = response.status();

    if status == reqwest::StatusCode::NOT_FOUND {
        return Err(CliError::NotFound(format!("job {} not found", job_id)));
    }

    if status == reqwest::StatusCode::BAD_REQUEST {
        let body = response
            .text()
            .await
            .map_err(|e| CliError::InvalidBody(e.to_string()))?;
        return Err(CliError::ApiError {
            code: status.as_u16(),
            message: body,
        });
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

    if json_mode {
        println!("{}", body);
    } else {
        println!("Job '{}' restarted.", job_id);
    }

    Ok(body)
}