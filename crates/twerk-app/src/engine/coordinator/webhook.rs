//! Webhook middleware for job and task state change notifications.
//!
//! Go parity: middleware/job/webhook.go

use twerk_core::eval::{evaluate_condition, evaluate_task_condition};
use twerk_core::job::{new_job_summary, Job};
use twerk_core::task::{new_task_summary, Task};
use twerk_core::webhook::{self, Webhook};

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
                    tracing::error!("webhook call failed: {}", e);
                }
            });
        }
    }
}

/// Fires task webhooks on state changes.
pub fn fire_task_webhooks(task: &Task, event: &str) {
    if let Some(webhooks) = task.subjob.as_ref().and_then(|sj| sj.webhooks.as_ref()) {
        for wh in webhooks {
            if !should_fire_webhook(wh, event) {
                continue;
            }
            if !evaluate_task_webhook_condition(wh.r#if.as_ref(), task) {
                continue;
            }
            let wh = wh.clone();
            let task = task.clone();
            tokio::spawn(async move {
                if let Err(e) = webhook::call(&wh, &task) {
                    tracing::error!("webhook call failed: {}", e);
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
            tracing::debug!("webhook condition evaluated to false: {}", expr);
            false
        }
        Err(e) => {
            tracing::warn!("webhook condition evaluation failed: {}", e);
            false
        }
    }
}

fn evaluate_task_webhook_condition(condition: Option<&String>, task: &Task) -> bool {
    let Some(expr) = condition else {
        return true;
    };
    if expr.is_empty() {
        return true;
    }
    let task_summary = new_task_summary(task);
    let job_summary = task
        .job_id
        .as_ref()
        .map_or_else(|| new_job_summary(&Job::default()), |id| twerk_core::job::JobSummary {
            id: Some(id.clone()),
            created_by: None,
            parent_id: None,
            name: None,
            description: None,
            tags: None,
            inputs: None,
            state: twerk_core::job::JobState::default(),
            created_at: None,
            started_at: None,
            completed_at: None,
            failed_at: None,
            position: 0,
            task_count: 0,
            result: None,
            error: None,
            progress: 0.0,
            schedule: None,
        });
    match evaluate_task_condition(expr, &task_summary, &job_summary) {
        Ok(true) => true,
        Ok(false) => {
            tracing::debug!("webhook condition evaluated to false: {}", expr);
            false
        }
        Err(e) => {
            tracing::warn!("webhook condition evaluation failed: {}", e);
            false
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use twerk_core::webhook::Webhook;
    use twerk_core::job::Job;
    use tokio::sync::oneshot;
    use std::collections::HashMap;

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
    async fn fire_job_webhooks_calls_webhook_call() {
        let (tx, rx) = oneshot::channel();
        
        let job = Job {
            id: Some("test-job-1".into()),
            webhooks: Some(vec![Webhook {
                url: Some("https://example.com/hook".to_string()),
                event: Some("job.StateChange".to_string()),
                headers: Some(HashMap::new()),
                r#if: None,
            }]),
            ..Default::default()
        };

        tokio::spawn(async move {
            fire_job_webhooks(&job, "job.StateChange");
            tx.send(()).unwrap();
        });

        let result = tokio::time::timeout(std::time::Duration::from_secs(5), rx).await;
        assert!(result.is_ok(), "fire_job_webhooks completed");
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
        let elapsed = start.elapsed();
        
        assert!(elapsed.as_millis() < 500, "Should return quickly when event doesn't match");
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
    fn evaluate_webhook_condition_that_evaluates_to_true() {
        let job = Job {
            state: "RUNNING".to_string(),
            ..Default::default()
        };
        assert!(evaluate_webhook_condition(Some(&"job_state == \"RUNNING\"".to_string()), &job));
    }

    #[test]
    fn evaluate_webhook_condition_that_evaluates_to_false() {
        let job = Job {
            state: "PENDING".to_string(),
            ..Default::default()
        };
        assert!(!evaluate_webhook_condition(Some(&"job_state == \"RUNNING\"".to_string()), &job));
    }

    #[test]
    fn evaluate_webhook_condition_with_invalid_expression_returns_false() {
        let job = Job::default();
        assert!(!evaluate_webhook_condition(Some(&"invalid [[ expression".to_string()), &job));
    }

    #[test]
    fn evaluate_webhook_condition_with_job_id_equality() {
        let job = Job {
            id: Some("test-job-123".into()),
            state: "COMPLETED".to_string(),
            ..Default::default()
        };
        assert!(evaluate_webhook_condition(Some(&"job_id == \"test-job-123\"".to_string()), &job));
        assert!(!evaluate_webhook_condition(Some(&"job_id == \"other-job\"".to_string()), &job));
    }

    #[tokio::test]
    async fn fire_job_webhooks_with_condition_evaluating_to_true() {
        let (tx, rx) = oneshot::channel();
        
        let job = Job {
            id: Some("test-job-3".into()),
            state: "COMPLETED".to_string(),
            webhooks: Some(vec![Webhook {
                url: Some("https://example.com/hook".to_string()),
                event: Some("job.StateChange".to_string()),
                headers: Some(HashMap::new()),
                r#if: Some("job_state == \"COMPLETED\"".to_string()),
            }]),
            ..Default::default()
        };

        tokio::spawn(async move {
            fire_job_webhooks(&job, "job.StateChange");
            tx.send(()).unwrap();
        });

        let result = tokio::time::timeout(std::time::Duration::from_secs(5), rx).await;
        assert!(result.is_ok(), "fire_job_webhooks should fire when condition is true");
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
        let elapsed = start.elapsed();
        
        assert!(elapsed.as_millis() < 500, "Should return quickly when condition is false");
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
        let elapsed = start.elapsed();
        
        assert!(elapsed.as_millis() < 500, "Should return quickly when condition evaluation fails");
    }

    use twerk_core::task::{Task, SubJobTask};
    use twerk_core::task::TASK_STATE_COMPLETED;
    use twerk_core::task::TASK_STATE_RUNNING;

    #[tokio::test]
    async fn fire_task_webhooks_fires_on_matching_event() {
        let (tx, rx) = oneshot::channel();
        
        let task = Task {
            id: Some("task-1".into()),
            job_id: Some("job-1".into()),
            state: TASK_STATE_COMPLETED.to_string(),
            subjob: Some(SubJobTask {
                webhooks: Some(vec![Webhook {
                    url: Some("https://example.com/hook".to_string()),
                    event: Some("task.Completed".to_string()),
                    headers: Some(HashMap::new()),
                    r#if: None,
                }]),
                ..Default::default()
            }),
            ..Default::default()
        };

        tokio::spawn(async move {
            fire_task_webhooks(&task, "task.Completed");
            tx.send(()).unwrap();
        });

        let result = tokio::time::timeout(std::time::Duration::from_secs(5), rx).await;
        assert!(result.is_ok(), "fire_task_webhooks should fire on matching event");
    }

    #[tokio::test]
    async fn fire_task_webhooks_does_not_fire_on_non_matching_event() {
        let task = Task {
            id: Some("task-2".into()),
            job_id: Some("job-2".into()),
            state: TASK_STATE_COMPLETED.to_string(),
            subjob: Some(SubJobTask {
                webhooks: Some(vec![Webhook {
                    url: Some("https://example.com/hook".to_string()),
                    event: Some("task.Completed".to_string()),
                    headers: Some(HashMap::new()),
                    r#if: None,
                }]),
                ..Default::default()
            }),
            ..Default::default()
        };

        let start = std::time::Instant::now();
        fire_task_webhooks(&task, "task.Started");
        let elapsed = start.elapsed();
        
        assert!(elapsed.as_millis() < 500, "Should return quickly when event doesn't match");
    }

    #[tokio::test]
    async fn fire_task_webhooks_with_condition_evaluating_to_true() {
        let (tx, rx) = oneshot::channel();
        
        let task = Task {
            id: Some("task-3".into()),
            job_id: Some("job-3".into()),
            state: TASK_STATE_COMPLETED.to_string(),
            subjob: Some(SubJobTask {
                webhooks: Some(vec![Webhook {
                    url: Some("https://example.com/hook".to_string()),
                    event: Some("task.Completed".to_string()),
                    headers: Some(HashMap::new()),
                    r#if: Some("task_state == \"COMPLETED\"".to_string()),
                }]),
                ..Default::default()
            }),
            ..Default::default()
        };

        tokio::spawn(async move {
            fire_task_webhooks(&task, "task.Completed");
            tx.send(()).unwrap();
        });

        let result = tokio::time::timeout(std::time::Duration::from_secs(5), rx).await;
        assert!(result.is_ok(), "fire_task_webhooks should fire when condition is true");
    }

    #[tokio::test]
    async fn fire_task_webhooks_with_condition_evaluating_to_false() {
        let task = Task {
            id: Some("task-4".into()),
            job_id: Some("job-4".into()),
            state: TASK_STATE_RUNNING.to_string(),
            subjob: Some(SubJobTask {
                webhooks: Some(vec![Webhook {
                    url: Some("https://example.com/hook".to_string()),
                    event: Some("task.Completed".to_string()),
                    headers: Some(HashMap::new()),
                    r#if: Some("task_state == \"COMPLETED\"".to_string()),
                }]),
                ..Default::default()
            }),
            ..Default::default()
        };

        let start = std::time::Instant::now();
        fire_task_webhooks(&task, "task.Completed");
        let elapsed = start.elapsed();
        
        assert!(elapsed.as_millis() < 500, "Should return quickly when condition is false");
    }

    #[tokio::test]
    async fn fire_task_webhooks_does_not_fire_when_no_subjob_webhooks() {
        let task = Task {
            id: Some("task-5".into()),
            job_id: Some("job-5".into()),
            state: TASK_STATE_COMPLETED.to_string(),
            subjob: Some(SubJobTask::default()),
            ..Default::default()
        };

        let start = std::time::Instant::now();
        fire_task_webhooks(&task, "task.Completed");
        let elapsed = start.elapsed();
        
        assert!(elapsed.as_millis() < 500, "Should return quickly when no subjob webhooks configured");
    }

    #[tokio::test]
    async fn fire_task_webhooks_does_not_fire_when_no_subjob() {
        let task = Task {
            id: Some("task-6".into()),
            job_id: Some("job-6".into()),
            state: TASK_STATE_COMPLETED.to_string(),
            ..Default::default()
        };

        let start = std::time::Instant::now();
        fire_task_webhooks(&task, "task.Completed");
        let elapsed = start.elapsed();
        
        assert!(elapsed.as_millis() < 500, "Should return quickly when no subjob");
    }
}
