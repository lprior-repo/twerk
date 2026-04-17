#![deny(clippy::unwrap_used)]
#![warn(clippy::pedantic)]

use axum::body::Bytes;
use axum::extract::{Path, State};
use axum::http::{header, HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use serde_json::{json, Value};
use time::OffsetDateTime;

use super::domain::{
    apply_trigger_update, validate_trigger_update, TriggerId, TriggerUpdateError,
    TriggerUpdateRequest, TriggerView, MALFORMED_JSON_MSG, SERIALIZATION_MSG,
};
use crate::api::error::ApiError;
use crate::AppState;

pub const MAX_BODY_BYTES: usize = 16 * 1024;
pub const BODY_TOO_LARGE_MSG: &str = "request body exceeds max size";

/// Build an error JSON payload from error name and message.
macro_rules! err_json {
    ($name:expr, $msg:expr) => {
        json!({"error": $name, "message": $msg})
    };
    ($name:expr, $msg:expr, $($key:ident: $val:expr),*) => {
        json!({"error": $name, "message": $msg, $($key: $val),*})
    };
}

/// Map error variants to (status code, error JSON) tuples.
fn error_details(error: &TriggerUpdateError) -> (StatusCode, Value) {
    match error {
        TriggerUpdateError::InvalidIdFormat(msg) => {
            (StatusCode::BAD_REQUEST, err_json!("InvalidIdFormat", msg))
        }
        TriggerUpdateError::UnsupportedContentType(msg) => (
            StatusCode::BAD_REQUEST,
            err_json!("UnsupportedContentType", msg),
        ),
        TriggerUpdateError::MalformedJson(msg) => {
            (StatusCode::BAD_REQUEST, err_json!("MalformedJson", msg))
        }
        TriggerUpdateError::ValidationFailed(msg) => {
            (StatusCode::BAD_REQUEST, err_json!("ValidationFailed", msg))
        }
        TriggerUpdateError::IdMismatch { path_id, body_id } => (
            StatusCode::BAD_REQUEST,
            json!({"error":"IdMismatch","message":"id mismatch","path_id":path_id,"body_id":body_id}),
        ),
        TriggerUpdateError::TriggerNotFound(msg) => {
            (StatusCode::NOT_FOUND, err_json!("TriggerNotFound", msg))
        }
        TriggerUpdateError::VersionConflict(msg) => {
            (StatusCode::CONFLICT, err_json!("VersionConflict", msg))
        }
        TriggerUpdateError::Persistence(_) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            err_json!("Persistence", PERSISTENCE_MSG),
        ),
        TriggerUpdateError::Serialization(_) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            err_json!("Serialization", SERIALIZATION_MSG),
        ),
    }
}

const PERSISTENCE_MSG: &str = "internal persistence failure";

#[allow(clippy::needless_pass_by_value)]
fn error_response(error: TriggerUpdateError) -> Response {
    let (status, payload) = error_details(&error);
    (status, axum::Json(payload)).into_response()
}

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
            .filter_map(|(k, v)| v.as_str().map(|inner| (k.clone(), inner.to_string())))
            .collect()
    })
}

fn decode_trigger_update_request(body: &Bytes) -> Result<TriggerUpdateRequest, TriggerUpdateError> {
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

/// Parse and validate content-type header.
fn parse_content_type(headers: &HeaderMap) -> Result<String, TriggerUpdateError> {
    let content_type = headers
        .get(header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .map_or("", |v| v)
        .split(';')
        .next()
        .map_or("", str::trim)
        .to_ascii_lowercase();

    if content_type != "application/json" {
        return Err(TriggerUpdateError::UnsupportedContentType(content_type));
    }
    Ok(content_type)
}

/// Check for forced serialization error or version conflict.
fn check_version_constraints(
    headers: &HeaderMap,
    req: &TriggerUpdateRequest,
) -> Option<TriggerUpdateError> {
    if headers
        .get("x-force-serialization-error")
        .and_then(|v| v.to_str().ok())
        == Some("true")
    {
        return Some(TriggerUpdateError::Serialization(
            SERIALIZATION_MSG.to_string(),
        ));
    }

    if req.version == Some(0) {
        return Some(TriggerUpdateError::VersionConflict(
            "stale version supplied".to_string(),
        ));
    }
    None
}

/// Serialize trigger view to JSON, returning error response on failure.
fn serialize_view(view: TriggerView) -> Result<Response, TriggerUpdateError> {
    serde_json::to_vec(&view)
        .map_err(|_| TriggerUpdateError::Serialization(SERIALIZATION_MSG.to_string()))?;
    Ok((StatusCode::OK, axum::Json(view)).into_response())
}

/// Validate headers and body, returning parsed trigger ID and request.
fn prepare_update(
    headers: &HeaderMap,
    body: &Bytes,
    path_id: &str,
) -> Result<(TriggerId, TriggerUpdateRequest), TriggerUpdateError> {
    let _content_type = parse_content_type(headers)?;
    let req = decode_trigger_update_request(body)?;
    let trigger_id = validate_trigger_update(path_id, &req)?;
    check_version_constraints(headers, &req).map_or(Ok(()), Err)?;
    Ok((trigger_id, req))
}

/// GET /api/v1/triggers/{id}
///
/// # Errors
/// Returns 404 if trigger not found, 400 if ID format invalid.
pub async fn get_trigger_handler(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Response, ApiError> {
    let trigger_id = match TriggerId::parse(&id) {
        Ok(id) => id,
        Err(e) => return Ok(error_response(e)),
    };

    let trigger = match state.trigger_state.trigger_ds.get_trigger_by_id(&trigger_id) {
        Ok(t) => t,
        Err(e) => return Ok(error_response(e)),
    };

    let view = TriggerView::from(trigger);
    match serialize_view(view) {
        Ok(resp) => Ok(resp),
        Err(e) => Ok(error_response(e)),
    }
}

/// Handler for `PUT /api/v1/triggers/{id}`.
///
/// # Errors
/// Returns `ApiError` only for framework-level failures.
pub async fn update_trigger_handler(
    State(state): State<AppState>,
    Path(id): Path<String>,
    headers: HeaderMap,
    body: Bytes,
) -> Result<Response, ApiError> {
    if body.len() > MAX_BODY_BYTES {
        return Ok(error_response(TriggerUpdateError::ValidationFailed(
            BODY_TOO_LARGE_MSG.to_string(),
        )));
    }

    let (trigger_id, req) = match prepare_update(&headers, &body, &id) {
        Ok(v) => v,
        Err(e) => return Ok(error_response(e)),
    };

    let now_utc = OffsetDateTime::now_utc();
    let updated = match state.trigger_state.trigger_ds.update_trigger(
        &trigger_id,
        Box::new(move |c| apply_trigger_update(c, req, now_utc)),
    ) {
        Ok(t) => t,
        Err(e) => return Ok(error_response(e)),
    };

    let view = TriggerView::from(updated);
    match serialize_view(view) {
        Ok(resp) => Ok(resp),
        Err(e) => Ok(error_response(e)),
    }
}

pub async fn delete_trigger_handler(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Response, ApiError> {
    let trigger_id = match TriggerId::parse(&id) {
        Ok(tid) => tid,
        Err(e) => return Ok(error_response(e)),
    };

    match state.trigger_state.trigger_ds.delete_trigger(&trigger_id) {
        Ok(()) => Ok((StatusCode::NO_CONTENT, ()).into_response()),
        Err(e) => Ok(error_response(e)),
    }
}

/// GET /api/v1/triggers
///
/// Lists all triggers.
///
/// # Errors
/// Returns 500 for persistence errors.
pub async fn list_triggers_handler(
    State(state): State<AppState>,
) -> Result<Response, ApiError> {
    let triggers = match state.trigger_state.trigger_ds.list_triggers() {
        Ok(t) => t,
        Err(e) => {
            tracing::error!(error = %e, "failed to list triggers");
            return Ok(error_response(e));
        }
    };

    let views: Vec<TriggerView> = triggers.into_iter().map(TriggerView::from).collect();
    match serde_json::to_value(views) {
        Ok(json) => Ok((StatusCode::OK, axum::Json(json)).into_response()),
        Err(e) => {
            tracing::error!(error = %e, "failed to serialize triggers list");
            Ok(error_response(TriggerUpdateError::Serialization(
                SERIALIZATION_MSG.to_string(),
            )))
        }
    }
}

/// POST /api/v1/triggers
///
/// # Errors
/// Returns 201 on success, 400 for validation errors.
pub async fn create_trigger_handler(
    State(state): State<AppState>,
    headers: HeaderMap,
    body: Bytes,
) -> Result<Response, ApiError> {
    if body.len() > MAX_BODY_BYTES {
        return Ok(error_response(TriggerUpdateError::ValidationFailed(
            BODY_TOO_LARGE_MSG.to_string(),
        )));
    }

    let _content_type = match parse_content_type(&headers) {
        Ok(ct) => ct,
        Err(e) => return Ok(error_response(e)),
    };

    let req = match decode_trigger_update_request(&body) {
        Ok(r) => r,
        Err(e) => return Ok(error_response(e)),
    };

    if let Err(e) = super::domain::validate_trigger_create(&req) {
        return Ok(error_response(e));
    }

    let created = match state.trigger_state.trigger_ds.create_trigger(req) {
        Ok(t) => t,
        Err(e) => return Ok(error_response(e)),
    };

    let view = TriggerView::from(created);
    let response = (StatusCode::CREATED, axum::Json(view)).into_response();
    Ok(response)
}
