use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use serde_json::json;
use thiserror::Error;

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
            Self::Internal(msg) => (StatusCode::INTERNAL_SERVER_ERROR, msg),
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
