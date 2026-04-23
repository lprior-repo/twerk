//! User command handlers
//!
//! HTTP client functions for user API operations.

use serde::Deserialize;

use crate::error::CliError;

#[derive(Debug, Deserialize)]
pub struct User {
    pub id: String,
    pub username: String,
    #[serde(default)]
    pub email: Option<String>,
    #[serde(default)]
    pub created_at: Option<String>,
    #[serde(default)]
    pub is_active: Option<bool>,
}

#[derive(Debug, Deserialize)]
pub struct UserCreateResponse {
    pub user: User,
    #[serde(default)]
    pub message: Option<String>,
}

pub async fn user_create(
    endpoint: &str,
    username: &str,
    json_mode: bool,
) -> Result<String, CliError> {
    let url = format!("{}/users", endpoint.trim_end_matches('/'));

    let body_json = serde_json::json!({ "username": username }).to_string();

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

    if status == reqwest::StatusCode::CREATED {
        let _user_resp: UserCreateResponse =
            serde_json::from_str(&body).map_err(|e| CliError::InvalidBody(e.to_string()))?;
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
