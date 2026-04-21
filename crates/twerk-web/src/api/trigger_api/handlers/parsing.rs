use std::collections::HashMap;

use axum::body::Bytes;
use axum::http::HeaderMap;
use serde::Deserialize;

use crate::api::content_type::normalized_content_type;

use super::super::domain::{
    validate_trigger_update, TriggerId, TriggerUpdateError, TriggerUpdateRequest,
    ACTION_REQUIRED_MSG, EVENT_REQUIRED_MSG, MALFORMED_JSON_MSG, NAME_REQUIRED_MSG,
};

const ENABLED_REQUIRED_MSG: &str = "enabled is required";

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct TriggerUpdatePayload {
    name: Option<String>,
    enabled: Option<bool>,
    event: Option<String>,
    condition: Option<String>,
    action: Option<String>,
    metadata: Option<HashMap<String, String>>,
    id: Option<String>,
    version: Option<u64>,
}

fn required_field(value: Option<String>, message: &str) -> Result<String, TriggerUpdateError> {
    value.ok_or_else(|| TriggerUpdateError::ValidationFailed(message.to_string()))
}

impl TryFrom<TriggerUpdatePayload> for TriggerUpdateRequest {
    type Error = TriggerUpdateError;

    fn try_from(value: TriggerUpdatePayload) -> Result<Self, Self::Error> {
        Ok(Self {
            name: required_field(value.name, NAME_REQUIRED_MSG)?,
            enabled: value.enabled.ok_or_else(|| {
                TriggerUpdateError::ValidationFailed(ENABLED_REQUIRED_MSG.to_string())
            })?,
            event: required_field(value.event, EVENT_REQUIRED_MSG)?,
            condition: value.condition,
            action: required_field(value.action, ACTION_REQUIRED_MSG)?,
            metadata: value.metadata,
            id: value.id,
            version: value.version,
        })
    }
}

/// Decode a JSON trigger request body into the internal request type.
///
/// # Errors
/// Returns an error when the body is not valid JSON or is not a JSON object.
pub fn decode_trigger_update_request(
    body: &Bytes,
) -> Result<TriggerUpdateRequest, TriggerUpdateError> {
    let payload = serde_json::from_slice::<TriggerUpdatePayload>(body)
        .map_err(|_| TriggerUpdateError::MalformedJson(MALFORMED_JSON_MSG.to_string()))?;
    payload.try_into()
}

/// Validate that the trigger request content type is JSON.
///
/// # Errors
/// Returns an error when the request content type is not `application/json`.
pub fn parse_content_type(headers: &HeaderMap) -> Result<(), TriggerUpdateError> {
    let content_type = normalized_content_type(headers);
    if content_type == "application/json" {
        Ok(())
    } else {
        Err(TriggerUpdateError::UnsupportedContentType(content_type))
    }
}

fn check_version_constraints(req: &TriggerUpdateRequest) -> Option<TriggerUpdateError> {
    if req.version == Some(0) {
        Some(TriggerUpdateError::VersionConflict(
            "stale version supplied".to_string(),
        ))
    } else {
        None
    }
}

/// Parse and validate a trigger update request before it reaches the datastore.
///
/// # Errors
/// Returns an error when the content type is unsupported, the JSON payload is malformed, the path
/// and body identifiers do not validate, or the supplied version is stale.
pub fn prepare_update(
    headers: &HeaderMap,
    body: &Bytes,
    path_id: &str,
) -> Result<(TriggerId, TriggerUpdateRequest), TriggerUpdateError> {
    parse_content_type(headers)?;
    let request = decode_trigger_update_request(body)?;
    let trigger_id = validate_trigger_update(path_id, &request)?;
    check_version_constraints(&request).map_or(Ok(()), Err)?;
    Ok((trigger_id, request))
}
