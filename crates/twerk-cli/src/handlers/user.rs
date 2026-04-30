//! User command handlers
//!
//! HTTP client functions for user API operations.

use crate::error::CliError;
use crate::handlers::common::TriggerErrorResponse;

pub async fn user_create(
    endpoint: &str,
    username: &str,
    password: &str,
    json_mode: bool,
) -> Result<String, CliError> {
    let url = format!("{}/users", endpoint.trim_end_matches('/'));

    let body_json =
        serde_json::json!({ "username": username, "password": password }).to_string();

    let client = reqwest::Client::new();
    let response = client
        .post(&url)
        .header("Content-Type", "application/json")
        .body(body_json)
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
        return Err(CliError::HttpStatus {
            status: status.as_u16(),
            reason: status
                .canonical_reason()
                .unwrap_or("Bad Request")
                .to_string(),
        });
    }

    if status == reqwest::StatusCode::CONFLICT {
        return Err(CliError::ApiError {
            code: status.as_u16(),
            message: format!("user '{}' already exists", username),
        });
    }

    if status == reqwest::StatusCode::OK {
        if json_mode {
            println!("{}", body);
        } else {
            println!("User '{}' created successfully.", username);
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
