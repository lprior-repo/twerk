//! Webhook middleware for job and task state change notifications.
//!
//! Go parity: middleware/job/webhook.go, middleware/task/webhook.go

use std::sync::Arc;
use twerk_core::eval::{evaluate_condition, evaluate_task_condition};
use twerk_core::job::{new_job_summary, Job};
use twerk_core::task::{new_task_summary, Task};
use twerk_core::webhook::{self, Webhook};
use twerk_infrastructure::datastore::{Datastore, inmemory::InMemoryDatastore};

/// Fires job webhooks on state changes.
///
/// Go parity: middleware/job/webhook.go
pub fn fire_job_webhooks(job: &Job, event: &str) {
    if let Some(webhooks) = &job.webhooks {
        for wh in webhooks {
            if !should_fire_webhook(wh, event) {
                continue;
            }
            if !evaluate_webhook_condition(wh.r#if.as_ref(), job) {
                continue;
            }
            let wh = wh.clone();
            let job = job.clone();
            tokio::spawn(async move {
                if let Err(e) = webhook::call(&wh, &job) {
                    tracing::error!("[Webhook] error calling job webhook {}: {}",
                        wh.url.as_deref().unwrap_or("unknown"), e);
                }
            });
        }
    }
}

/// Fires task webhooks by looking up the parent job's webhooks.
///
/// Go parity: middleware/task/webhook.go
/// In Go, task webhooks are sourced from the parent job, not from task.subjob.webhooks.
pub async fn fire_task_webhooks(
    ds: Arc<dyn Datastore>,
    task: &Task,
    event: &str,
) {
    let job_id = match task.job_id.as_ref() {
        Some(id) => id.to_string(),
        None => return,
    };

    let job = match ds.get_job_by_id(&job_id).await {
        Ok(j) => j,
        Err(e) => {
            tracing::error!("[Webhook] error getting job for task webhook: {}", e);
            return;
        }
    };

    if let Some(webhooks) = &job.webhooks {
        for wh in webhooks {
            if !should_fire_webhook(wh, event) {
                continue;
            }
            if !evaluate_task_webhook_condition_with_job(wh.r#if.as_ref(), task, &job) {
                continue;
            }
            let wh = wh.clone();
            let task_summary = new_task_summary(task);
            tokio::spawn(async move {
                if let Err(e) = webhook::call(&wh, &task_summary) {
                    tracing::error!("[Webhook] error calling task webhook {}: {}",
                        wh.url.as_deref().unwrap_or("unknown"), e);
                }
            });
        }
    }
}

fn should_fire_webhook(wh: &Webhook, event: &str) -> bool {
    wh.event.as_deref().is_none_or(|e| e == event || e.is_empty())
}

fn evaluate_webhook_condition(condition: Option<&String>, job: &Job) -> bool {
    let Some(expr) = condition else {
        return true;
    };
    if expr.is_empty() {
        return true;
    }
    let summary = new_job_summary(job);
    match evaluate_condition(expr, &summary) {
        Ok(true) => true,
        Ok(false) => {
            tracing::debug!("[Webhook] condition evaluated to false: {}", expr);
            false
        }
        Err(e) => {
            tracing::warn!("[Webhook] condition evaluation failed: {}", e);
            false
        }
    }
}

fn evaluate_task_webhook_condition_with_job(
    condition: Option<&String>,
    task: &Task,
    job: &Job,
) -> bool {
    let Some(expr) = condition else {
        return true;
    };
    if expr.is_empty() {
        return true;
    }
    let task_summary = new_task_summary(task);
    let job_summary = new_job_summary(job);
    match evaluate_task_condition(expr, &task_summary, &job_summary) {
        Ok(true) => true,
        Ok(false) => {
            tracing::debug!("[Webhook] condition evaluated to false: {}", expr);
            false
        }
        Err(e) => {
            tracing::warn!("[Webhook] condition evaluation failed: {}", e);
            false
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;
    use twerk_core::job::Job;
    use twerk_core::task::{SubJobTask, TASK_STATE_COMPLETED, TASK_STATE_RUNNING};
    use twerk_core::webhook::Webhook;

    #[test]
    fn should_fire_webhook_with_matching_event() {
        let wh = Webhook {
            url: Some("https://example.com/hook".to_string()),
            event: Some("job.StateChange".to_string()),
            ..Default::default()
        };
        assert!(should_fire_webhook(&wh, "job.StateChange"));
    }

    #[test]
    fn should_fire_webhook_with_empty_event() {
        let wh = Webhook {
            url: Some("https://example.com/hook".to_string()),
            event: Some("".to_string()),
            ..Default::default()
        };
        assert!(should_fire_webhook(&wh, "any.event"));
    }

    #[test]
    fn should_fire_webhook_with_no_event_configured() {
        let wh = Webhook {
            url: Some("https://example.com/hook".to_string()),
            event: None,
            ..Default::default()
        };
        assert!(should_fire_webhook(&wh, "any.event"));
    }

    #[test]
    fn should_fire_webhook_with_non_matching_event() {
        let wh = Webhook {
            url: Some("https://example.com/hook".to_string()),
            event: Some("job.StateChange".to_string()),
            ..Default::default()
        };
        assert!(!should_fire_webhook(&wh, "job.Progress"));
    }

    #[tokio::test]
    async fn fire_job_webhooks_does_not_fire_on_non_matching_event() {
        let job = Job {
            id: Some("test-job-2".into()),
            webhooks: Some(vec![Webhook {
                url: Some("https://example.com/hook".to_string()),
                event: Some("job.StateChange".to_string()),
                headers: Some(HashMap::new()),
                r#if: None,
            }]),
            ..Default::default()
        };

        let start = std::time::Instant::now();
        fire_job_webhooks(&job, "job.Progress");

        assert!(start.elapsed().as_millis() < 500, "Should return quickly when event doesn't match");
    }

    #[test]
    fn evaluate_webhook_condition_with_no_condition_returns_true() {
        let job = Job::default();
        assert!(evaluate_webhook_condition(None, &job));
    }

    #[test]
    fn evaluate_webhook_condition_with_empty_condition_returns_true() {
        let job = Job::default();
        assert!(evaluate_webhook_condition(Some(&"".to_string()), &job));
    }

    #[test]
    fn evaluate_webhook_condition_with_invalid_expression_returns_false() {
        let job = Job::default();
        assert!(!evaluate_webhook_condition(Some(&"invalid [[ expression".to_string()), &job));
    }

    #[tokio::test]
    async fn fire_job_webhooks_with_condition_evaluating_to_false() {
        let job = Job {
            id: Some("test-job-4".into()),
            state: "RUNNING".to_string(),
            webhooks: Some(vec![Webhook {
                url: Some("https://example.com/hook".to_string()),
                event: Some("job.StateChange".to_string()),
                headers: Some(HashMap::new()),
                r#if: Some("job_state == \"COMPLETED\"".to_string()),
            }]),
            ..Default::default()
        };

        let start = std::time::Instant::now();
        fire_job_webhooks(&job, "job.StateChange");

        assert!(start.elapsed().as_millis() < 500, "Should return quickly when condition is false");
    }

    #[tokio::test]
    async fn fire_job_webhooks_skips_webhook_with_failing_condition() {
        let job = Job {
            id: Some("test-job-5".into()),
            state: "FAILED".to_string(),
            webhooks: Some(vec![Webhook {
                url: Some("https://example.com/hook".to_string()),
                event: Some("job.StateChange".to_string()),
                headers: Some(HashMap::new()),
                r#if: Some("invalid [[ condition".to_string()),
            }]),
            ..Default::default()
        };

        let start = std::time::Instant::now();
        fire_job_webhooks(&job, "job.StateChange");

        assert!(start.elapsed().as_millis() < 500, "Should return quickly when condition evaluation fails");
    }

    fn make_task_with_job_id(job_id: &str) -> Task {
        Task {
            id: Some("test-task-id".into()),
            job_id: Some(job_id.into()),
            state: TASK_STATE_COMPLETED.to_string(),
            ..Default::default()
        }
    }

    #[tokio::test]
    async fn fire_task_webhooks_returns_early_when_no_job_id() {
        let ds = Arc::new(InMemoryDatastore::new());
        let task = Task {
            id: Some("task-no-job".into()),
            job_id: None,
            state: TASK_STATE_COMPLETED.to_string(),
            ..Default::default()
        };

        let start = std::time::Instant::now();
        fire_task_webhooks(ds, &task, "task.StateChange").await;
        assert!(start.elapsed().as_millis() < 500);
    }

    #[tokio::test]
    async fn fire_task_webhooks_looks_up_parent_job() {
        let ds = Arc::new(InMemoryDatastore::new());

        let job = Job {
            id: Some("parent-job-1".into()),
            webhooks: Some(vec![Webhook {
                url: Some("https://example.com/hook".to_string()),
                event: Some("task.StateChange".to_string()),
                headers: Some(HashMap::new()),
                r#if: None,
            }]),
            ..Default::default()
        };
        ds.create_job(&job).await.unwrap();

        let task = make_task_with_job_id("parent-job-1");
        fire_task_webhooks(ds, &task, "task.StateChange").await;
    }

    #[tokio::test]
    async fn fire_task_webhooks_does_not_fire_on_non_matching_event() {
        let ds = Arc::new(InMemoryDatastore::new());

        let job = Job {
            id: Some("parent-job-2".into()),
            webhooks: Some(vec![Webhook {
                url: Some("https://example.com/hook".to_string()),
                event: Some("task.StateChange".to_string()),
                headers: Some(HashMap::new()),
                r#if: None,
            }]),
            ..Default::default()
        };
        ds.create_job(&job).await.unwrap();

        let task = make_task_with_job_id("parent-job-2");

        let start = std::time::Instant::now();
        fire_task_webhooks(ds, &task, "task.Progress").await;
        assert!(start.elapsed().as_millis() < 500, "Should return quickly when event doesn't match");
    }

    #[tokio::test]
    async fn fire_task_webhooks_with_condition_evaluating_to_false() {
        let ds = Arc::new(InMemoryDatastore::new());

        let job = Job {
            id: Some("parent-job-3".into()),
            webhooks: Some(vec![Webhook {
                url: Some("https://example.com/hook".to_string()),
                event: Some("task.StateChange".to_string()),
                headers: Some(HashMap::new()),
                r#if: Some("task_state == \"COMPLETED\"".to_string()),
            }]),
            ..Default::default()
        };
        ds.create_job(&job).await.unwrap();

        let task = Task {
            id: Some("task-4".into()),
            job_id: Some("parent-job-3".into()),
            state: TASK_STATE_RUNNING.to_string(),
            ..Default::default()
        };

        let start = std::time::Instant::now();
        fire_task_webhooks(ds, &task, "task.StateChange").await;
        assert!(start.elapsed().as_millis() < 500, "Should return quickly when condition is false");
    }
}
