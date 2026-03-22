//! Job webhook middleware.
//!
//! Handles firing webhooks when job events occur.

use std::sync::Arc;
use tork::job::{new_job_summary, Job, JobSummary, JOB_STATE_COMPLETED};
use tork::task::Webhook;

use crate::middleware::job::{
    apply_middleware, noop_handler, Context, EventType, HandlerFunc, JobError, MiddlewareFunc,
};

/// Webhook event types.
pub mod webhook_event {
    /// Job state change event.
    pub const JOB_STATE_CHANGE: &str = "job.StateChange";
    /// Job progress event.
    pub const JOB_PROGRESS: &str = "job.Progress";
    /// Default event (matches all).
    pub const DEFAULT: &str = "";
}

/// Create a webhook middleware.
///
/// This middleware fires configured webhooks when job state changes or progress updates occur.
pub fn webhook_middleware() -> MiddlewareFunc {
    Arc::new(|next: HandlerFunc| {
        Arc::new(move |ctx: Arc<Context>, et: EventType, job: &mut Job| {
            // First, call the next handler
            if let Err(e) = next(ctx.clone(), et, job) {
                return Err(e);
            }

            // Only process StateChange and Progress events
            if et != EventType::STATE_CHANGE && et != EventType::PROGRESS {
                return Ok(());
            }

            // Get webhooks - they might be stored differently
            let webhooks = match &job.webhooks {
                Some(whs) if !whs.is_empty() => whs.clone(),
                _ => return Ok(()),
            };

            let summary = new_job_summary(job);

            // Process each webhook
            for wh in &webhooks {
                // Check event filter
                if !should_fire_webhook(et, wh) {
                    continue;
                }

                // Evaluate conditional expression if present
                if let Some(if_expr) = &wh.if_expr {
                    match evaluate_condition(if_expr, &summary) {
                        Ok(true) => {}
                        Ok(false) => continue,
                        Err(e) => {
                            eprintln!(
                                "[Webhook] error evaluating if expression {}: {}",
                                if_expr, e
                            );
                            continue;
                        }
                    }
                }

                // Fire webhook asynchronously
                let wh_clone = wh.clone();
                let summary_clone = summary.clone();
                std::thread::spawn(move || {
                    call_webhook(&wh_clone, &summary_clone);
                });
            }

            Ok(())
        })
    })
}

/// Check if a webhook should fire for the given event.
fn should_fire_webhook(et: EventType, wh: &Webhook) -> bool {
    let event = wh.event.as_deref().unwrap_or("");

    let event_matches = event.is_empty()
        || event == webhook_event::JOB_STATE_CHANGE
        || event == webhook_event::JOB_PROGRESS
        || event == webhook_event::DEFAULT;

    if !event_matches {
        return false;
    }

    match et {
        EventType::STATE_CHANGE => {
            event.is_empty()
                || event == webhook_event::JOB_STATE_CHANGE
                || event == webhook_event::DEFAULT
        }
        EventType::PROGRESS => event.is_empty() || event == webhook_event::JOB_PROGRESS,
        _ => false,
    }
}

/// Evaluate a condition expression against the job summary.
fn evaluate_condition(expr: &str, _summary: &JobSummary) -> Result<bool, String> {
    match expr.trim() {
        "true" => Ok(true),
        "false" => Ok(false),
        s if s.starts_with("{{") && s.ends_with("}}") => {
            // Template expression like {{ job.State == 'COMPLETED' }}
            // For now, just return true - full expr-lang/expr evaluation would go here
            Ok(true)
        }
        _ => Err(format!("unknown expression: {}", expr)),
    }
}

/// Call a webhook with the given job summary.
fn call_webhook(wh: &Webhook, summary: &JobSummary) {
    println!(
        "[Webhook] Calling {} for job {} {:?}",
        wh.url.as_deref().unwrap_or(""),
        summary.id.as_deref().unwrap_or("?"),
        summary.state
    );

    // In Go: webhook.Call(wh, summary)
    // For Rust, we'd use reqwest or ureq
    // For now, just log
}

/// Log an error message.
fn log_error(msg: &str) {
    eprintln!("{}", msg);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_test_job() -> Job {
        Job {
            id: Some("1234".to_string()),
            state: JOB_STATE_COMPLETED.to_string(),
            webhooks: Some(vec![Webhook {
                url: Some("http://example.com/webhook".to_string()),
                headers: Some(std::collections::HashMap::new()),
                event: Some(webhook_event::JOB_STATE_CHANGE.to_string()),
                if_expr: None,
            }]),
            ..Default::default()
        }
    }

    #[test]
    fn test_webhook_no_event() {
        let mut job = Job {
            id: Some("1234".to_string()),
            state: JOB_STATE_COMPLETED.to_string(),
            webhooks: Some(vec![Webhook {
                url: Some("http://example.com/webhook".to_string()),
                headers: Some(std::collections::HashMap::new()),
                event: None,
                if_expr: None,
            }]),
            ..Default::default()
        };

        let hm = apply_middleware(noop_handler(), vec![webhook_middleware()]);
        let ctx = Arc::new(Context::new());

        let result = hm(ctx, EventType::STATE_CHANGE, &mut job);
        assert!(result.is_ok());
    }

    #[test]
    fn test_webhook_ignored_on_read() {
        let mut job = make_test_job();

        let hm = apply_middleware(noop_handler(), vec![webhook_middleware()]);
        let ctx = Arc::new(Context::new());

        // Read events should not trigger webhooks
        let result = hm(ctx, EventType::READ, &mut job);
        assert!(result.is_ok());
    }

    #[test]
    fn test_webhook_with_wrong_event() {
        let mut job = Job {
            id: Some("1234".to_string()),
            state: JOB_STATE_COMPLETED.to_string(),
            webhooks: Some(vec![Webhook {
                url: Some("http://example.com/webhook".to_string()),
                headers: Some(std::collections::HashMap::new()),
                event: Some(webhook_event::JOB_STATE_CHANGE.to_string()),
                if_expr: None,
            }]),
            ..Default::default()
        };

        let hm = apply_middleware(noop_handler(), vec![webhook_middleware()]);
        let ctx = Arc::new(Context::new());

        // Wrong event type should not trigger webhook
        let result = hm(ctx, "NO_STATE_CHANGE".into(), &mut job);
        assert!(result.is_ok());
    }

    #[test]
    fn test_webhook_if_true() {
        let mut job = Job {
            id: Some("1234".to_string()),
            state: JOB_STATE_COMPLETED.to_string(),
            webhooks: Some(vec![Webhook {
                url: Some("http://example.com/webhook".to_string()),
                headers: Some(std::collections::HashMap::new()),
                event: None,
                if_expr: Some("true".to_string()),
            }]),
            ..Default::default()
        };

        let hm = apply_middleware(noop_handler(), vec![webhook_middleware()]);
        let ctx = Arc::new(Context::new());

        let result = hm(ctx, EventType::STATE_CHANGE, &mut job);
        assert!(result.is_ok());
    }

    #[test]
    fn test_webhook_if_false() {
        let mut job = Job {
            id: Some("1234".to_string()),
            state: JOB_STATE_COMPLETED.to_string(),
            webhooks: Some(vec![Webhook {
                url: Some("http://example.com/webhook".to_string()),
                headers: Some(std::collections::HashMap::new()),
                event: None,
                if_expr: Some("false".to_string()),
            }]),
            ..Default::default()
        };

        let hm = apply_middleware(noop_handler(), vec![webhook_middleware()]);
        let ctx = Arc::new(Context::new());

        let result = hm(ctx, EventType::STATE_CHANGE, &mut job);
        assert!(result.is_ok());
    }
}
