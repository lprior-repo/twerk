//! Webhook module for the coordinator
//!
//! Handles triggering of job and task webhooks using a Data-Calc-Actions approach.

#![deny(clippy::unwrap_used)]
#![deny(clippy::expect_used)]
#![deny(clippy::panic)]
#![warn(clippy::pedantic)]
#![forbid(unsafe_code)]

use anyhow::Result;
use std::sync::Arc;
use twerk_core::job::Job;
use twerk_core::task::{Task, TaskSummary};
use twerk_core::webhook::{self, Webhook};
use twerk_infrastructure::datastore::Datastore;

// ── Actions ───────────────────────────────────────────────────

/// Fires webhooks for a job based on the event type.
///
/// This is an **Action** that spawns background tasks for network calls.
pub fn fire_job_webhooks(job: &Job, event: &str) {
    let event = event.to_string();

    // Pure Calculation: Filter webhooks that match the event
    let matching_webhooks = job.webhooks.as_ref().map_or_else(Vec::new, |whs| {
        whs.iter()
            .filter(|wh| wh.event.as_deref().is_some_and(|e| e == event))
            .cloned()
            .collect::<Vec<_>>()
    });

    // Action: Spawn blocking tasks for network calls
    let job_clone = job.clone();
    for wh in matching_webhooks {
        let job = job_clone.clone();
        tokio::task::spawn_blocking(move || {
            if let Err(e) = call_job_webhook(&wh, &job) {
                tracing::error!(
                    url = wh.url.as_deref().unwrap_or("unknown"),
                    error = %e,
                    "[Webhook] job webhook failed"
                );
            }
        });
    }
}

/// Fires webhooks for a task based on the event type.
///
/// This is an **Action** that retrieves the job and spawns background tasks.
///
/// # Errors
/// Returns error if the datastore query fails.
pub async fn fire_task_webhooks(ds: Arc<dyn Datastore>, task: &Task, event: &str) -> Result<()> {
    let job_id = task
        .job_id
        .as_ref()
        .map(std::string::ToString::to_string)
        .ok_or_else(|| anyhow::anyhow!("task has no job_id"))?;

    let job = ds.get_job_by_id(&job_id).await?;
    let event = event.to_string();

    // Pure Calculation: Filter webhooks and create summary
    let matching_webhooks = job.webhooks.as_ref().map_or_else(Vec::new, |whs| {
        whs.iter()
            .filter(|wh| wh.event.as_deref().is_some_and(|e| e == event))
            .cloned()
            .collect::<Vec<_>>()
    });

    if matching_webhooks.is_empty() {
        return Ok(());
    }

    let summary = twerk_core::task::new_task_summary(task);

    // Action: Spawn blocking tasks
    for wh in matching_webhooks {
        let summary = summary.clone();
        tokio::task::spawn_blocking(move || {
            if let Err(e) = call_task_webhook(&wh, &summary) {
                tracing::error!(
                    url = wh.url.as_deref().unwrap_or("unknown"),
                    error = %e,
                    "[Webhook] task webhook failed"
                );
            }
        });
    }

    Ok(())
}

// ── Internal Actions ──────────────────────────────────────────

/// Performs the blocking webhook call for a job.
fn call_job_webhook(wh: &Webhook, job: &Job) -> Result<()> {
    webhook::call(wh, job).map_err(|e| anyhow::anyhow!(e))
}

/// Performs the blocking webhook call for a task.
fn call_task_webhook(wh: &Webhook, task_summary: &TaskSummary) -> Result<()> {
    webhook::call(wh, task_summary).map_err(|e| anyhow::anyhow!(e))
}

#[cfg(test)]
mod tests {
    use super::*;
    use twerk_core::webhook::Webhook;

    #[tokio::test]
    async fn should_fire_webhook_with_matching_event() {
        let job = Job {
            webhooks: Some(vec![Webhook {
                url: Some("http://localhost:8080".to_string()),
                event: Some("job.Completed".to_string()),
                ..Default::default()
            }]),
            ..Default::default()
        };

        // This just verifies the logic flow doesn't panic
        fire_job_webhooks(&job, "job.Completed");
    }
}
