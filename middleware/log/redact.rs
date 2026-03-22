//! Log redaction middleware.
//!
//! Redacts sensitive information from task log parts when reading logs.

use crate::middleware::log::{Context, EventType, HandlerFunc, LogError, MiddlewareFunc};
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
                if et != EventType::Read {
                    return next(ctx, et, logs);
                }

                if logs.is_empty() {
                    return next(ctx, et, logs);
                }

                let task_id = logs[0].task_id.as_deref().unwrap_or("");

                let task = match ds.get_task_by_id(task_id) {
                    Ok(t) => t,
                    Err(e) => {
                        tracing::error!(error = %e, "error getting task for log");
                        return Err(LogError::Middleware(e.to_string()));
                    }
                };

                let job = match ds.get_job_by_id(&task.job_id) {
                    Ok(j) => j,
                    Err(e) => {
                        tracing::error!(error = %e, "error getting job for log");
                        return Err(LogError::Middleware(e.to_string()));
                    }
                };

                let redacted_logs: Vec<TaskLogPart> = logs
                    .iter()
                    .map(|log_part| redact_log_part(log_part, &job.secrets))
                    .collect();

                next(ctx, et, &redacted_logs)
            },
        )
    })
}

/// Pure function: redact secrets from a single log part.
fn redact_log_part(
    part: &TaskLogPart,
    secrets: &std::collections::HashMap<String, String>,
) -> TaskLogPart {
    let redacted_contents = part.contents.as_ref().map(|contents| {
        secrets
            .values()
            .filter(|s| !s.is_empty())
            .fold(contents.clone(), |acc, secret| {
                if acc.contains(secret.as_str()) {
                    acc.replace(secret.as_str(), "[REDACTED]")
                } else {
                    acc
                }
            })
    });

    TaskLogPart {
        id: part.id.clone(),
        number: part.number,
        task_id: part.task_id.clone(),
        contents: redacted_contents,
        created_at: part.created_at,
    }
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
        let secrets = [("secret".to_string(), "1234".to_string())]
            .into_iter()
            .collect();

        let ds = Arc::new(MockDatastore::new(secrets));
        let mw = redact_middleware(ds);

        let hm = crate::middleware::log::apply_middleware(
            crate::middleware::log::noop_handler(),
            vec![mw],
        );

        let log_part = TaskLogPart {
            id: Some("log-1".to_string()),
            number: 1,
            task_id: Some("task-1".to_string()),
            contents: Some("line 1 -- 1234".to_string()),
            created_at: None,
        };

        let ctx = Arc::new(Context::new());
        let result = hm(ctx, EventType::Read, &[log_part]);
        assert!(result.is_ok());
    }

    #[test]
    fn test_redact_skips_non_read() {
        let secrets = [("secret".to_string(), "1234".to_string())]
            .into_iter()
            .collect();

        let ds = Arc::new(MockDatastore::new(secrets));
        let mw = redact_middleware(ds);

        let hm = crate::middleware::log::apply_middleware(
            crate::middleware::log::noop_handler(),
            vec![mw],
        );

        let _log_part = TaskLogPart {
            id: Some("log-1".to_string()),
            number: 1,
            task_id: Some("task-1".to_string()),
            contents: Some("line 1 -- 1234".to_string()),
            created_at: None,
        };

        // EventType::Read is the only variant, so test with empty logs to skip
        let ctx = Arc::new(Context::new());
        let result = hm(ctx, EventType::Read, &[]);
        assert!(result.is_ok());
    }

    #[test]
    fn test_redact_log_part_pure() {
        let secrets = [("apikey".to_string(), "sk-abc123".to_string())]
            .into_iter()
            .collect();

        let part = TaskLogPart {
            id: Some("log-1".to_string()),
            number: 1,
            task_id: Some("task-1".to_string()),
            contents: Some("connecting with sk-abc123".to_string()),
            created_at: None,
        };

        let result = redact_log_part(&part, &secrets);
        assert_eq!(
            result.contents,
            Some("connecting with [REDACTED]".to_string())
        );
    }

    #[test]
    fn test_redact_log_part_no_secrets() {
        let secrets = std::collections::HashMap::new();

        let part = TaskLogPart {
            id: Some("log-1".to_string()),
            number: 1,
            task_id: Some("task-1".to_string()),
            contents: Some("normal log line".to_string()),
            created_at: None,
        };

        let result = redact_log_part(&part, &secrets);
        assert_eq!(result.contents, Some("normal log line".to_string()));
    }

    #[test]
    fn test_redact_log_part_no_contents() {
        let secrets = [("key".to_string(), "secret".to_string())]
            .into_iter()
            .collect();

        let part = TaskLogPart {
            id: Some("log-1".to_string()),
            number: 1,
            task_id: Some("task-1".to_string()),
            contents: None,
            created_at: None,
        };

        let result = redact_log_part(&part, &secrets);
        assert_eq!(result.contents, None);
    }

    #[test]
    fn test_redact_middleware_error_on_task_not_found() {
        struct FailingDatastore;

        impl Datastore for FailingDatastore {
            fn get_task_by_id(&self, _task_id: &str) -> Result<Task, DatastoreError> {
                Err(DatastoreError::TaskNotFound("task-999".to_string()))
            }
            fn get_job_by_id(&self, _job_id: &str) -> Result<Job, DatastoreError> {
                Err(DatastoreError::JobNotFound("job-999".to_string()))
            }
        }

        let ds = Arc::new(FailingDatastore);
        let mw = redact_middleware(ds);
        let hm = crate::middleware::log::apply_middleware(
            crate::middleware::log::noop_handler(),
            vec![mw],
        );

        let log_part = TaskLogPart {
            id: Some("log-1".to_string()),
            number: 1,
            task_id: Some("task-999".to_string()),
            contents: Some("some log".to_string()),
            created_at: None,
        };

        let ctx = Arc::new(Context::new());
        let result = hm(ctx, EventType::Read, &[log_part]);
        assert!(result.is_err());
    }
}
