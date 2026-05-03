use axum::body::Bytes;
use axum::extract::{Path, State};
use axum::http::{HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use time::OffsetDateTime;

use crate::api::error::ApiError;
use crate::api::openapi_types::TriggerErrorResponse;
use crate::AppState;

use super::super::domain::{
    apply_trigger_update, validate_trigger_create, TriggerUpdateError, TriggerUpdateRequest,
    TriggerView,
};
use super::parsing::{decode_trigger_update_request, parse_content_type, prepare_update};
use super::response::{error_response, serialize_view};
use super::{BODY_TOO_LARGE_MSG, MAX_BODY_BYTES};

/// Handler for `PUT /triggers/{id}`.
#[utoipa::path(
    put,
    path = "/triggers/{id}",
    request_body = TriggerUpdateRequest,
    params(("id" = String, Path, description = "Trigger ID")),
    responses(
        (status = 200, description = "Trigger updated", body = TriggerView, content_type = "application/json"),
        (status = 400, description = "Validation error or invalid request", body = TriggerErrorResponse, content_type = "application/json"),
        (status = 404, description = "Trigger not found", body = TriggerErrorResponse, content_type = "application/json"),
        (status = 409, description = "Version conflict", body = TriggerErrorResponse, content_type = "application/json")
    )
)]
/// # Errors
/// This handler does not currently return `Err(ApiError)` directly; request validation, trigger
/// update, and serialization failures are converted into HTTP responses.
pub async fn update_trigger_handler(
    State(state): State<AppState>,
    Path(id): Path<String>,
    headers: HeaderMap,
    body: Bytes,
) -> Result<Response, ApiError> {
    if body.len() > MAX_BODY_BYTES {
        return Ok(error_response(&TriggerUpdateError::ValidationFailed(
            BODY_TOO_LARGE_MSG.to_string(),
        )));
    }

    let (trigger_id, request) = match prepare_update(&headers, &body, &id) {
        Ok(value) => value,
        Err(error) => return Ok(error_response(&error)),
    };

    let updated = match state.trigger_state.trigger_ds.update_trigger(
        &trigger_id,
        Box::new(move |current| apply_trigger_update(current, request, OffsetDateTime::now_utc())),
    ) {
        Ok(trigger) => trigger,
        Err(error) => return Ok(error_response(&error)),
    };

    match serialize_view(TriggerView::from(updated)) {
        Ok(response) => Ok(response),
        Err(error) => Ok(error_response(&error)),
    }
}

/// POST /triggers
#[utoipa::path(
    post,
    path = "/triggers",
    request_body = TriggerUpdateRequest,
    responses(
        (status = 201, description = "Trigger created", body = TriggerView, content_type = "application/json"),
        (status = 400, description = "Validation error", body = TriggerErrorResponse, content_type = "application/json")
    )
)]
/// # Errors
/// This handler does not currently return `Err(ApiError)` directly; request validation, trigger
/// creation, and serialization failures are converted into HTTP responses.
pub async fn create_trigger_handler(
    State(state): State<AppState>,
    headers: HeaderMap,
    body: Bytes,
) -> Result<Response, ApiError> {
    if body.len() > MAX_BODY_BYTES {
        return Ok(error_response(&TriggerUpdateError::ValidationFailed(
            BODY_TOO_LARGE_MSG.to_string(),
        )));
    }

    if let Err(error) = parse_content_type(&headers) {
        return Ok(error_response(&error));
    }

    let request = match decode_trigger_update_request(&body) {
        Ok(request) => request,
        Err(error) => return Ok(error_response(&error)),
    };
    if let Err(error) = validate_trigger_create(&request) {
        return Ok(error_response(&error));
    }

    let created = match state.trigger_state.trigger_ds.create_trigger(request) {
        Ok(trigger) => trigger,
        Err(error) => return Ok(error_response(&error)),
    };

    Ok((StatusCode::CREATED, axum::Json(TriggerView::from(created))).into_response())
}
