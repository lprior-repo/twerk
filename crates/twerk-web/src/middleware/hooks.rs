//! Middleware hooks for processing API responses.
//!
//! This module provides specialized middleware hooks following the Go Tork pattern:
//! - `on_read_job`: Masks sensitive fields when reading a job from the API
//! - `on_read_task`: Masks sensitive fields when reading a task from the API
//!
//! The middleware pattern:
//! - `HandlerFunc` takes context, event type, and mutable reference to Job/Task
//! - `MiddlewareFunc` wraps a `HandlerFunc` and can modify behavior
//! - When event type is `Read`, the middleware applies redaction
//! - `apply_middleware` composes multiple middleware functions

use std::collections::HashMap;
use std::sync::Arc;
use twerk_core::job::{Job, JobSummary};
use twerk_core::task::Task;

use crate::api::redact;

pub const REDACTED_STR: &str = "[REDACTED]";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum JobEventType {
    StateChange,
    Progress,
    Read,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TaskEventType {
    Started,
    StateChange,
    Redelivered,
    Progress,
    Read,
}

pub type JobHandlerFunc = Arc<
    dyn Fn(Arc<JobContext>, JobEventType, &mut Job) -> Result<(), JobMiddlewareError> + Send + Sync,
>;
pub type TaskHandlerFunc = Arc<
    dyn Fn(Arc<TaskContext>, TaskEventType, &mut Task) -> Result<(), TaskMiddlewareError>
        + Send
        + Sync,
>;

pub type JobMiddlewareFunc = Arc<dyn Fn(JobHandlerFunc) -> JobHandlerFunc + Send + Sync>;
pub type TaskMiddlewareFunc = Arc<dyn Fn(TaskHandlerFunc) -> TaskHandlerFunc + Send + Sync>;

#[derive(Debug, thiserror::Error)]
pub enum JobMiddlewareError {
    #[error("job not found: {0}")]
    JobNotFound(String),
    #[error("context cancelled")]
    ContextCancelled,
    #[error("context deadline exceeded")]
    ContextDeadlineExceeded,
    #[error("middleware error: {0}")]
    Middleware(String),
    #[error("datastore error: {0}")]
    Datastore(String),
}

#[derive(Debug, thiserror::Error)]
pub enum TaskMiddlewareError {
    #[error("task not found: {0}")]
    TaskNotFound(String),
    #[error("context cancelled")]
    ContextCancelled,
    #[error("context deadline exceeded")]
    ContextDeadlineExceeded,
    #[error("middleware error: {0}")]
    Middleware(String),
    #[error("datastore error: {0}")]
    Datastore(String),
}

#[derive(Debug, Clone)]
pub enum JobContext {
    Cancelled,
    DeadlineExceeded,
    Values(HashMap<String, String>),
}

impl JobContext {
    #[must_use]
    pub const fn is_cancelled(&self) -> bool {
        matches!(self, Self::Cancelled)
    }
    #[must_use]
    pub const fn is_deadline_exceeded(&self) -> bool {
        matches!(self, Self::DeadlineExceeded)
    }
    #[must_use]
    pub fn get(&self, key: &str) -> Option<&str> {
        match self {
            Self::Values(vals) => vals.get(key).map(String::as_str),
            Self::Cancelled | Self::DeadlineExceeded => None,
        }
    }
}

#[derive(Debug, Clone)]
pub enum TaskContext {
    Cancelled,
    DeadlineExceeded,
    Values(HashMap<String, String>),
}

impl TaskContext {
    #[must_use]
    pub const fn is_cancelled(&self) -> bool {
        matches!(self, Self::Cancelled)
    }
    #[must_use]
    pub const fn is_deadline_exceeded(&self) -> bool {
        matches!(self, Self::DeadlineExceeded)
    }
    #[must_use]
    pub fn get(&self, key: &str) -> Option<&str> {
        match self {
            Self::Values(vals) => vals.get(key).map(String::as_str),
            Self::Cancelled | Self::DeadlineExceeded => None,
        }
    }
}

pub fn apply_job_middleware(h: JobHandlerFunc, mws: &[JobMiddlewareFunc]) -> JobHandlerFunc {
    let mut handler = h;
    for mw in mws.iter().rev() {
        handler = mw(handler);
    }
    handler
}

pub fn apply_task_middleware(h: TaskHandlerFunc, mws: &[TaskMiddlewareFunc]) -> TaskHandlerFunc {
    let mut handler = h;
    for mw in mws.iter().rev() {
        handler = mw(handler);
    }
    handler
}

pub fn on_read_job<S: std::hash::BuildHasher>(
    job: &mut Job,
    _secrets: &HashMap<String, String, S>,
) {
    redact::redact_job(job);
}

pub fn on_read_job_summary(summary: &mut JobSummary) {
    redact::redact_job_summary(summary);
}

pub fn on_read_task<S: std::hash::BuildHasher>(
    task: &mut Task,
    secrets: &HashMap<String, String, S>,
) {
    redact::redact_task(task, secrets);
}

#[must_use]
pub fn create_read_job_middleware() -> JobMiddlewareFunc {
    Arc::new(move |next: JobHandlerFunc| {
        let next = next.clone();
        Arc::new(
            move |ctx: Arc<JobContext>, et: JobEventType, job: &mut Job| {
                if et == JobEventType::Read {
                    let secrets = job.secrets.clone().unwrap_or_default();
                    on_read_job(job, &secrets);
                }
                next(ctx, et, job)
            },
        )
    })
}

#[must_use]
pub fn create_read_task_middleware() -> TaskMiddlewareFunc {
    Arc::new(move |next: TaskHandlerFunc| {
        let next = next.clone();
        Arc::new(
            move |ctx: Arc<TaskContext>, et: TaskEventType, task: &mut Task| {
                if et == TaskEventType::Read {
                    let secrets = HashMap::new();
                    on_read_task(task, &secrets);
                }
                next(ctx, et, task)
            },
        )
    })
}
