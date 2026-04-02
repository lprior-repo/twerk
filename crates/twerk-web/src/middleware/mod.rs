//! Web middleware types for HTTP request/response handling.
//!
//! This module provides middleware hooks including `onReadJob` and `onReadTask`
//! which are called when reading job/task data from the API to mask sensitive fields.

use axum::http::{Request, StatusCode};
use axum::middleware::Next;

pub mod hooks;

pub use hooks::{
    apply_job_middleware, apply_task_middleware, create_read_job_middleware,
    create_read_task_middleware, on_read_job, on_read_job_summary, on_read_task, JobContext,
    JobEventType, JobHandlerFunc, JobMiddlewareError, JobMiddlewareFunc, TaskContext,
    TaskEventType, TaskHandlerFunc, TaskMiddlewareError, TaskMiddlewareFunc,
};

// Re-export UsernameValue from twerk_core for use in request extensions
pub use twerk_core::user::UsernameValue;

pub async fn logging_middleware(
    request: Request<axum::body::Body>,
    next: Next,
) -> axum::response::Response {
    let method = request.method().clone();
    let uri = request.uri().clone();
    tracing::info!("HTTP request: {} {}", method, uri);
    let response = next.run(request).await;
    tracing::info!("HTTP response: {} {} -> {}", method, uri, response.status());
    response
}

pub async fn auth_middleware(
    request: Request<axum::body::Body>,
    next: Next,
) -> Result<axum::response::Response, StatusCode> {
    Ok(next.run(request).await)
}
