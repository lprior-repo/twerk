//! Twerk Engine - Type definitions for handlers, errors, and middleware

use std::collections::HashMap;
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;
use twerk_core::job::Job;
use twerk_core::task::Task;
use twerk_core::task::TaskLogPart;

// Re-export from state module
pub use crate::engine::state::{Mode, State};

// ── Handler function types ────────────────────────────────────

/// Typed task middleware function.
///
/// Follows the same pattern as `middleware::task::MiddlewareFunc`:
/// wraps a [`TaskHandlerFunc`] and returns a wrapped handler.
pub type TaskMiddlewareFunc = Arc<dyn Fn(TaskHandlerFunc) -> TaskHandlerFunc + Send + Sync>;

/// Typed job middleware function.
pub type JobMiddlewareFunc = Arc<dyn Fn(JobHandlerFunc) -> JobHandlerFunc + Send + Sync>;

/// Typed log middleware function.
pub type LogMiddlewareFunc = Arc<dyn Fn(LogHandlerFunc) -> LogHandlerFunc + Send + Sync>;

/// Typed node middleware function.
pub type NodeMiddlewareFunc = Arc<dyn Fn(NodeHandlerFunc) -> NodeHandlerFunc + Send + Sync>;

/// Typed web (axum) middleware function.
///
/// Wraps an axum `Next` and returns a pinned future yielding an HTTP
/// response, matching the axum middleware signature.
pub type WebMiddlewareFunc = Arc<
    dyn Fn(
            axum::http::Request<axum::body::Body>,
            axum::middleware::Next,
        ) -> Pin<Box<dyn Future<Output = axum::response::Response> + Send>>
        + Send
        + Sync,
>;

/// Typed API endpoint handler.
///
/// An `Arc`-wrapped async function that receives an axum request parts
/// reference and the request body bytes, returning a response.
pub type EndpointHandler = Arc<
    dyn Fn(
            axum::http::request::Parts,
            bytes::Bytes,
        ) -> Pin<Box<dyn Future<Output = axum::response::Response> + Send>>
        + Send
        + Sync,
>;

/// Job listener callback type
pub type JobListener = Arc<dyn Fn(Job) + Send + Sync>;

/// Task event type for middleware handlers.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TaskEventType {
    Started,
    StateChange,
    Redelivered,
    Progress,
}

/// Task handler function (mirrors `coordinator::handlers::TaskHandlerFunc`).
pub type TaskHandlerFunc =
    Arc<dyn Fn(Arc<()>, TaskEventType, &mut Task) -> Result<(), TaskHandlerError> + Send + Sync>;

/// Job handler function.
pub type JobHandlerFunc =
    Arc<dyn Fn(Arc<()>, JobEventType, &mut Job) -> Result<(), JobHandlerError> + Send + Sync>;

/// Log handler function.
pub type LogHandlerFunc =
    Arc<dyn Fn(Arc<()>, &[TaskLogPart]) -> Result<(), LogHandlerError> + Send + Sync>;

/// Node handler function.
pub type NodeHandlerFunc =
    Arc<dyn Fn(Arc<()>, &mut twerk_core::node::Node) -> Result<(), NodeHandlerError> + Send + Sync>;

/// Job event type for middleware handlers.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum JobEventType {
    StateChange,
    Progress,
    Read,
}

// ── Handler errors (per-category, thiserror) ──────────────────

/// Error returned by task middleware/handlers.
#[derive(Debug, Clone, PartialEq, thiserror::Error)]
pub enum TaskHandlerError {
    #[error("task handler error: {0}")]
    Handler(String),
    #[error("task datastore error: {0}")]
    Datastore(String),
}

/// Error returned by job middleware/handlers.
#[derive(Debug, Clone, PartialEq, thiserror::Error)]
pub enum JobHandlerError {
    #[error("job handler error: {0}")]
    Handler(String),
    #[error("job datastore error: {0}")]
    Datastore(String),
}

/// Error returned by log middleware/handlers.
#[derive(Debug, Clone, PartialEq, thiserror::Error)]
pub enum LogHandlerError {
    #[error("log handler error: {0}")]
    Handler(String),
    #[error("log middleware error: {0}")]
    Middleware(String),
}

/// Error returned by node middleware/handlers.
#[derive(Debug, Clone, PartialEq, thiserror::Error)]
pub enum NodeHandlerError {
    #[error("node handler error: {0}")]
    Handler(String),
    #[error("node datastore error: {0}")]
    Datastore(String),
}

/// Handle returned by `Engine::submit_task` containing the task metadata.
#[derive(Debug, Clone)]
pub struct TaskHandle {
    pub task_id: twerk_core::id::TaskId,
}

/// Error returned by `Engine::submit_task`.
#[derive(Debug, Clone, PartialEq, thiserror::Error)]
pub enum SubmitTaskError {
    #[error("engine is not running")]
    NotRunning,
    #[error("task with id {0} already exists")]
    DuplicateTaskId(twerk_core::id::TaskId),
}

// ── Configuration ──────────────────────────────────────────────

/// Middleware configuration — fully typed, zero `dyn Any`.
///
/// Note: does not derive `Debug` because the inner `dyn Fn` trait
/// objects do not implement `Debug`.
#[derive(Default, Clone)]
pub struct Middleware {
    pub web: Vec<WebMiddlewareFunc>,
    pub task: Vec<TaskMiddlewareFunc>,
    pub job: Vec<JobMiddlewareFunc>,
    pub node: Vec<NodeMiddlewareFunc>,
    pub log: Vec<LogMiddlewareFunc>,
}

/// Engine configuration.
///
/// Note: does not derive `Debug` because `Middleware` and
/// `EndpointHandler` (`dyn Fn`) do not implement `Debug`.
#[derive(Default)]
pub struct Config {
    pub mode: Mode,
    pub engine_id: Option<String>,
    pub hostname: Option<String>,
    pub middleware: Middleware,
    pub endpoints: HashMap<String, EndpointHandler>,
}
