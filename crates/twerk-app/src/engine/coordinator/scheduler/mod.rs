//! Task scheduler for the coordinator
//!
//! Schedules tasks based on their type: regular, parallel, each, or subjob.

#![deny(clippy::unwrap_used)]
#![deny(clippy::expect_used)]
#![deny(clippy::panic)]
#![warn(clippy::pedantic)]
#![allow(
    clippy::cast_possible_wrap,
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss
)]

use anyhow::Result;
use std::sync::Arc;

use tracing::instrument;

mod each;
mod parallel;
mod regular;
mod shared;
mod subjob;

#[cfg(test)]
mod mock;
#[cfg(test)]
mod tests;

// ── Typed errors for scheduler operations ──────────────────────────

#[derive(Debug, thiserror::Error)]
pub(super) enum SchedulerError {
    #[error("task ID required for {scheduler} scheduling")]
    TaskIdRequired { scheduler: String },
    #[error("job ID required for {scheduler} scheduling")]
    JobIdRequired { scheduler: String },
    #[error("missing {scheduler} config")]
    MissingConfig { scheduler: String },
    #[error("missing parallel tasks")]
    MissingParallelTasks,
    #[error("each list must be an array")]
    EachListMustBeArray,
    #[error("missing each task template")]
    MissingEachTemplate,
    #[error("failed to evaluate {context}: {error}")]
    Evaluation { context: String, error: String },
}

/// Scheduler handles task scheduling based on task type.
pub struct Scheduler {
    ds: Arc<dyn twerk_infrastructure::datastore::Datastore>,
    broker: Arc<dyn twerk_infrastructure::broker::Broker>,
}

impl Scheduler {
    /// Creates a new scheduler instance.
    #[must_use]
    pub fn new(
        ds: Arc<dyn twerk_infrastructure::datastore::Datastore>,
        broker: Arc<dyn twerk_infrastructure::broker::Broker>,
    ) -> Self {
        Self { ds, broker }
    }

    /// Schedules a task based on its type (regular, parallel, each, or subjob).
    /// # Errors
    /// Returns error if task scheduling fails.
    #[instrument(name = "schedule_task", skip_all)]
    pub async fn schedule_task(&self, task: twerk_core::task::Task) -> Result<()> {
        if task.parallel.is_some() {
            self.schedule_parallel_task(task).await
        } else if task.each.is_some() {
            self.schedule_each_task(task).await
        } else if task.subjob.is_some() {
            self.schedule_subjob_task(task).await
        } else {
            self.schedule_regular_task(task).await
        }
    }
}
