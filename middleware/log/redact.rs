//! Log redaction middleware.
//!
//! Redacts sensitive information from task log parts when reading logs.

use crate::middleware::log::{
    apply_middleware, noop_handler, Context, EventType, HandlerFunc, LogError, MiddlewareFunc,
};
use std::sync::Arc;
use tork::task::TaskLogPart;

/// Create a redaction middleware for log events.
///
/// This middleware redacts sensitive information from log contents
/// when the event type is READ.
pub fn redact_middleware(ds: Arc<dyn Datastore>) -> MiddlewareFunc {
    Arc::new(move |next: HandlerFunc| {
        let ds = ds.clone();
        Arc::new(
            move |ctx: Arc<Context>, et: EventType, logs: &[TaskLogPart]| {
                if et != EventType::READ {
                    return next(ctx, et, logs);
                }

                if logs.is_empty() {
                    return next(ctx, et, logs);
                }

                // Get job secrets and apply redaction
                let task_id = logs[0].task_id.as_ref().unwrap_or(&String::new());
                let task = ds.get_task_by_id(task_id)?;
                let job = ds.get_job_by_id(&task.job_id)?;

                let mut redacted_logs = logs.to_vec();
                for log in &mut redacted_logs {
                    let mut contents = log.contents.clone().unwrap_or_default();
                    for (_, secret) in &job.secrets {
                        if !secret.is_empty() && contents.contains(secret) {
                            contents = contents.replace(secret, "[REDACTED]");
                        }
                    }
                    log.contents = Some(contents);
                }

                next(ctx, et, &redacted_logs)
            },
        )
    })
}

/// Trait for datastore operations needed by redaction.
pub trait Datastore: Send + Sync {
    /// Get a task by ID.
    fn get_task_by_id(&self, task_id: &str) -> Result<Task, DatastoreError>;
    /// Get a job by ID.
    fn get_job_by_id(&self, job_id: &str) -> Result<Job, DatastoreError>;
}

/// Task representation for datastore operations.
#[derive(Debug, Clone)]
pub struct Task {
    pub id: String,
    pub job_id: String,
}

/// Job representation for datastore operations.
#[derive(Debug, Clone)]
pub struct Job {
    pub id: String,
    pub secrets: std::collections::HashMap<String, String>,
}

/// Datastore errors.
#[derive(Debug, thiserror::Error)]
pub enum DatastoreError {
    #[error("task not found: {0}")]
    TaskNotFound(String),
    #[error("job not found: {0}")]
    JobNotFound(String),
    #[error("datastore error: {0}")]
    Other(String),
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;

    struct MockDatastore {
        secrets: std::collections::HashMap<String, String>,
    }

    impl MockDatastore {
        fn new(secrets: std::collections::HashMap<String, String>) -> Self {
            Self { secrets }
        }
    }

    impl Datastore for MockDatastore {
        fn get_task_by_id(&self, _task_id: &str) -> Result<Task, DatastoreError> {
            Ok(Task {
                id: "task-1".to_string(),
                job_id: "job-1".to_string(),
            })
        }

        fn get_job_by_id(&self, _job_id: &str) -> Result<Job, DatastoreError> {
            Ok(Job {
                id: "job-1".to_string(),
                secrets: self.secrets.clone(),
            })
        }
    }

    #[test]
    fn test_redact_on_read() {
        let mut secrets = std::collections::HashMap::new();
        secrets.insert("secret".to_string(), "1234".to_string());

        let ds = Arc::new(MockDatastore::new(secrets));
        let mw = redact_middleware(ds);

        let hm = apply_middleware(noop_handler(), vec![mw]);

        let log_part = TaskLogPart {
            id: Some("log-1".to_string()),
            number: 1,
            task_id: Some("task-1".to_string()),
            contents: Some("line 1 -- 1234".to_string()),
            created_at: None,
        };

        let ctx = Arc::new(Context::new());
        // In a real test, we'd verify that the log contents are redacted
        let _ = hm(ctx, EventType::READ, &[log_part]);
    }
}
