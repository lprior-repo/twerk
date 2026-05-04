//! Job command handlers
//!
//! HTTP client functions for job API operations.

use serde::Deserialize;

use crate::error::CliError;

#[derive(Debug, Deserialize)]
pub struct JobResponse {
    #[serde(default)]
    pub id: Option<String>,
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub state: Option<String>,
    #[serde(default)]
    pub created_at: Option<String>,
    #[serde(default)]
    pub created_by: Option<String>,
    #[serde(default)]
    pub task_count: Option<i64>,
    #[serde(default)]
    pub error: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct JobLogEntry {
    #[serde(default)]
    pub contents: Option<String>,
    #[serde(default)]
    pub timestamp: Option<String>,
}

pub async fn job_list(endpoint: &str, json_mode: bool) -> Result<String, CliError> {
    let url = format!("{}/jobs", endpoint.trim_end_matches('/'));

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

    if json_mode {
        println!("{}", body);
    } else {
        println!("Job list retrieved");
    }

    Ok(body)
}

pub async fn job_create(endpoint: &str, body: &str, json_mode: bool) -> Result<String, CliError> {
    let url = format!("{}/jobs", endpoint.trim_end_matches('/'));

    let response = reqwest::Client::new()
        .post(&url)
        .header("Content-Type", "application/json")
        .body(body.to_string())
        .send()
        .await
        .map_err(CliError::Http)?;

    let status = response.status();

    if !status.is_success() {
        return Err(CliError::HttpStatus {
            status: status.as_u16(),
            reason: status.canonical_reason().unwrap_or("Unknown").to_string(),
        });
    }

    let response_body = response
        .text()
        .await
        .map_err(|e| CliError::InvalidBody(e.to_string()))?;

    if json_mode {
        println!("{}", response_body);
    } else {
        println!("Job created");
    }

    Ok(response_body)
}

pub async fn job_get(endpoint: &str, id: &str, json_mode: bool) -> Result<String, CliError> {
    let url = format!("{}/jobs/{}", endpoint.trim_end_matches('/'), id);

    let response = reqwest::get(&url).await.map_err(CliError::Http)?;

    let status = response.status();

    if status == reqwest::StatusCode::NOT_FOUND {
        return Err(CliError::NotFound(format!("job {} not found", id)));
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
        println!("Job: {}", id);
    }

    Ok(body)
}

pub async fn job_log(endpoint: &str, id: &str, json_mode: bool) -> Result<String, CliError> {
    let url = format!("{}/jobs/{}/log", endpoint.trim_end_matches('/'), id);

    let response = reqwest::get(&url).await.map_err(CliError::Http)?;

    let status = response.status();

    if status == reqwest::StatusCode::NOT_FOUND {
        return Err(CliError::NotFound(format!("job {} not found", id)));
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
        println!("Job log for: {}", id);
    }

    Ok(body)
}

pub async fn job_cancel(endpoint: &str, id: &str, json_mode: bool) -> Result<String, CliError> {
    let url = format!("{}/jobs/{}/cancel", endpoint.trim_end_matches('/'), id);

    let response = reqwest::Client::new()
        .put(&url)
        .send()
        .await
        .map_err(CliError::Http)?;

    let status = response.status();

    if status == reqwest::StatusCode::NOT_FOUND {
        return Err(CliError::NotFound(format!("job {} not found", id)));
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
        println!("Job {} cancelled", id);
    }

    Ok(body)
}

pub async fn job_restart(endpoint: &str, id: &str, json_mode: bool) -> Result<String, CliError> {
    let url = format!("{}/jobs/{}/restart", endpoint.trim_end_matches('/'), id);

    let response = reqwest::Client::new()
        .put(&url)
        .send()
        .await
        .map_err(CliError::Http)?;

    let status = response.status();

    if status == reqwest::StatusCode::NOT_FOUND {
        return Err(CliError::NotFound(format!("job {} not found", id)));
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
        println!("Job {} restarted", id);
    }

    Ok(body)
}
