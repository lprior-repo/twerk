#![deny(clippy::unwrap_used)]
#![warn(clippy::pedantic)]

use std::collections::HashMap;

use serde::{Deserialize, Serialize};
use thiserror::Error;
use time::OffsetDateTime;

pub const TRIGGER_ID_MIN_LEN: usize = 1;
pub const TRIGGER_ID_MAX_LEN: usize = 1000;
pub const TRIGGER_FIELD_MAX_LEN: usize = 64;

pub const NAME_REQUIRED_MSG: &str = "name must be non-empty after trim";
pub const EVENT_REQUIRED_MSG: &str = "event must be non-empty after trim";
pub const ACTION_REQUIRED_MSG: &str = "action must be non-empty after trim";
pub const METADATA_KEY_MSG: &str = "metadata key must be non-empty ASCII";
pub const UPDATED_AT_BACKWARDS_MSG: &str = "updated_at cannot move backwards";
pub const MALFORMED_JSON_MSG: &str = "malformed JSON body";
pub const SERIALIZATION_MSG: &str = "failed to serialize response";

// ---------------------------------------------------------------------------
// TriggerId
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct TriggerId(String);

impl TriggerId {
    /// Parse and validate a trigger identifier.
    ///
    /// # Errors
    /// Returns `InvalidIdFormat` when length or character rules fail.
    pub fn parse(raw: &str) -> Result<Self, TriggerUpdateError> {
        let len = raw.len();
        let has_valid_chars = raw
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-');
        if !(TRIGGER_ID_MIN_LEN..=TRIGGER_ID_MAX_LEN).contains(&len) || !has_valid_chars {
            return Err(TriggerUpdateError::InvalidIdFormat(raw.to_string()));
        }
        Ok(Self(raw.to_string()))
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl From<&str> for TriggerId {
    fn from(value: &str) -> Self {
        Self(value.to_string())
    }
}

// ---------------------------------------------------------------------------
// Trigger types
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq, utoipa::ToSchema)]
pub struct TriggerUpdateRequest {
    pub name: String,
    pub enabled: bool,
    pub event: String,
    pub condition: Option<String>,
    pub action: String,
    pub metadata: Option<HashMap<String, String>>,
    pub id: Option<String>,
    #[serde(default)]
    pub version: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Trigger {
    pub id: TriggerId,
    pub name: String,
    pub enabled: bool,
    pub event: String,
    pub condition: Option<String>,
    pub action: String,
    pub metadata: HashMap<String, String>,
    pub version: u64,
    pub created_at: OffsetDateTime,
    pub updated_at: OffsetDateTime,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, utoipa::ToSchema)]
pub struct TriggerView {
    pub id: String,
    pub name: String,
    pub enabled: bool,
    pub event: String,
    pub condition: Option<String>,
    pub action: String,
    pub metadata: HashMap<String, String>,
    pub version: u64,
    pub created_at: OffsetDateTime,
    pub updated_at: OffsetDateTime,
}

impl From<Trigger> for TriggerView {
    fn from(value: Trigger) -> Self {
        Self {
            id: value.id.as_str().to_string(),
            name: value.name,
            enabled: value.enabled,
            event: value.event,
            condition: value.condition,
            action: value.action,
            metadata: value.metadata,
            version: value.version,
            created_at: value.created_at,
            updated_at: value.updated_at,
        }
    }
}

// ---------------------------------------------------------------------------
// Error types
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Error, PartialEq, Eq)]
pub enum TriggerUpdateError {
    #[error("invalid id format: {0}")]
    InvalidIdFormat(String),
    #[error("unsupported content type: {0}")]
    UnsupportedContentType(String),
    #[error("malformed json: {0}")]
    MalformedJson(String),
    #[error("validation failed: {0}")]
    ValidationFailed(String),
    #[error("id mismatch")]
    IdMismatch { path_id: String, body_id: String },
    #[error("trigger not found: {0}")]
    TriggerNotFound(String),
    #[error("version conflict: {0}")]
    VersionConflict(String),
    #[error("persistence error: {0}")]
    Persistence(String),
    #[error("serialization error: {0}")]
    Serialization(String),
}

// ---------------------------------------------------------------------------
// Validation functions
// ---------------------------------------------------------------------------

fn validate_required_field(
    value: &str,
    required_msg: &str,
    field_name: &str,
) -> Result<(), TriggerUpdateError> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return Err(TriggerUpdateError::ValidationFailed(
            required_msg.to_string(),
        ));
    }
    if trimmed.len() > TRIGGER_FIELD_MAX_LEN {
        return Err(TriggerUpdateError::ValidationFailed(format!(
            "{field_name} exceeds max length"
        )));
    }
    Ok(())
}

fn normalize_required_field(value: &str) -> String {
    value.trim().to_string()
}

fn validate_metadata(metadata: Option<&HashMap<String, String>>) -> Result<(), TriggerUpdateError> {
    let invalid = metadata
        .into_iter()
        .flat_map(|map| map.keys())
        .any(|key| key.is_empty() || !key.is_ascii());
    if invalid {
        return Err(TriggerUpdateError::ValidationFailed(
            METADATA_KEY_MSG.to_string(),
        ));
    }
    Ok(())
}

/// Validate update request against contract preconditions.
///
/// # Errors
/// Returns field validation, id format, or id mismatch errors.
pub fn validate_trigger_update(
    path_id: &str,
    req: &TriggerUpdateRequest,
) -> Result<TriggerId, TriggerUpdateError> {
    let parsed_id = TriggerId::parse(path_id)?;

    validate_required_field(&req.name, NAME_REQUIRED_MSG, "name")?;
    validate_required_field(&req.event, EVENT_REQUIRED_MSG, "event")?;
    validate_required_field(&req.action, ACTION_REQUIRED_MSG, "action")?;
    validate_metadata(req.metadata.as_ref())?;

    match req.id.as_ref() {
        Some(body_id) if body_id != path_id => Err(TriggerUpdateError::IdMismatch {
            path_id: path_id.to_string(),
            body_id: body_id.clone(),
        }),
        _ => Ok(parsed_id),
    }
}

/// Validate create request fields without path ID check.
///
/// # Errors
/// Returns field validation or id format errors.
pub fn validate_trigger_create(req: &TriggerUpdateRequest) -> Result<(), TriggerUpdateError> {
    validate_required_field(&req.name, NAME_REQUIRED_MSG, "name")?;
    validate_required_field(&req.event, EVENT_REQUIRED_MSG, "event")?;
    validate_required_field(&req.action, ACTION_REQUIRED_MSG, "action")?;
    validate_metadata(req.metadata.as_ref())?;
    if let Some(ref id) = req.id {
        TriggerId::parse(id).map_err(|e| TriggerUpdateError::InvalidIdFormat(e.to_string()))?;
    }
    Ok(())
}

/// Validate that the update timestamp is not before the current timestamp.
fn validate_timestamp_monotonicity(
    now_utc: OffsetDateTime,
    current_updated_at: OffsetDateTime,
) -> Result<(), TriggerUpdateError> {
    if now_utc < current_updated_at {
        return Err(TriggerUpdateError::ValidationFailed(
            UPDATED_AT_BACKWARDS_MSG.to_string(),
        ));
    }
    Ok(())
}

/// Apply validated request to current trigger state.
///
/// # Errors
/// Returns validation errors for malformed fields or backward timestamps.
/// Returns VersionConflict if the request version does not match the stored version.
pub fn apply_trigger_update(
    current: Trigger,
    req: TriggerUpdateRequest,
    now_utc: OffsetDateTime,
) -> Result<Trigger, TriggerUpdateError> {
    if let Some(req_version) = req.version {
        if req_version != current.version {
            return Err(TriggerUpdateError::VersionConflict(
                "stale version supplied".to_string(),
            ));
        }
    }
    validate_required_field(&req.name, NAME_REQUIRED_MSG, "name")?;
    validate_required_field(&req.event, EVENT_REQUIRED_MSG, "event")?;
    validate_required_field(&req.action, ACTION_REQUIRED_MSG, "action")?;
    validate_metadata(req.metadata.as_ref())?;
    validate_timestamp_monotonicity(now_utc, current.updated_at)?;

    Ok(Trigger {
        id: current.id,
        name: normalize_required_field(&req.name),
        enabled: req.enabled,
        event: normalize_required_field(&req.event),
        condition: req.condition,
        action: normalize_required_field(&req.action),
        metadata: req.metadata.unwrap_or_default(),
        version: current.version.saturating_add(1),
        created_at: current.created_at,
        updated_at: now_utc,
    })
}
