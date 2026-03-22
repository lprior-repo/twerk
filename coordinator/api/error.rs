//! API error types and response helpers.
//!
//! Provides a unified error type that maps domain errors to HTTP responses.

use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use serde_json::json;

/// API error with HTTP status code and message.
#[derive(Debug, Clone)]
pub struct ApiError {
    pub status: StatusCode,
    pub message: String,
}

impl ApiError {
    /// Create a 400 Bad Request error.
    #[must_use]
    pub fn bad_request(message: impl std::fmt::Display) -> Self {
        Self {
            status: StatusCode::BAD_REQUEST,
            message: message.to_string(),
        }
    }

    /// Create a 404 Not Found error.
    #[must_use]
    pub fn not_found(message: impl std::fmt::Display) -> Self {
        Self {
            status: StatusCode::NOT_FOUND,
            message: message.to_string(),
        }
    }

    /// Create a 500 Internal Server Error.
    #[must_use]
    pub fn internal(message: impl std::fmt::Display) -> Self {
        Self {
            status: StatusCode::INTERNAL_SERVER_ERROR,
            message: message.to_string(),
        }
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let body = json!({ "error": self.message });
        (self.status, axum::Json(body)).into_response()
    }
}

impl From<anyhow::Error> for ApiError {
    fn from(err: anyhow::Error) -> Self {
        Self::internal(err.to_string())
    }
}
