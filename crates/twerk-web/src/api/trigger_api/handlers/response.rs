use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use serde_json::{json, Value};

use crate::api::trigger_api::domain::{TriggerUpdateError, SERIALIZATION_MSG};

const PERSISTENCE_MSG: &str = "internal persistence failure";

macro_rules! err_json {
    ($name:expr, $message:expr) => {
        json!({"error": $name, "message": $message})
    };
    ($name:expr, $message:expr, $($key:ident: $value:expr),*) => {
        json!({"error": $name, "message": $message, $(stringify!($key): $value),*})
    };
}

fn error_details(error: &TriggerUpdateError) -> (StatusCode, Value) {
    match error {
        TriggerUpdateError::InvalidIdFormat(message) => (
            StatusCode::BAD_REQUEST,
            err_json!("InvalidIdFormat", message),
        ),
        TriggerUpdateError::UnsupportedContentType(message) => (
            StatusCode::BAD_REQUEST,
            err_json!("UnsupportedContentType", message),
        ),
        TriggerUpdateError::MalformedJson(message) => {
            (StatusCode::BAD_REQUEST, err_json!("MalformedJson", message))
        }
        TriggerUpdateError::ValidationFailed(message) => (
            StatusCode::BAD_REQUEST,
            err_json!("ValidationFailed", message),
        ),
        TriggerUpdateError::IdMismatch { path_id, body_id } => (
            StatusCode::BAD_REQUEST,
            err_json!("IdMismatch", "id mismatch", path_id: path_id, body_id: body_id),
        ),
        TriggerUpdateError::TriggerNotFound(message) => {
            (StatusCode::NOT_FOUND, err_json!("TriggerNotFound", message))
        }
        TriggerUpdateError::VersionConflict(message) => {
            (StatusCode::CONFLICT, err_json!("VersionConflict", message))
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

#[must_use]
pub fn error_response(error: &TriggerUpdateError) -> Response {
    let (status, payload) = error_details(error);
    (status, axum::Json(payload)).into_response()
}

/// Serialize a trigger view into a JSON HTTP response.
///
/// # Errors
/// Returns an error when the trigger view cannot be serialized for response generation.
pub fn serialize_view(
    view: super::super::domain::TriggerView,
) -> Result<Response, TriggerUpdateError> {
    serde_json::to_vec(&view)
        .map_err(|_| TriggerUpdateError::Serialization(SERIALIZATION_MSG.to_string()))?;
    Ok((StatusCode::OK, axum::Json(view)).into_response())
}
