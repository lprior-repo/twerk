//! Health check command
//!
//! Performs an HTTP health check against the Twerk endpoint.

use serde::Deserialize;

use super::error::CliError;

/// Health check response body
#[derive(Debug, Deserialize)]
pub struct HealthResponse {
    /// The health status
    pub status: String,
}

/// Perform a health check against the specified endpoint
///
/// # Arguments
///
/// * `endpoint` - The base URL of the Twerk service
/// * `json_mode` - If true, output JSON instead of human-readable text
///
/// # Errors
///
/// Returns [`CliError::Http`] if the request fails.
/// Returns [`CliError::HealthFailed`] if the status code is not 200.
/// Returns [`CliError::InvalidBody`] if the response body cannot be parsed.
pub async fn health_check(endpoint: &str, json_mode: bool) -> Result<String, CliError> {
    let url = format!("{}/health", endpoint.trim_end_matches('/'));

    let response = reqwest::get(&url).await.map_err(CliError::Http)?;

    let status = response.status();

    if status != reqwest::StatusCode::OK {
        return Err(CliError::HealthFailed {
            status: status.as_u16(),
        });
    }

    let body = response
        .text()
        .await
        .map_err(|e| CliError::InvalidBody(e.to_string()))?;

    let health_response: HealthResponse =
        serde_json::from_str(&body).map_err(|e| CliError::InvalidBody(e.to_string()))?;

    if json_mode {
        println!(
            r#"{{"type":"health","endpoint":"{}","status":"{}","ok":true}}"#,
            endpoint, health_response.status
        );
    } else {
        println!("Status: {}", health_response.status);
    }

    Ok(health_response.status)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[allow(clippy::unwrap_used)]
    #[test]
    fn test_health_response_deserialize() -> Result<(), Box<dyn std::error::Error>> {
        let json = r#"{"status": "ok"}"#;
        let response: HealthResponse = serde_json::from_str(json)?;
        assert_eq!(response.status, "ok");
        Ok(())
    }

    #[allow(clippy::unwrap_used)]
    #[test]
    fn test_health_response_deserialize_with_extra_fields() -> Result<(), Box<dyn std::error::Error>>
    {
        let json = r#"{"status": "ok", "extra": "ignored"}"#;
        let response: HealthResponse = serde_json::from_str(json)?;
        assert_eq!(response.status, "ok");
        Ok(())
    }

    #[test]
    fn test_health_response_debug() {
        let response = HealthResponse {
            status: "ok".to_string(),
        };
        let debug_str = format!("{:?}", response);
        assert!(debug_str.contains("ok"));
    }
}
