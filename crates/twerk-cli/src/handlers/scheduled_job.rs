//! Scheduled job command handlers
//!
//! HTTP client functions for scheduled job API operations.

use serde::Deserialize;

use crate::error::CliError;

#[derive(Debug, Deserialize)]
pub struct ScheduledJobResponse {
    #[serde(default)]
    pub id: Option<String>,
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub state: Option<String>,
    #[serde(default)]
    pub cron: Option<String>,
    #[serde(default)]
    pub created_at: Option<String>,
}

pub async fn scheduled_job_list(endpoint: &str, json_mode: bool) -> Result<String, CliError> {
    let url = format!("{}/scheduled-jobs", endpoint.trim_end_matches('/'));

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
        println!("Scheduled job list retrieved");
    }

    Ok(body)
}

pub async fn scheduled_job_create(endpoint: &str, body: &str, json_mode: bool) -> Result<String, CliError> {
    let url = format!("{}/scheduled-jobs", endpoint.trim_end_matches('/'));

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
        println!("Scheduled job created");
    }

    Ok(response_body)
}

pub async fn scheduled_job_get(endpoint: &str, id: &str, json_mode: bool) -> Result<String, CliError> {
    let url = format!("{}/scheduled-jobs/{}", endpoint.trim_end_matches('/'), id);

    let response = reqwest::get(&url).await.map_err(CliError::Http)?;

    let status = response.status();

    if status == reqwest::StatusCode::NOT_FOUND {
        return Err(CliError::NotFound(format!("scheduled job {} not found", id)));
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
        println!("Scheduled job: {}", id);
    }

    Ok(body)
}

pub async fn scheduled_job_delete(endpoint: &str, id: &str, json_mode: bool) -> Result<String, CliError> {
    let url = format!("{}/scheduled-jobs/{}", endpoint.trim_end_matches('/'), id);

    let response = reqwest::Client::new()
        .delete(&url)
        .send()
        .await
        .map_err(CliError::Http)?;

    let status = response.status();

    if status == reqwest::StatusCode::NOT_FOUND {
        return Err(CliError::NotFound(format!("scheduled job {} not found", id)));
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
        println!("Scheduled job {} deleted", id);
    }

    Ok(body)
}

pub async fn scheduled_job_pause(endpoint: &str, id: &str, json_mode: bool) -> Result<String, CliError> {
    let url = format!("{}/scheduled-jobs/{}/pause", endpoint.trim_end_matches('/'), id);

    let response = reqwest::Client::new()
        .put(&url)
        .send()
        .await
        .map_err(CliError::Http)?;

    let status = response.status();

    if status == reqwest::StatusCode::NOT_FOUND {
        return Err(CliError::NotFound(format!("scheduled job {} not found", id)));
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
        println!("Scheduled job {} paused", id);
    }

    Ok(body)
}

pub async fn scheduled_job_resume(endpoint: &str, id: &str, json_mode: bool) -> Result<String, CliError> {
    let url = format!("{}/scheduled-jobs/{}/resume", endpoint.trim_end_matches('/'), id);

    let response = reqwest::Client::new()
        .put(&url)
        .send()
        .await
        .map_err(CliError::Http)?;

    let status = response.status();

    if status == reqwest::StatusCode::NOT_FOUND {
        return Err(CliError::NotFound(format!("scheduled job {} not found", id)));
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
        println!("Scheduled job {} resumed", id);
    }

    Ok(body)
}