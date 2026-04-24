//! Scheduled job command handlers
//!
//! HTTP client functions for scheduled job API operations.

use serde::Deserialize;

use crate::error::CliError;

#[derive(Debug, Deserialize)]
pub struct ScheduledJobPage {
    pub items: Vec<ScheduledJobSummaryItem>,
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
pub struct ScheduledJobSummaryItem {
    #[serde(default)]
    pub id: Option<String>,
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub state: Option<String>,
    #[serde(default)]
    pub cron: Option<String>,
    #[serde(default)]
    pub created_at: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ScheduledJobDetail {
    #[serde(default)]
    pub id: Option<String>,
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub cron: Option<String>,
    #[serde(default)]
    pub state: Option<String>,
    #[serde(default)]
    pub created_at: Option<String>,
    #[serde(default)]
    pub created_by: Option<serde_json::Value>,
    #[serde(default)]
    pub inputs: Option<serde_json::Value>,
    #[serde(default)]
    pub tasks: Option<Vec<serde_json::Value>>,
    #[serde(default)]
    pub defaults: Option<serde_json::Value>,
}

#[derive(Debug, Deserialize)]
pub struct StatusMessage {
    pub status: String,
}

pub async fn scheduled_job_list(
    endpoint: &str,
    page: Option<i64>,
    size: Option<i64>,
    json_mode: bool,
) -> Result<String, CliError> {
    let mut url = format!("{}/scheduled-jobs", endpoint.trim_end_matches('/'));
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

    let page: ScheduledJobPage =
        serde_json::from_str(&body).map_err(|e| CliError::InvalidBody(e.to_string()))?;

    if json_mode {
        println!("{}", body);
    } else {
        if page.items.is_empty() {
            println!("No scheduled jobs found.");
        } else {
            println!(
                "{:<40} {:>15} {:>10} {:>20}",
                "ID", "NAME", "STATE", "CRON"
            );
            println!("{}", "-".repeat(90));
            for job in &page.items {
                let id = job.id.as_deref().unwrap_or("unknown");
                let name = job.name.as_deref().unwrap_or("-");
                let state = job.state.as_deref().unwrap_or("unknown");
                let cron = job.cron.as_deref().unwrap_or("-");
                println!(
                    "{:<40} {:>15} {:>10} {:>20}",
                    &id[..id.len().min(40)],
                    &name[..name.len().min(15)],
                    state,
                    cron
                );
            }
            println!("---");
            println!(
                "Page {}/{} ({} scheduled jobs total)",
                page.number, page.total_pages, page.total_items
            );
        }
    }

    Ok(body)
}

pub async fn scheduled_job_create(
    endpoint: &str,
    body_json: &str,
    json_mode: bool,
) -> Result<String, CliError> {
    let url = format!("{}/scheduled-jobs", endpoint.trim_end_matches('/'));

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
        let job: ScheduledJobSummaryItem =
            serde_json::from_str(&body).map_err(|e| CliError::InvalidBody(e.to_string()))?;
        println!(
            "Scheduled job created: {} ({})",
            job.id.as_deref().unwrap_or("unknown"),
            job.name.as_deref().unwrap_or("unnamed")
        );
    }

    Ok(body)
}

pub async fn scheduled_job_get(
    endpoint: &str,
    job_id: &str,
    json_mode: bool,
) -> Result<String, CliError> {
    let url = format!(
        "{}/scheduled-jobs/{}",
        endpoint.trim_end_matches('/'),
        job_id
    );

    let response = reqwest::get(&url).await.map_err(CliError::Http)?;

    let status = response.status();

    if status == reqwest::StatusCode::NOT_FOUND {
        return Err(CliError::NotFound(format!(
            "scheduled job {} not found",
            job_id
        )));
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

    let job: ScheduledJobDetail =
        serde_json::from_str(&body).map_err(|e| CliError::InvalidBody(e.to_string()))?;

    if json_mode {
        println!("{}", body);
    } else {
        println!("Scheduled Job: {}", job.id.as_deref().unwrap_or("unknown"));
        if let Some(name) = &job.name {
            println!("Name: {}", name);
        }
        if let Some(desc) = &job.description {
            println!("Description: {}", desc);
        }
        if let Some(state) = &job.state {
            println!("State: {}", state);
        }
        if let Some(cron) = &job.cron {
            println!("Cron: {}", cron);
        }
        if let Some(created) = &job.created_at {
            println!("Created: {}", created);
        }
    }

    Ok(body)
}

pub async fn scheduled_job_delete(
    endpoint: &str,
    job_id: &str,
    json_mode: bool,
) -> Result<String, CliError> {
    let url = format!(
        "{}/scheduled-jobs/{}",
        endpoint.trim_end_matches('/'),
        job_id
    );

    let client = reqwest::Client::new();
    let response = client.delete(&url).send().await.map_err(CliError::Http)?;

    let status = response.status();

    if status == reqwest::StatusCode::NOT_FOUND {
        return Err(CliError::NotFound(format!(
            "scheduled job {} not found",
            job_id
        )));
    }

    if !status.is_success() {
        return Err(CliError::HttpStatus {
            status: status.as_u16(),
            reason: status.canonical_reason().unwrap_or("Unknown").to_string(),
        });
    }

    if json_mode {
        println!(r#"{{"type":"scheduled_job","id":"{}","deleted":true}}"#, job_id);
    } else {
        println!("Scheduled job '{}' deleted.", job_id);
    }

    Ok(format!(r#"{{"deleted":true,"id":"{}"}}"#, job_id))
}

pub async fn scheduled_job_pause(
    endpoint: &str,
    job_id: &str,
    json_mode: bool,
) -> Result<String, CliError> {
    let url = format!(
        "{}/scheduled-jobs/{}/pause",
        endpoint.trim_end_matches('/'),
        job_id
    );

    let client = reqwest::Client::new();
    let response = client
        .put(&url)
        .send()
        .await
        .map_err(CliError::Http)?;

    let status = response.status();

    if status == reqwest::StatusCode::NOT_FOUND {
        return Err(CliError::NotFound(format!(
            "scheduled job {} not found",
            job_id
        )));
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
        println!("Scheduled job '{}' paused.", job_id);
    }

    Ok(body)
}

pub async fn scheduled_job_resume(
    endpoint: &str,
    job_id: &str,
    json_mode: bool,
) -> Result<String, CliError> {
    let url = format!(
        "{}/scheduled-jobs/{}/resume",
        endpoint.trim_end_matches('/'),
        job_id
    );

    let client = reqwest::Client::new();
    let response = client
        .put(&url)
        .send()
        .await
        .map_err(CliError::Http)?;

    let status = response.status();

    if status == reqwest::StatusCode::NOT_FOUND {
        return Err(CliError::NotFound(format!(
            "scheduled job {} not found",
            job_id
        )));
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
        println!("Scheduled job '{}' resumed.", job_id);
    }

    Ok(body)
}