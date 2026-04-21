use axum::body::Bytes;
use axum::http::HeaderMap;
use serde_json::Value;

use crate::api::content_type::normalized_content_type;

use super::super::domain::{
    validate_trigger_update, TriggerId, TriggerUpdateError, TriggerUpdateRequest,
    MALFORMED_JSON_MSG,
};

fn as_optional_string(value: Option<&Value>) -> Option<String> {
    value
        .and_then(Value::as_str)
        .map(std::string::ToString::to_string)
}

fn as_optional_string_map(
    value: Option<&Value>,
) -> Option<std::collections::HashMap<String, String>> {
    value.and_then(Value::as_object).map(|map| {
        map.iter()
            .filter_map(|(key, value)| value.as_str().map(|inner| (key.clone(), inner.to_string())))
            .collect()
    })
}

/// Decode a JSON trigger request body into the internal request type.
///
/// # Errors
/// Returns an error when the body is not valid JSON or is not a JSON object.
pub fn decode_trigger_update_request(
    body: &Bytes,
) -> Result<TriggerUpdateRequest, TriggerUpdateError> {
    let parsed_value: Value = serde_json::from_slice(body)
        .map_err(|_| TriggerUpdateError::MalformedJson(MALFORMED_JSON_MSG.to_string()))?;

    let object = parsed_value
        .as_object()
        .ok_or_else(|| TriggerUpdateError::MalformedJson(MALFORMED_JSON_MSG.to_string()))?;

    Ok(TriggerUpdateRequest {
        name: as_optional_string(object.get("name")).unwrap_or_default(),
        enabled: object
            .get("enabled")
            .and_then(Value::as_bool)
            .unwrap_or(false),
        event: as_optional_string(object.get("event")).unwrap_or_default(),
        condition: as_optional_string(object.get("condition")),
        action: as_optional_string(object.get("action")).unwrap_or_default(),
        metadata: as_optional_string_map(object.get("metadata")),
        id: as_optional_string(object.get("id")),
        version: object.get("version").and_then(Value::as_u64),
    })
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
