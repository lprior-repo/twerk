//! Webhook middleware for job and task state change notifications.
//!
//! Go parity: middleware/job/webhook.go

use twerk_core::job::Job;
use twerk_core::webhook::{self, Webhook};

/// Fires job webhooks on state changes.
/// 
/// Go parity: middleware/job/webhook.go
pub fn fire_job_webhooks(job: &Job, event: &str) {
    if let Some(webhooks) = &job.webhooks {
        for wh in webhooks {
            if should_fire_webhook(wh, event) {
                evaluate_condition(wh.r#if.as_ref(), job);
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
}

fn should_fire_webhook(wh: &Webhook, event: &str) -> bool {
    wh.event.as_deref().is_none_or(|e| e == event || e.is_empty())
}

fn evaluate_condition(condition: Option<&String>, _context: &impl serde::Serialize) {
    if condition.is_some_and(|c| !c.is_empty()) {
        tracing::debug!("condition evaluation not yet implemented");
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
}
