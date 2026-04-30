//! Shared types and utilities for handler modules
//!
//! Contains URL-encoding helpers and shared response types used across handlers.

use serde::Deserialize;
use time::OffsetDateTime;

/// URL-encodes a path segment for safe inclusion in API URLs.
pub fn encode_path_segment(segment: &str) -> String {
    percent_encoding::utf8_percent_encode(segment, percent_encoding::NON_ALPHANUMERIC).to_string()
}

/// Standard error response returned by the API on failures.
#[derive(Debug, Deserialize)]
pub struct TriggerErrorResponse {
    pub error: String,
    pub message: String,
    #[serde(default)]
    pub path_id: Option<String>,
    #[serde(default)]
    pub body_id: Option<String>,
}

/// View model for a trigger resource returned by the API.
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
