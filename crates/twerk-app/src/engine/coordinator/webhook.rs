//! Webhook module for the coordinator
//!
//! Handles triggering of job and task webhooks using a Data-Calc-Actions approach.
//! The async webhook call uses reqwest with `tokio::time::sleep` for proper non-blocking retries.

#![deny(clippy::unwrap_used)]
#![deny(clippy::expect_used)]
#![deny(clippy::panic)]
#![warn(clippy::pedantic)]
#![forbid(unsafe_code)]

use anyhow::Result;
use reqwest::Client;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::Semaphore;
use tracing::instrument;
use twerk_common::constants::DEFAULT_TASK_NAME;
use twerk_core::job::Job;
use twerk_core::task::Task;
use twerk_core::webhook::{
    self, Webhook, WEBHOOK_DEFAULT_MAX_ATTEMPTS, WEBHOOK_DEFAULT_TIMEOUT_SECS,
};
use twerk_infrastructure::datastore::Datastore;

// ── Typed errors for webhook operations ────────────────────────────

#[derive(Debug, thiserror::Error)]
enum WebhookError {
    #[error("task has no job_id")]
    MissingJobId,
}

// ── Async HTTP Client ───────────────────────────────────────────

/// Async webhook caller with retry logic using `tokio::time::sleep`.
///
/// This is the **Action** layer - pure async I/O with proper backoff.
#[instrument(name = "call_webhook_async", skip_all, fields(url = %wh.url.as_deref().map_or("unknown", |s| s)))]
async fn call_webhook_async(
    wh: &Webhook,
    body: &impl serde::Serialize,
) -> Result<(), webhook::WebhookError> {
    let url = wh
        .url
        .as_ref()
        .ok_or_else(|| webhook::WebhookError::NonRetryableError("missing url".to_string(), 0))?;

    let serialized =
        serde_json::to_string(body).map_err(|_| webhook::WebhookError::SerializationError)?;

    let client = Client::builder()
        .timeout(Duration::from_secs(WEBHOOK_DEFAULT_TIMEOUT_SECS))
        .build()
        .map_err(|_| webhook::WebhookError::ClientBuildError)?;

    for attempt in 0..WEBHOOK_DEFAULT_MAX_ATTEMPTS {
        let current_attempt = attempt + 1;
        let remaining = WEBHOOK_DEFAULT_MAX_ATTEMPTS - current_attempt;

        // Build request with headers
        let mut request = client
            .post(url)
            .header("Content-Type", "application/json; charset=UTF-8")
            .body(serialized.clone());

        if let Some(ref headers) = wh.headers {
            for (k, v) in headers {
                request = request.header(k, v);
            }
        }

        // Execute HTTP request
        let status = if let Ok(resp) = request.send().await {
            resp.status().as_u16()
        } else {
            tracing::info!(
                webhook_url = %url,
                attempt = current_attempt,
                "[Webhook] request to {} failed with connection error",
                url
            );
            continue;
        };

        // Check result - success (2xx), non-retryable, or retryable
        if (200..300).contains(&status) {
            return Ok(());
        }

        if !webhook::is_retryable(status) {
            return Err(webhook::WebhookError::NonRetryableError(
                url.clone(),
                status,
            ));
        }

        tracing::info!(
            webhook_url = %url,
            status,
            attempt = current_attempt,
            "[Webhook] request to {} failed with {}",
            url, status
        );

        if !webhook::should_retry(Ok(status), remaining) {
            break;
        }

        // Async backoff - does NOT block the thread
        tokio::time::sleep(webhook::backoff_duration(current_attempt)).await;
    }

    Err(webhook::WebhookError::MaxAttemptsExceeded(
        url.clone(),
        WEBHOOK_DEFAULT_MAX_ATTEMPTS,
    ))
}

// ── Actions ───────────────────────────────────────────────────

/// Fires webhooks for a job based on the event type.
///
/// This is an **Action** that spawns background tasks for async network calls.
#[instrument(name = "fire_job_webhooks", skip_all, fields(event = %event))]
pub async fn fire_job_webhooks(job: &Job, event: &str) {
    let event = event.to_string();

    // Pure Calculation: Filter webhooks that match the event
    let matching_webhooks = job.webhooks.as_ref().map_or_else(Vec::new, |whs| {
        whs.iter()
            .filter(|wh| wh.event.as_deref().is_some_and(|e| e == event))
            .cloned()
            .collect::<Vec<_>>()
    });

    // Action: Spawn async tasks for network calls (bounded concurrency)
    let semaphore = Arc::new(Semaphore::new(16));
    for wh in matching_webhooks {
        let job = job.clone();
        let wh = wh.clone();
        let permit = semaphore.clone().acquire_owned().await.unwrap();
        tokio::spawn(async move {
            let _permit = permit;
            if let Err(e) = call_webhook_async(&wh, &job).await {
                tracing::error!(
                    url = wh.url.as_deref().map_or(DEFAULT_TASK_NAME, |s| s),
                    error = %e,
                    "[Webhook] job webhook failed"
                );
            }
        });
    }
}

/// Fires webhooks for a task based on the event type.
///
/// This is an **Action** that retrieves the job and spawns async tasks.
///
/// # Errors
/// Returns error if the datastore query fails.
#[instrument(name = "fire_task_webhooks", skip_all, fields(event = %event))]
pub async fn fire_task_webhooks(ds: Arc<dyn Datastore>, task: &Task, event: &str) -> Result<()> {
    let job_id = task
        .job_id
        .as_ref()
        .map(std::string::ToString::to_string)
        .ok_or_else(|| WebhookError::MissingJobId)?;

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

    // Action: Spawn async tasks (bounded concurrency)
    let semaphore = Arc::new(Semaphore::new(16));
    for wh in matching_webhooks {
        let summary = summary.clone();
        let wh = wh.clone();
        let permit = semaphore.clone().acquire_owned().await.unwrap();
        tokio::spawn(async move {
            let _permit = permit;
            if let Err(e) = call_webhook_async(&wh, &summary).await {
                tracing::error!(
                    url = wh.url.as_deref().map_or(DEFAULT_TASK_NAME, |s| s),
                    error = %e,
                    "[Webhook] task webhook failed"
                );
            }
        });
    }

    Ok(())
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
        fire_job_webhooks(&job, "job.Completed").await;
    }
}
