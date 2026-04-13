use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use serde_json::json;
use thiserror::Error;
use tracing::error;

const INTERNAL_ERROR_MSG: &str = "Internal Server Error";

#[derive(Debug, Error, PartialEq, Eq)]
pub enum ApiError {
    #[error("{0}")]
    BadRequest(String),
    #[error("{0}")]
    NotFound(String),
    #[error("{0}")]
    Internal(String),
}

impl ApiError {
    pub fn bad_request(msg: impl Into<String>) -> Self {
        Self::BadRequest(msg.into())
    }
    pub fn not_found(msg: impl Into<String>) -> Self {
        Self::NotFound(msg.into())
    }
    pub fn internal(msg: impl Into<String>) -> Self {
        Self::Internal(msg.into())
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let (status, msg) = match self {
            Self::BadRequest(msg) => (StatusCode::BAD_REQUEST, msg),
            Self::NotFound(msg) => (StatusCode::NOT_FOUND, msg),
            Self::Internal(msg) => {
                error!(error = %msg, "internal server error");
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    INTERNAL_ERROR_MSG.to_string(),
                )
            }
        };
        let body = axum::Json(json!({
            "message": msg
        }));
        (status, body).into_response()
    }
}

impl From<twerk_infrastructure::datastore::Error> for ApiError {
    fn from(err: twerk_infrastructure::datastore::Error) -> Self {
        match err {
            twerk_infrastructure::datastore::Error::UserNotFound => {
                Self::NotFound("user not found".to_string())
            }
            twerk_infrastructure::datastore::Error::JobNotFound => {
                Self::NotFound("job not found".to_string())
            }
            twerk_infrastructure::datastore::Error::TaskNotFound => {
                Self::NotFound("task not found".to_string())
            }
            twerk_infrastructure::datastore::Error::ScheduledJobNotFound => {
                Self::NotFound("scheduled job not found".to_string())
            }
            twerk_infrastructure::datastore::Error::NodeNotFound => {
                Self::NotFound("node not found".to_string())
            }
            _ => Self::Internal(err.to_string()),
        }
    }
}

impl From<anyhow::Error> for ApiError {
    fn from(err: anyhow::Error) -> Self {
        Self::Internal(err.to_string())
    }
}

impl From<twerk_core::id::IdError> for ApiError {
    fn from(err: twerk_core::id::IdError) -> Self {
        Self::Internal(err.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::to_bytes;
    use axum::response::IntoResponse;

    async fn extract_response_body(response: Response) -> String {
        let body = response.into_body();
        let bytes = to_bytes(body, usize::MAX).await.unwrap_or_default();
        String::from_utf8(bytes.to_vec()).unwrap_or_default()
    }

    #[tokio::test]
    async fn into_response_internal_sanitizes_message() {
        let error = ApiError::Internal(
            "secret stack trace: connection refused db://admin:pass@host".to_string(),
        );
        let response = error.into_response();
        let body = extract_response_body(response).await;
        assert!(
            !body.contains("secret"),
            "body should not contain secret: {body}"
        );
        assert!(
            !body.contains("db://"),
            "body should not contain connection string: {body}"
        );
        assert!(
            body.contains("Internal Server Error"),
            "body should contain generic message: {body}"
        );
    }

    #[tokio::test]
    async fn into_response_bad_request_preserves_message() {
        let error = ApiError::bad_request("invalid input");
        let response = error.into_response();
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
        let body = extract_response_body(response).await;
        assert!(
            body.contains("invalid input"),
            "body should preserve message: {body}"
        );
    }

    #[tokio::test]
    async fn into_response_not_found_returns_404() {
        let error = ApiError::not_found("resource gone");
        let response = error.into_response();
        assert_eq!(response.status(), StatusCode::NOT_FOUND);
        let body = extract_response_body(response).await;
        assert!(
            body.contains("resource gone"),
            "body should preserve message: {body}"
        );
    }

    #[test]
    fn from_datastore_error_maps_user_not_found() {
        let err = twerk_infrastructure::datastore::Error::UserNotFound;
        let api_err: ApiError = err.into();
        assert_eq!(api_err, ApiError::NotFound("user not found".to_string()));
    }

    #[test]
    fn from_datastore_error_maps_job_not_found() {
        let err = twerk_infrastructure::datastore::Error::JobNotFound;
        let api_err: ApiError = err.into();
        assert_eq!(api_err, ApiError::NotFound("job not found".to_string()));
    }

    #[test]
    fn from_datastore_error_maps_task_not_found() {
        let err = twerk_infrastructure::datastore::Error::TaskNotFound;
        let api_err: ApiError = err.into();
        assert_eq!(api_err, ApiError::NotFound("task not found".to_string()));
    }

    #[test]
    fn from_datastore_error_maps_scheduled_job_not_found() {
        let err = twerk_infrastructure::datastore::Error::ScheduledJobNotFound;
        let api_err: ApiError = err.into();
        assert_eq!(
            api_err,
            ApiError::NotFound("scheduled job not found".to_string())
        );
    }

    #[test]
    fn from_datastore_error_maps_node_not_found() {
        let err = twerk_infrastructure::datastore::Error::NodeNotFound;
        let api_err: ApiError = err.into();
        assert_eq!(api_err, ApiError::NotFound("node not found".to_string()));
    }

    #[test]
    fn from_datastore_error_maps_unknown_to_internal() {
        let err = twerk_infrastructure::datastore::Error::Database("table not found".to_string());
        let api_err: ApiError = err.into();
        assert_eq!(
            api_err,
            ApiError::Internal("database error: table not found".to_string())
        );
    }

    #[test]
    fn from_anyhow_error_maps_to_internal() {
        let err = anyhow::anyhow!("something broke");
        let api_err: ApiError = err.into();
        assert_eq!(api_err, ApiError::Internal("something broke".to_string()));
    }

    #[tokio::test]
    async fn into_response_not_found_preserves_exact_message_payload() {
        let response = ApiError::NotFound("missing trigger".to_string()).into_response();
        assert_eq!(response.status(), StatusCode::NOT_FOUND);
        let body = extract_response_body(response).await;
        assert_eq!(body, r#"{"message":"missing trigger"}"#);
    }

    #[tokio::test]
    async fn into_response_internal_returns_sanitized_exact_payload() {
        let response = ApiError::Internal("leaky detail".to_string()).into_response();
        assert_eq!(response.status(), StatusCode::INTERNAL_SERVER_ERROR);
        let body = extract_response_body(response).await;
        assert_eq!(body, r#"{"message":"Internal Server Error"}"#);
    }
}
