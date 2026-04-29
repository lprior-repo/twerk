//! User command handlers
//!
//! HTTP client functions for user API operations.

use crate::error::CliError;

#[derive(Debug, serde::Deserialize)]
struct ApiErrorResponse {
    message: String,
}

fn api_error_message(body: &str) -> Option<String> {
    serde_json::from_str::<ApiErrorResponse>(body)
        .ok()
        .map(|response| response.message)
}

pub async fn user_create(
    endpoint: &str,
    username: &str,
    password: &str,
    json_mode: bool,
) -> Result<String, CliError> {
    let url = format!("{}/users", endpoint.trim_end_matches('/'));

    let body_json = serde_json::json!({ "username": username, "password": password }).to_string();

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
        if let Some(message) = api_error_message(&body) {
            return Err(CliError::ApiError {
                code: status.as_u16(),
                message,
            });
        }
        return Err(CliError::HttpStatus {
            status: status.as_u16(),
            reason: status.canonical_reason().map_or_else(
                || "Bad Request".to_string(),
                std::string::ToString::to_string,
            ),
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
            if !body.is_empty() {
                println!("{}", body);
            }
        } else {
            println!("User '{}' created successfully.", username);
        }
        return Ok(body);
    }

    if !status.is_success() {
        return Err(CliError::HttpStatus {
            status: status.as_u16(),
            reason: status
                .canonical_reason()
                .map_or_else(|| "Unknown".to_string(), std::string::ToString::to_string),
        });
    }

    Ok(body)
}
