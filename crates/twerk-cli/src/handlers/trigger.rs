//! Trigger command handlers
//!
//! HTTP client functions for trigger API operations.

use serde::Deserialize;
use time::OffsetDateTime;

use crate::error::CliError;

#[derive(Debug, Deserialize)]
pub struct TriggerView {
    pub id: String,
    pub name: String,
    pub enabled: bool,
    pub event: String,
    #[serde(default)]
    pub condition: Option<String>,
    pub action: String,
    #[serde(default)]
    pub metadata: std::collections::HashMap<String, String>,
    pub version: u64,
    #[serde(with = "time::serde::rfc3339")]
    pub created_at: OffsetDateTime,
    #[serde(with = "time::serde::rfc3339")]
    pub updated_at: OffsetDateTime,
}

#[derive(Debug, Deserialize)]
pub struct TriggerErrorResponse {
    pub error: String,
    pub message: String,
    #[serde(default)]
    pub path_id: Option<String>,
    #[serde(default)]
    pub body_id: Option<String>,
}

pub async fn trigger_list(endpoint: &str, json_mode: bool) -> Result<String, CliError> {
    let url = format!("{}/api/v1/triggers", endpoint.trim_end_matches('/'));

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

    let triggers: Vec<TriggerView> =
        serde_json::from_str(&body).map_err(|e| CliError::InvalidBody(e.to_string()))?;

    if json_mode {
        println!("{}", body);
    } else {
        if triggers.is_empty() {
            println!("No triggers found.");
        } else {
            println!(
                "{:<20} {:<30} {:<15} {:<10}",
                "ID", "NAME", "EVENT", "ENABLED"
            );
            println!("{}", "-".repeat(80));
            for t in &triggers {
                println!(
                    "{:<20} {:<30} {:<15} {:<10}",
                    t.id, t.name, t.event, t.enabled
                );
            }
        }
    }

    Ok(body)
}

pub async fn trigger_get(endpoint: &str, id: &str, json_mode: bool) -> Result<String, CliError> {
    let url = format!("{}/api/v1/triggers/{}", endpoint.trim_end_matches('/'), id);

    let response = reqwest::get(&url).await.map_err(CliError::Http)?;

    let status = response.status();

    let body = response
        .text()
        .await
        .map_err(|e| CliError::InvalidBody(e.to_string()))?;

    if status == reqwest::StatusCode::NOT_FOUND {
        if let Ok(err_resp) = serde_json::from_str::<TriggerErrorResponse>(&body) {
            return Err(CliError::ApiError {
                code: status.as_u16(),
                message: err_resp.message,
            });
        }
        return Err(CliError::NotFound(format!("trigger {} not found", id)));
    }

    if status == reqwest::StatusCode::BAD_REQUEST {
        if let Ok(err_resp) = serde_json::from_str::<TriggerErrorResponse>(&body) {
            return Err(CliError::ApiError {
                code: status.as_u16(),
                message: err_resp.message,
            });
        }
    }

    if !status.is_success() {
        return Err(CliError::HttpStatus {
            status: status.as_u16(),
            reason: status.canonical_reason().unwrap_or("Unknown").to_string(),
        });
    }

    let trigger: TriggerView =
        serde_json::from_str(&body).map_err(|e| CliError::InvalidBody(e.to_string()))?;

    if json_mode {
        println!("{}", body);
    } else {
        println!("Trigger: {}", trigger.id);
        println!("Name: {}", trigger.name);
        println!("Enabled: {}", trigger.enabled);
        println!("Event: {}", trigger.event);
        if let Some(condition) = &trigger.condition {
            println!("Condition: {}", condition);
        }
        println!("Action: {}", trigger.action);
        if !trigger.metadata.is_empty() {
            println!("Metadata:");
            for (k, v) in &trigger.metadata {
                println!("  {}: {}", k, v);
            }
        }
        println!("Version: {}", trigger.version);
        println!("Created: {}", trigger.created_at);
        println!("Updated: {}", trigger.updated_at);
    }

    Ok(body)
}

pub async fn trigger_create(
    endpoint: &str,
    body_json: &str,
    json_mode: bool,
) -> Result<String, CliError> {
    let url = format!("{}/api/v1/triggers", endpoint.trim_end_matches('/'));

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
        if let Ok(err_resp) = serde_json::from_str::<TriggerErrorResponse>(&body) {
            return Err(CliError::ApiError {
                code: status.as_u16(),
                message: err_resp.message,
            });
        }
    }

    if status == reqwest::StatusCode::CREATED {
        let trigger: TriggerView =
            serde_json::from_str(&body).map_err(|e| CliError::InvalidBody(e.to_string()))?;
        if json_mode {
            println!("{}", body);
        } else {
            println!("Trigger created: {}", trigger.id);
        }
        return Ok(body);
    }

    if !status.is_success() {
        return Err(CliError::HttpStatus {
            status: status.as_u16(),
            reason: status.canonical_reason().unwrap_or("Unknown").to_string(),
        });
    }

    Ok(body)
}

pub async fn trigger_update(
    endpoint: &str,
    id: &str,
    body_json: &str,
    json_mode: bool,
) -> Result<String, CliError> {
    let url = format!("{}/api/v1/triggers/{}", endpoint.trim_end_matches('/'), id);

    let client = reqwest::Client::new();
    let response = client
        .put(&url)
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
        if let Ok(err_resp) = serde_json::from_str::<TriggerErrorResponse>(&body) {
            return Err(CliError::ApiError {
                code: status.as_u16(),
                message: err_resp.message,
            });
        }
    }

    if status == reqwest::StatusCode::NOT_FOUND {
        if let Ok(err_resp) = serde_json::from_str::<TriggerErrorResponse>(&body) {
            return Err(CliError::ApiError {
                code: status.as_u16(),
                message: err_resp.message,
            });
        }
    }

    if status == reqwest::StatusCode::CONFLICT {
        if let Ok(err_resp) = serde_json::from_str::<TriggerErrorResponse>(&body) {
            return Err(CliError::ApiError {
                code: status.as_u16(),
                message: err_resp.message,
            });
        }
    }

    if status.is_success() {
        let trigger: TriggerView =
            serde_json::from_str(&body).map_err(|e| CliError::InvalidBody(e.to_string()))?;
        if json_mode {
            println!("{}", body);
        } else {
            println!("Trigger updated: {}", trigger.id);
        }
        return Ok(body);
    }

    Err(CliError::HttpStatus {
        status: status.as_u16(),
        reason: status.canonical_reason().unwrap_or("Unknown").to_string(),
    })
}

pub async fn trigger_delete(endpoint: &str, id: &str, json_mode: bool) -> Result<String, CliError> {
    let url = format!("{}/api/v1/triggers/{}", endpoint.trim_end_matches('/'), id);

    let client = reqwest::Client::new();
    let response = client.delete(&url).send().await.map_err(CliError::Http)?;

    let status = response.status();
    let body = response
        .text()
        .await
        .map_err(|e| CliError::InvalidBody(e.to_string()))?;

    if status == reqwest::StatusCode::NOT_FOUND {
        if let Ok(err_resp) = serde_json::from_str::<TriggerErrorResponse>(&body) {
            return Err(CliError::ApiError {
                code: status.as_u16(),
                message: err_resp.message,
            });
        }
        return Err(CliError::NotFound(format!("trigger {} not found", id)));
    }

    if status == reqwest::StatusCode::BAD_REQUEST {
        if let Ok(err_resp) = serde_json::from_str::<TriggerErrorResponse>(&body) {
            return Err(CliError::ApiError {
                code: status.as_u16(),
                message: err_resp.message,
            });
        }
    }

    if status == reqwest::StatusCode::NO_CONTENT || status.is_success() {
        if json_mode {
            if !body.is_empty() {
                println!("{}", body);
            }
        } else {
            println!("Trigger '{}' deleted.", id);
        }
        return Ok(body);
    }

    Err(CliError::HttpStatus {
        status: status.as_u16(),
        reason: status.canonical_reason().unwrap_or("Unknown").to_string(),
    })
}
