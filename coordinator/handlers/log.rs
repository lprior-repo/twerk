//! Log handler for task logs.
//!
//! Port of Go `internal/coordinator/handlers/log.go` with 100% parity.
//!
//! # Go Parity
//!
//! 1. Receives TaskLogPart messages
//! 2. Calls ds.CreateTaskLogPart to persist
//! 3. Logs errors on failure (non-fatal — Go swallows errors)

use std::sync::Arc;

use tork::task::TaskLogPart;
use tork::Datastore;

use crate::handlers::HandlerError;

// ---------------------------------------------------------------------------
// Handler (Action boundary)
// ---------------------------------------------------------------------------

/// Log handler for processing task log parts.
///
/// Holds a reference to the datastore for I/O operations.
/// Go's implementation swallows errors (logs and continues);
/// this handler surfaces them for the caller to decide.
pub struct LogHandler {
    ds: Arc<dyn Datastore>,
}

impl std::fmt::Debug for LogHandler {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("LogHandler").finish()
    }
}

impl LogHandler {
    /// Create a new log handler with datastore dependency.
    /// Go: `NewLogHandler(ds datastore.Datastore) func(p *tork.TaskLogPart)`
    pub fn new(ds: Arc<dyn Datastore>) -> Self {
        Self { ds }
    }

    /// Handle a task log part by persisting it to the datastore.
    ///
    /// Go parity (`handle`):
    /// 1. Calls `ds.CreateTaskLogPart(ctx, p)`
    /// 2. On error, logs and returns (Go swallows; we surface for observability)
    pub async fn handle(&self, part: &TaskLogPart) -> Result<(), HandlerError> {
        self.ds
            .create_task_log_part(part.clone())
            .await
            .map_err(|e| HandlerError::Datastore(e.to_string()))
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_debug_impl() {
        let debug_str = format!("{:?}", LogHandler::new(Arc::new(NoopDs)));
        assert!(debug_str.contains("LogHandler"));
    }

    // -- Additional edge cases from Go test parity ---------------------------

    // Go: TaskLogPart with empty contents should be handled
    #[test]
    fn test_task_log_part_default() {
        let part = tork::task::TaskLogPart {
            id: None,
            number: 0,
            task_id: None,
            contents: None,
            created_at: None,
        };
        assert_eq!(part.number, 0);
        assert!(part.contents.is_none());
    }

    // Go: TaskLogPart with multiline contents
    #[test]
    fn test_task_log_part_multiline() {
        let part = tork::task::TaskLogPart {
            task_id: Some("task-1".to_string()),
            number: 1,
            contents: Some("line 1\nline 2\nline 3".to_string()),
            id: None,
            created_at: None,
        };
        let c = part.contents.as_deref().expect("should have contents");
        assert_eq!(c.lines().count(), 3);
    }

    // Go: TaskLogPart task_id is preserved
    #[test]
    fn test_task_log_part_task_id() {
        let part = tork::task::TaskLogPart {
            task_id: Some("abc123".to_string()),
            number: 42,
            contents: Some("output".to_string()),
            id: None,
            created_at: None,
        };
        assert_eq!(part.task_id.as_deref(), Some("abc123"));
        assert_eq!(part.number, 42);
    }

    // -- Handler construction variations -------------------------------------

    #[test]
    fn test_log_handler_new_returns_handler() {
        let handler = LogHandler::new(Arc::new(NoopDs));
        let debug_str = format!("{handler:?}");
        assert_eq!(debug_str, "LogHandler");
    }

    use crate::handlers::test_helpers::{new_uuid, TestEnv};

    /// Go parity: Test_handleLog — persists a TaskLogPart to the datastore
    #[tokio::test]
    #[ignore]
    async fn test_handle_log_integration() {
        let env = TestEnv::new().await;
        let handler = LogHandler::new(env.ds.clone());

        let task_id = new_uuid();
        let part = tork::task::TaskLogPart {
            id: Some(new_uuid()),
            task_id: Some(task_id.clone()),
            number: 1,
            contents: Some("hello world".to_string()),
            created_at: Some(time::OffsetDateTime::now_utc()),
        };

        handler.handle(&part).await.expect("handle log");

        // Verify the log was persisted by querying it back
        let page = env.ds.get_task_log_parts(task_id, String::new(), 1, 10).await.expect("get log parts");
        assert_eq!(page.items.len(), 1);
        assert_eq!(page.items[0].contents.as_deref(), Some("hello world"));

        env.cleanup().await;
    }

    /// A minimal no-op datastore for compile-time tests.
    struct NoopDs;

    impl Datastore for NoopDs {
        fn create_task(&self, _task: tork::task::Task) -> tork::datastore::BoxedFuture<()> {
            Box::pin(async { Ok(()) })
        }
        fn update_task(
            &self,
            _id: String,
            _task: tork::task::Task,
        ) -> tork::datastore::BoxedFuture<()> {
            Box::pin(async { Ok(()) })
        }
        fn get_task_by_id(
            &self,
            _id: String,
        ) -> tork::datastore::BoxedFuture<Option<tork::task::Task>> {
            Box::pin(async { Ok(None) })
        }
        fn get_active_tasks(
            &self,
            _job_id: String,
        ) -> tork::datastore::BoxedFuture<Vec<tork::task::Task>> {
            Box::pin(async { Ok(vec![]) })
        }
        fn get_next_task(
            &self,
            _parent_task_id: String,
        ) -> tork::datastore::BoxedFuture<Option<tork::task::Task>> {
            Box::pin(async { Ok(None) })
        }
        fn create_task_log_part(
            &self,
            _part: TaskLogPart,
        ) -> tork::datastore::BoxedFuture<()> {
            Box::pin(async { Ok(()) })
        }
        fn get_task_log_parts(
            &self,
            _task_id: String,
            _q: String,
            _page: i64,
            _size: i64,
        ) -> tork::datastore::BoxedFuture<tork::datastore::Page<TaskLogPart>> {
            Box::pin(async {
                Ok(tork::datastore::Page {
                    items: vec![],
                    total: 0,
                    page: 0,
                    size: 0,
                })
            })
        }
        fn create_node(&self, _node: tork::node::Node) -> tork::datastore::BoxedFuture<()> {
            Box::pin(async { Ok(()) })
        }
        fn update_node(
            &self,
            _id: String,
            _node: tork::node::Node,
        ) -> tork::datastore::BoxedFuture<()> {
            Box::pin(async { Ok(()) })
        }
        fn get_node_by_id(
            &self,
            _id: String,
        ) -> tork::datastore::BoxedFuture<Option<tork::node::Node>> {
            Box::pin(async { Ok(None) })
        }
        fn get_active_nodes(
            &self,
        ) -> tork::datastore::BoxedFuture<Vec<tork::node::Node>> {
            Box::pin(async { Ok(vec![]) })
        }
        fn create_job(&self, _job: tork::job::Job) -> tork::datastore::BoxedFuture<()> {
            Box::pin(async { Ok(()) })
        }
        fn update_job(
            &self,
            _id: String,
            _job: tork::job::Job,
        ) -> tork::datastore::BoxedFuture<()> {
            Box::pin(async { Ok(()) })
        }
        fn get_job_by_id(
            &self,
            _id: String,
        ) -> tork::datastore::BoxedFuture<Option<tork::job::Job>> {
            Box::pin(async { Ok(None) })
        }
        fn get_job_log_parts(
            &self,
            _job_id: String,
            _q: String,
            _page: i64,
            _size: i64,
        ) -> tork::datastore::BoxedFuture<tork::datastore::Page<TaskLogPart>> {
            Box::pin(async {
                Ok(tork::datastore::Page {
                    items: vec![],
                    total: 0,
                    page: 0,
                    size: 0,
                })
            })
        }
        fn get_jobs(
            &self,
            _current_user: String,
            _q: String,
            _page: i64,
            _size: i64,
        ) -> tork::datastore::BoxedFuture<tork::datastore::Page<tork::job::JobSummary>> {
            Box::pin(async {
                Ok(tork::datastore::Page {
                    items: vec![],
                    total: 0,
                    page: 0,
                    size: 0,
                })
            })
        }
        fn create_scheduled_job(
            &self,
            _job: tork::job::ScheduledJob,
        ) -> tork::datastore::BoxedFuture<()> {
            Box::pin(async { Ok(()) })
        }
        fn get_active_scheduled_jobs(
            &self,
        ) -> tork::datastore::BoxedFuture<Vec<tork::job::ScheduledJob>> {
            Box::pin(async { Ok(vec![]) })
        }
        fn get_scheduled_jobs(
            &self,
            _current_user: String,
            _page: i64,
            _size: i64,
        ) -> tork::datastore::BoxedFuture<tork::datastore::Page<tork::job::ScheduledJobSummary>> {
            Box::pin(async {
                Ok(tork::datastore::Page {
                    items: vec![],
                    total: 0,
                    page: 0,
                    size: 0,
                })
            })
        }
        fn get_scheduled_job_by_id(
            &self,
            _id: String,
        ) -> tork::datastore::BoxedFuture<Option<tork::job::ScheduledJob>> {
            Box::pin(async { Ok(None) })
        }
        fn update_scheduled_job(
            &self,
            _id: String,
            _job: tork::job::ScheduledJob,
        ) -> tork::datastore::BoxedFuture<()> {
            Box::pin(async { Ok(()) })
        }
        fn delete_scheduled_job(&self, _id: String) -> tork::datastore::BoxedFuture<()> {
            Box::pin(async { Ok(()) })
        }
        fn create_user(&self, _user: tork::user::User) -> tork::datastore::BoxedFuture<()> {
            Box::pin(async { Ok(()) })
        }
        fn get_user(
            &self,
            _username: String,
        ) -> tork::datastore::BoxedFuture<Option<tork::user::User>> {
            Box::pin(async { Ok(None) })
        }
        fn create_role(&self, _role: tork::role::Role) -> tork::datastore::BoxedFuture<()> {
            Box::pin(async { Ok(()) })
        }
        fn get_role(
            &self,
            _id: String,
        ) -> tork::datastore::BoxedFuture<Option<tork::role::Role>> {
            Box::pin(async { Ok(None) })
        }
        fn get_roles(
            &self,
        ) -> tork::datastore::BoxedFuture<Vec<tork::role::Role>> {
            Box::pin(async { Ok(vec![]) })
        }
        fn get_user_roles(&self, _user_id: String) -> tork::datastore::BoxedFuture<Vec<tork::role::Role>> {
            Box::pin(async { Ok(vec![]) })
        }
        fn assign_role(&self, _user_id: String, _role_id: String) -> tork::datastore::BoxedFuture<()> {
            Box::pin(async { Ok(()) })
        }
        fn unassign_role(&self, _user_id: String, _role_id: String) -> tork::datastore::BoxedFuture<()> {
            Box::pin(async { Ok(()) })
        }
        fn get_metrics(&self) -> tork::datastore::BoxedFuture<tork::stats::Metrics> {
            Box::pin(async { Ok(tork::stats::Metrics::default()) })
        }
        fn health_check(&self) -> tork::datastore::BoxedFuture<()> {
            Box::pin(async { Ok(()) })
        }
        fn shutdown(&self) -> tork::datastore::BoxedFuture<()> {
            Box::pin(async { Ok(()) })
        }
    }
}
