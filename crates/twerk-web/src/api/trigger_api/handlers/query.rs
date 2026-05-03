use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};

use crate::api::error::ApiError;
use crate::api::openapi_types::TriggerErrorResponse;
use crate::AppState;

use super::super::domain::{TriggerId, TriggerUpdateError, TriggerView, SERIALIZATION_MSG};
use super::response::{error_response, serialize_view};

/// GET /triggers/{id}
#[utoipa::path(
    get,
    path = "/triggers/{id}",
    params(("id" = String, Path, description = "Trigger ID")),
    responses(
        (status = 200, description = "Trigger found", body = TriggerView, content_type = "application/json"),
        (status = 404, description = "Trigger not found", body = TriggerErrorResponse, content_type = "application/json"),
        (status = 400, description = "Invalid ID format", body = TriggerErrorResponse, content_type = "application/json")
    )
)]
/// # Errors
/// This handler does not currently return `Err(ApiError)` directly; invalid identifiers, missing
/// triggers, and serialization failures are converted into HTTP responses.
pub async fn get_trigger_handler(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Response, ApiError> {
    let trigger_id = match TriggerId::parse(&id) {
        Ok(trigger_id) => trigger_id,
        Err(error) => return Ok(error_response(&error)),
    };
    let trigger = match state
        .trigger_state
        .trigger_ds
        .get_trigger_by_id(&trigger_id)
    {
        Ok(trigger) => trigger,
        Err(error) => return Ok(error_response(&error)),
    };

    match serialize_view(TriggerView::from(trigger)) {
        Ok(response) => Ok(response),
        Err(error) => Ok(error_response(&error)),
    }
}

#[utoipa::path(
    delete,
    path = "/triggers/{id}",
    params(("id" = String, Path, description = "Trigger ID")),
    responses(
        (status = 204, description = "Trigger deleted"),
        (status = 404, description = "Trigger not found", body = TriggerErrorResponse, content_type = "application/json"),
        (status = 400, description = "Invalid ID format", body = TriggerErrorResponse, content_type = "application/json")
    )
)]
#[allow(clippy::missing_errors_doc)]
pub async fn delete_trigger_handler(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Response, ApiError> {
    let trigger_id = match TriggerId::parse(&id) {
        Ok(trigger_id) => trigger_id,
        Err(error) => return Ok(error_response(&error)),
    };

    match state.trigger_state.trigger_ds.delete_trigger(&trigger_id) {
        Ok(()) => Ok((StatusCode::NO_CONTENT, ()).into_response()),
        Err(error) => Ok(error_response(&error)),
    }
}

/// GET /triggers
#[utoipa::path(
    get,
    path = "/triggers",
    responses(
        (status = 200, description = "List of triggers", body = Vec<TriggerView>, content_type = "application/json"),
        (status = 500, description = "Persistence error", body = TriggerErrorResponse, content_type = "application/json")
    )
)]
/// # Errors
/// This handler does not currently return `Err(ApiError)` directly; persistence and serialization
/// failures are converted into HTTP responses.
pub async fn list_triggers_handler(State(state): State<AppState>) -> Result<Response, ApiError> {
    let triggers = match state.trigger_state.trigger_ds.list_triggers() {
        Ok(triggers) => triggers,
        Err(error) => {
            tracing::error!(error = %error, "failed to list triggers");
            return Ok(error_response(&error));
        }
    };

    match serde_json::to_value(
        triggers
            .into_iter()
            .map(TriggerView::from)
            .collect::<Vec<_>>(),
    ) {
        Ok(json) => Ok((StatusCode::OK, axum::Json(json)).into_response()),
        Err(error) => {
            tracing::error!(error = %error, "failed to serialize triggers list");
            Ok(error_response(&TriggerUpdateError::Serialization(
                SERIALIZATION_MSG.to_string(),
            )))
        }
    }
}
