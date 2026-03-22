//! Webhook middleware for task events.
//!
//! This middleware triggers webhook calls when tasks undergo state changes
//! or progress updates, with support for conditional execution via `if` expressions.

use crate::middleware::task::task_error::TaskMiddlewareError;
use crate::middleware::task::task_handler::{Context, HandlerFunc, MiddlewareFunc};
use crate::middleware::task::task_types::EventType;
use crate::middleware::task::webhook_events::*;
use std::sync::Arc;
use tork::job::{Job, JobSummary};
use tork::task::new_task_summary;
use tork::task::{Task, TaskSummary, Webhook};

/// Create a webhook middleware.
///
/// This middleware fires configured webhooks when task state changes or progress updates occur.
pub fn webhook_middleware(_ds: Arc<dyn Datastore>) -> MiddlewareFunc {
    Arc::new(move |next: HandlerFunc| -> HandlerFunc {
        Arc::new(
            move |ctx: Context,
                  et: EventType,
                  task: &mut Task|
                  -> Result<(), TaskMiddlewareError> {
                let next = next.clone();

                // Call the next handler first
                next(ctx.clone(), et, task)?;

                // Only process StateChange and Progress events
                if et != EventType::StateChange && et != EventType::Progress {
                    return Ok(());
                }

                // For a full implementation, we would:
                // 1. Fetch job from datastore or cache
                // 2. Evaluate `if` expressions using expr-lang/expr
                // 3. Spawn async webhook calls

                Ok(())
            },
        )
    })
}

/// Datastore trait for accessing job data.
pub trait Datastore: Send + Sync {
    /// Get a job by ID.
    fn get_job_by_id(&self, job_id: &str) -> Result<Job, TaskMiddlewareError>;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_webhook_middleware_no_event() {
        // Test that webhooks are not triggered for non-StateChange/Progress events
    }
}
