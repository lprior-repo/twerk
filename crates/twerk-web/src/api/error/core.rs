use axum::http::{HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use serde_json::json;
use thiserror::Error;
use tracing::error;
use utoipa::ToSchema;

const PROBLEM_CONTENT_TYPE: &str = "application/problem+json";

#[derive(Debug, Error, PartialEq, Eq, ToSchema)]
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

    fn problem_type_uri(&self) -> &'static str {
        match self {
            Self::BadRequest(_) => "about:blank",
            Self::NotFound(_) => "https://httpstatus.es/404",
            Self::Internal(_) => "https://httpstatus.es/500",
        }
    }

    fn problem_title(&self) -> &'static str {
        match self {
            Self::BadRequest(_) => "Bad Request",
            Self::NotFound(_) => "Not Found",
            Self::Internal(_) => "Internal Server Error",
        }
    }

    fn to_problem_detail(&self) -> String {
        match self {
            Self::BadRequest(msg) | Self::NotFound(msg) | Self::Internal(msg) => msg.to_string(),
        }
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let status = match self {
            Self::BadRequest(_) => StatusCode::BAD_REQUEST,
            Self::NotFound(_) => StatusCode::NOT_FOUND,
            Self::Internal(ref msg) => {
                error!(error = %msg, "internal server error");
                StatusCode::INTERNAL_SERVER_ERROR
            }
        };

        let detail = self.to_problem_detail();
        let problem = json!({
            "type": self.problem_type_uri(),
            "title": self.problem_title(),
            "status": status.as_u16(),
            "detail": detail
        });

        let mut headers = HeaderMap::new();
        headers.insert(
            axum::http::header::CONTENT_TYPE,
            PROBLEM_CONTENT_TYPE.parse().unwrap(),
        );

        (status, headers, axum::Json(problem)).into_response()
    }
}
