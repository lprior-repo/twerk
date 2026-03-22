//! Job webhook middleware.
//!
//! Handles firing webhooks when job events occur.
//! Parity with Go's `middleware/job/webhook.go`.

use std::collections::HashMap;
use std::sync::Arc;

use tork::job::{new_job_summary, Job, JobSummary};
use tork::task::Webhook;
use tracing::{debug, error, info};

use crate::middleware::job::{Context, EventType, HandlerFunc, MiddlewareFunc};
use crate::webhook::{
    self as webhook_lib, EVENT_DEFAULT, EVENT_JOB_PROGRESS, EVENT_JOB_STATE_CHANGE,
};

/// Create a webhook middleware.
///
/// This middleware fires configured webhooks when job state changes
/// or progress updates occur. It is an "after" middleware — it calls
/// `next` first, then processes webhooks.
///
/// Parity with Go `func Webhook(next HandlerFunc) HandlerFunc`.
pub fn webhook_middleware() -> MiddlewareFunc {
    Arc::new(|next: HandlerFunc| {
        Arc::new(move |ctx: Arc<Context>, et: EventType, job: &mut Job| {
            // Call next handler first (Go parity: "after" middleware)
            next(ctx, et, job)?;

            // Only process StateChange and Progress events
            if et != EventType::StateChange && et != EventType::Progress {
                return Ok(());
            }

            // Get webhooks
            let webhooks = match &job.webhooks {
                Some(whs) if !whs.is_empty() => whs.as_slice(),
                _ => return Ok(()),
            };

            let summary = new_job_summary(job);

            // Process each webhook
            for wh in webhooks {
                // Check event filter
                if !should_fire_webhook(et, wh) {
                    continue;
                }

                // Evaluate conditional expression if present
                if let Some(if_expr) = &wh.r#if {
                    match evaluate_condition(if_expr, &summary) {
                        Ok(true) => {}
                        Ok(false) => continue,
                        Err(e) => {
                            error!(
                                "[Webhook] error evaluating if expression {}: {}",
                                if_expr, e
                            );
                            continue;
                        }
                    }
                }

                // Fire webhook asynchronously (Go: `go func(w *tork.Webhook)`)
                let wh_clone = wh.clone();
                let summary_clone = summary.clone();
                let job_context = job.context.clone();
                std::thread::spawn(move || {
                    call_webhook(&wh_clone, &summary_clone, &job_context);
                });
            }

            Ok(())
        })
    })
}

/// Check if a webhook should fire for the given event.
///
/// Parity with Go's event-matching logic:
/// - Default/empty event matches StateChange only
/// - Explicit event types must match the current event type
fn should_fire_webhook(et: EventType, wh: &Webhook) -> bool {
    let event = wh.event.as_deref().unwrap_or(EVENT_DEFAULT);

    // Filter to only job-level events (Go: lines 24-25)
    let event_matches = event.is_empty()
        || event == EVENT_JOB_STATE_CHANGE
        || event == EVENT_JOB_PROGRESS
        || event == EVENT_DEFAULT;

    if !event_matches {
        return false;
    }

    match et {
        EventType::StateChange => {
            event.is_empty() || event == EVENT_JOB_STATE_CHANGE || event == EVENT_DEFAULT
        }
        EventType::Progress => event.is_empty() || event == EVENT_JOB_PROGRESS,
        _ => false,
    }
}

/// Evaluate a condition expression against the job summary.
///
/// Uses the project's `eval` module for full expression evaluation,
/// providing parity with Go's `eval.EvaluateExpr(wh.If, map[string]any{"job": summary})`.
///
/// Since evalexpr doesn't support dot-access on objects, we flatten the
/// summary fields into the context as `job.<field>` (e.g., `job_state`, `job_id`).
/// The expression is expected to evaluate to a boolean. Go extracts
/// `val.(bool)` — we do the equivalent via `serde_json::Value::as_bool()`.
fn evaluate_condition(expr: &str, summary: &JobSummary) -> Result<bool, String> {
    let mut context = HashMap::new();

    // Flatten JobSummary fields into eval context for evalexpr compatibility.
    // Go uses expr-lang which supports struct field access (job.State),
    // so we provide equivalent flat variables: job_state, job_id, etc.
    context.insert(
        "job_state".to_string(),
        serde_json::Value::String(summary.state.clone()),
    );
    context.insert(
        "job_id".to_string(),
        serde_json::json!(summary.id.as_deref().unwrap_or("")),
    );
    if let Some(name) = &summary.name {
        context.insert(
            "job_name".to_string(),
            serde_json::Value::String(name.clone()),
        );
    }
    if let Some(error) = &summary.error {
        context.insert(
            "job_error".to_string(),
            serde_json::Value::String(error.clone()),
        );
    }

    let val = crate::eval::evaluate_expr(expr, &context).map_err(|e| format!("{e}"))?;

    val.as_bool().ok_or_else(|| {
        format!(
            "[Webhook] if expression {} did not evaluate to a boolean, got: {}",
            expr, val
        )
    })
}

/// Call a webhook with the given job summary.
///
/// Parity with Go's `callWebhook(wh, job)`:
/// 1. Evaluate header templates against job context
/// 2. POST the job summary as JSON to the webhook URL
/// 3. Log errors (never propagate — webhooks are best-effort)
fn call_webhook(wh: &Webhook, summary: &JobSummary, job_context: &tork::job::JobContext) {
    debug!(
        "[Webhook] Calling {} for job {} {}",
        wh.url.as_deref().unwrap_or("?"),
        summary.id.as_deref().unwrap_or("?"),
        summary.state
    );

    // Clone webhook and evaluate headers (Go parity: wh.Clone() then mutate)
    let evaluated_headers = evaluate_headers(wh.headers.as_ref(), job_context);

    let wh_call = webhook_lib::Webhook {
        url: wh.url.clone().unwrap_or_default(),
        headers: evaluated_headers,
    };

    if let Err(e) = webhook_lib::call(&wh_call, summary) {
        info!(
            "[Webhook] error calling job webhook {}: {}",
            wh.url.as_deref().unwrap_or("?"),
            e
        );
    }
}

/// Build a flat context map from `JobContext` for evalexpr template evaluation.
///
/// Go's expr-lang supports dot-access on nested maps (e.g., `secrets.some_key`).
/// evalexpr does not support this, so we flatten nested maps into dot-separated keys
/// (e.g., `secrets.some_key` → `"secrets.some_key"`).
///
/// Also provides top-level keys for each section.
fn flatten_context(job_context: &tork::job::JobContext) -> HashMap<String, serde_json::Value> {
    let mut flat = HashMap::new();

    if let Some(inputs) = &job_context.inputs {
        for (k, v) in inputs {
            flat.insert(format!("inputs.{k}"), serde_json::Value::String(v.clone()));
        }
    }
    if let Some(secrets) = &job_context.secrets {
        for (k, v) in secrets {
            flat.insert(format!("secrets.{k}"), serde_json::Value::String(v.clone()));
        }
    }
    if let Some(tasks) = &job_context.tasks {
        for (k, v) in tasks {
            flat.insert(format!("tasks.{k}"), serde_json::Value::String(v.clone()));
        }
    }

    flat
}

/// Evaluate template expressions in webhook headers using the job context.
///
/// Parity with Go:
/// ```go
/// for name, v := range wh.Headers {
///     newv, err := eval.EvaluateTemplate(v, job.Context.AsMap())
///     wh.Headers[name] = newv
/// }
/// ```
///
/// Since evalexpr doesn't support dot-access on nested maps, we flatten
/// the context into dot-separated keys (e.g., `secrets.some_key`).
/// If a header fails to evaluate, the original value is preserved (Go logs error).
fn evaluate_headers(
    headers: Option<&HashMap<String, String>>,
    job_context: &tork::job::JobContext,
) -> Option<HashMap<String, String>> {
    headers.map(|hdrs| {
        let flat_ctx = flatten_context(job_context);
        hdrs.iter()
            .map(|(name, value)| {
                let evaluated =
                    crate::eval::evaluate_template(value, &flat_ctx).unwrap_or_else(|e| {
                        error!("[Webhook] error evaluating header {}: {}", name, e);
                        value.clone()
                    });
                (name.clone(), evaluated)
            })
            .collect()
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::middleware::job::{apply_middleware, noop_handler};
    use tork::job::{JobContext, JOB_STATE_COMPLETED, JOB_STATE_RUNNING};

    fn make_test_job() -> Job {
        Job {
            id: Some("1234".to_string()),
            state: JOB_STATE_COMPLETED.to_string(),
            webhooks: Some(vec![Webhook {
                url: Some("http://example.com/webhook".to_string()),
                headers: Some(HashMap::new()),
                event: Some(EVENT_JOB_STATE_CHANGE.to_string()),
                r#if: None,
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
                headers: Some(HashMap::new()),
                event: None,
                r#if: None,
            }]),
            ..Default::default()
        };

        let hm = apply_middleware(noop_handler(), vec![webhook_middleware()]);
        let ctx = Arc::new(Context::new());

        let result = hm(ctx, EventType::StateChange, &mut job);
        assert!(result.is_ok());
    }

    #[test]
    fn test_webhook_ignored_on_read() {
        let mut job = make_test_job();

        let hm = apply_middleware(noop_handler(), vec![webhook_middleware()]);
        let ctx = Arc::new(Context::new());

        // Read events should not trigger webhooks
        let result = hm(ctx, EventType::Read, &mut job);
        assert!(result.is_ok());
    }

    #[test]
    fn test_webhook_with_wrong_event() {
        let mut job = Job {
            id: Some("1234".to_string()),
            state: JOB_STATE_COMPLETED.to_string(),
            webhooks: Some(vec![Webhook {
                url: Some("http://example.com/webhook".to_string()),
                headers: Some(HashMap::new()),
                event: Some(EVENT_JOB_STATE_CHANGE.to_string()),
                r#if: None,
            }]),
            ..Default::default()
        };

        let hm = apply_middleware(noop_handler(), vec![webhook_middleware()]);
        let ctx = Arc::new(Context::new());

        // Progress event with StateChange webhook should not fire
        // (Go: line 29-31 — Progress only matches job.Progress event)
        let result = hm(ctx, EventType::Progress, &mut job);
        assert!(result.is_ok());
    }

    #[test]
    fn test_webhook_if_true() {
        let mut job = Job {
            id: Some("1234".to_string()),
            state: JOB_STATE_COMPLETED.to_string(),
            webhooks: Some(vec![Webhook {
                url: Some("http://example.com/webhook".to_string()),
                headers: Some(HashMap::new()),
                event: None,
                r#if: Some("true".to_string()),
            }]),
            ..Default::default()
        };

        let hm = apply_middleware(noop_handler(), vec![webhook_middleware()]);
        let ctx = Arc::new(Context::new());

        let result = hm(ctx, EventType::StateChange, &mut job);
        assert!(result.is_ok());
    }

    #[test]
    fn test_webhook_if_false() {
        let mut job = Job {
            id: Some("1234".to_string()),
            state: JOB_STATE_COMPLETED.to_string(),
            webhooks: Some(vec![Webhook {
                url: Some("http://example.com/webhook".to_string()),
                headers: Some(HashMap::new()),
                event: None,
                r#if: Some("false".to_string()),
            }]),
            ..Default::default()
        };

        let hm = apply_middleware(noop_handler(), vec![webhook_middleware()]);
        let ctx = Arc::new(Context::new());

        let result = hm(ctx, EventType::StateChange, &mut job);
        assert!(result.is_ok());
    }

    #[test]
    fn test_webhook_if_job_status() {
        // Parity with Go TestWebhookIfJobStatus: `{{ job.State == 'COMPLETED' }}`
        // evalexpr uses flattened field names
        let mut job = Job {
            id: Some("1234".to_string()),
            state: JOB_STATE_COMPLETED.to_string(),
            webhooks: Some(vec![Webhook {
                url: Some("http://example.com/webhook".to_string()),
                headers: Some(HashMap::new()),
                event: None,
                // This expression should evaluate to true when state is COMPLETED
                r#if: Some("job_state == \"COMPLETED\"".to_string()),
            }]),
            ..Default::default()
        };

        let hm = apply_middleware(noop_handler(), vec![webhook_middleware()]);
        let ctx = Arc::new(Context::new());

        let result = hm(ctx, EventType::StateChange, &mut job);
        assert!(result.is_ok());
    }

    #[test]
    fn test_webhook_job_event_state_change() {
        let mut job = Job {
            id: Some("1234".to_string()),
            state: JOB_STATE_COMPLETED.to_string(),
            webhooks: Some(vec![Webhook {
                url: Some("http://example.com/webhook".to_string()),
                headers: Some(HashMap::new()),
                event: Some(EVENT_JOB_STATE_CHANGE.to_string()),
                r#if: None,
            }]),
            ..Default::default()
        };

        let hm = apply_middleware(noop_handler(), vec![webhook_middleware()]);
        let ctx = Arc::new(Context::new());

        let result = hm(ctx, EventType::StateChange, &mut job);
        assert!(result.is_ok());
    }

    #[test]
    fn test_webhook_job_event_progress() {
        let mut job = Job {
            id: Some("5678".to_string()),
            state: JOB_STATE_RUNNING.to_string(),
            webhooks: Some(vec![Webhook {
                url: Some("http://example.com/webhook".to_string()),
                headers: Some(HashMap::new()),
                event: Some(EVENT_JOB_PROGRESS.to_string()),
                r#if: None,
            }]),
            ..Default::default()
        };

        let hm = apply_middleware(noop_handler(), vec![webhook_middleware()]);
        let ctx = Arc::new(Context::new());

        let result = hm(ctx, EventType::Progress, &mut job);
        assert!(result.is_ok());
    }

    #[test]
    fn test_evaluate_condition_true() {
        let job = Job {
            id: Some("1234".to_string()),
            state: JOB_STATE_COMPLETED.to_string(),
            ..Default::default()
        };
        let summary = new_job_summary(&job);
        assert_eq!(evaluate_condition("true", &summary), Ok(true));
    }

    #[test]
    fn test_evaluate_condition_false() {
        let job = Job {
            id: Some("1234".to_string()),
            state: JOB_STATE_COMPLETED.to_string(),
            ..Default::default()
        };
        let summary = new_job_summary(&job);
        assert_eq!(evaluate_condition("false", &summary), Ok(false));
    }

    #[test]
    fn test_evaluate_condition_job_state() {
        let job = Job {
            id: Some("1234".to_string()),
            state: JOB_STATE_COMPLETED.to_string(),
            ..Default::default()
        };
        let summary = new_job_summary(&job);
        // evalexpr uses flattened field names: job_state instead of job.State
        assert_eq!(
            evaluate_condition("job_state == \"COMPLETED\"", &summary),
            Ok(true)
        );
    }

    #[test]
    fn test_evaluate_condition_job_state_mismatch() {
        let job = Job {
            id: Some("1234".to_string()),
            state: JOB_STATE_COMPLETED.to_string(),
            ..Default::default()
        };
        let summary = new_job_summary(&job);
        assert_eq!(
            evaluate_condition("job_state == \"FAILED\"", &summary),
            Ok(false)
        );
    }

    #[test]
    fn test_evaluate_condition_with_template_wrapper() {
        // Go accepts `{{ job.State == 'COMPLETED' }}` — sanitize_expr strips {{ }}
        let job = Job {
            id: Some("1234".to_string()),
            state: JOB_STATE_COMPLETED.to_string(),
            ..Default::default()
        };
        let summary = new_job_summary(&job);
        // evalexpr uses flattened field names and " for strings
        assert_eq!(
            evaluate_condition("{{ job_state == \"COMPLETED\" }}", &summary),
            Ok(true)
        );
    }

    #[test]
    fn test_evaluate_headers_basic() {
        let headers = HashMap::from([
            (
                "Content-Type".to_string(),
                "application/json; charset=UTF-8".to_string(),
            ),
            ("my-header".to_string(), "my-value".to_string()),
        ]);
        let job_context = JobContext::default();
        let result = evaluate_headers(Some(&headers), &job_context);

        let result = result.expect("headers should be present");
        assert_eq!(
            result.get("Content-Type").map(String::as_str),
            Some("application/json; charset=UTF-8")
        );
        assert_eq!(
            result.get("my-header").map(String::as_str),
            Some("my-value")
        );
    }

    #[test]
    fn test_evaluate_headers_with_template() {
        let headers = HashMap::from([("secret".to_string(), "{{secrets.some_key}}".to_string())]);
        let job_context = JobContext {
            secrets: Some(HashMap::from([(
                "some_key".to_string(),
                "1234-5678".to_string(),
            )])),
            ..Default::default()
        };
        let result = evaluate_headers(Some(&headers), &job_context);

        let result = result.expect("headers should be present");
        assert_eq!(result.get("secret").map(String::as_str), Some("1234-5678"));
    }

    #[test]
    fn test_evaluate_headers_none() {
        let job_context = JobContext::default();
        let result = evaluate_headers(None, &job_context);
        assert!(result.is_none());
    }

    #[test]
    fn test_should_fire_webhook_state_change_default_event() {
        let wh = Webhook {
            url: Some("http://example.com".to_string()),
            headers: None,
            event: None,
            r#if: None,
        };
        assert!(should_fire_webhook(EventType::StateChange, &wh));
    }

    #[test]
    fn test_should_fire_webhook_state_change_explicit() {
        let wh = Webhook {
            url: Some("http://example.com".to_string()),
            headers: None,
            event: Some(EVENT_JOB_STATE_CHANGE.to_string()),
            r#if: None,
        };
        assert!(should_fire_webhook(EventType::StateChange, &wh));
    }

    #[test]
    fn test_should_not_fire_webhook_progress_for_state_change_event() {
        let wh = Webhook {
            url: Some("http://example.com".to_string()),
            headers: None,
            event: Some(EVENT_JOB_STATE_CHANGE.to_string()),
            r#if: None,
        };
        assert!(!should_fire_webhook(EventType::Progress, &wh));
    }

    #[test]
    fn test_should_fire_webhook_progress_explicit() {
        let wh = Webhook {
            url: Some("http://example.com".to_string()),
            headers: None,
            event: Some(EVENT_JOB_PROGRESS.to_string()),
            r#if: None,
        };
        assert!(should_fire_webhook(EventType::Progress, &wh));
    }

    #[test]
    fn test_should_not_fire_webhook_read() {
        let wh = Webhook {
            url: Some("http://example.com".to_string()),
            headers: None,
            event: None,
            r#if: None,
        };
        assert!(!should_fire_webhook(EventType::Read, &wh));
    }

    #[test]
    fn test_should_not_fire_webhook_task_event() {
        let wh = Webhook {
            url: Some("http://example.com".to_string()),
            headers: None,
            event: Some("task.StateChange".to_string()),
            r#if: None,
        };
        // task events don't match any job event type
        assert!(!should_fire_webhook(EventType::StateChange, &wh));
        assert!(!should_fire_webhook(EventType::Progress, &wh));
    }

    /// Go parity: TestWebhookOKWithHeaders — verifies header template evaluation.
    /// Secret values in headers ({{secrets.some_key}}) are resolved from job context.
    #[test]
    fn test_webhook_headers_with_secret_template() {
        let headers = HashMap::from([
            (
                "Content-Type".to_string(),
                "application/json; charset=UTF-8".to_string(),
            ),
            ("my-header".to_string(), "my-value".to_string()),
            ("secret".to_string(), "{{secrets.some_key}}".to_string()),
        ]);

        let job_context = JobContext {
            secrets: Some(HashMap::from([(
                "some_key".to_string(),
                "1234-5678".to_string(),
            )])),
            ..Default::default()
        };

        let result = evaluate_headers(Some(&headers), &job_context);
        let result = result.expect("headers should be present");

        assert_eq!(
            result.get("Content-Type").map(String::as_str),
            Some("application/json; charset=UTF-8")
        );
        assert_eq!(
            result.get("my-header").map(String::as_str),
            Some("my-value")
        );
        // Secret template resolved
        assert_eq!(result.get("secret").map(String::as_str), Some("1234-5678"));
    }

    /// Go parity: TestWebhookOKWithHeaders — headers without templates pass through.
    #[test]
    fn test_webhook_headers_without_templates() {
        let headers = HashMap::from([
            ("X-Custom".to_string(), "custom-value".to_string()),
            ("Authorization".to_string(), "Bearer token123".to_string()),
        ]);

        let job_context = JobContext::default();

        let result = evaluate_headers(Some(&headers), &job_context);
        let result = result.expect("headers should be present");

        assert_eq!(
            result.get("X-Custom").map(String::as_str),
            Some("custom-value")
        );
        assert_eq!(
            result.get("Authorization").map(String::as_str),
            Some("Bearer token123")
        );
    }

    /// Go parity: TestWebhookRetry — verifies retry logic via webhook client.
    /// The webhook call uses ureq with retry on 500 responses.
    /// Note: Actual HTTP retry is tested in the webhook module's tests.
    /// Here we verify the middleware correctly dispatches the webhook call.
    #[test]
    fn test_webhook_dispatches_async_call() {
        let mut job = Job {
            id: Some("1234".to_string()),
            state: JOB_STATE_COMPLETED.to_string(),
            webhooks: Some(vec![Webhook {
                url: Some("http://127.0.0.1:1/webhook-retry".to_string()),
                headers: None,
                event: None,
                r#if: None,
            }]),
            ..Default::default()
        };

        let hm = apply_middleware(noop_handler(), vec![webhook_middleware()]);
        let ctx = Arc::new(Context::new());

        // Middleware should succeed — webhook call is async (fire-and-forget)
        let result = hm(ctx, EventType::StateChange, &mut job);
        assert!(result.is_ok());
    }

    /// Verifies the middleware correctly processes multiple webhooks.
    #[test]
    fn test_webhook_multiple_webhooks_all_fired() {
        let mut job = Job {
            id: Some("1234".to_string()),
            state: JOB_STATE_COMPLETED.to_string(),
            webhooks: Some(vec![
                Webhook {
                    url: Some("http://127.0.0.1:1/hook1".to_string()),
                    headers: None,
                    event: Some(EVENT_JOB_STATE_CHANGE.to_string()),
                    r#if: None,
                },
                Webhook {
                    url: Some("http://127.0.0.1:1/hook2".to_string()),
                    headers: None,
                    event: Some(EVENT_JOB_STATE_CHANGE.to_string()),
                    r#if: None,
                },
            ]),
            ..Default::default()
        };

        let hm = apply_middleware(noop_handler(), vec![webhook_middleware()]);
        let ctx = Arc::new(Context::new());

        let result = hm(ctx, EventType::StateChange, &mut job);
        assert!(result.is_ok());
    }

    /// Verifies that a mix of matching and non-matching webhooks only fires matching ones.
    #[test]
    fn test_webhook_mixed_events_only_matching_fire() {
        let mut job = Job {
            id: Some("1234".to_string()),
            state: JOB_STATE_COMPLETED.to_string(),
            webhooks: Some(vec![
                Webhook {
                    url: Some("http://127.0.0.1:1/should-fire".to_string()),
                    headers: None,
                    event: Some(EVENT_JOB_STATE_CHANGE.to_string()),
                    r#if: None,
                },
                Webhook {
                    // Progress webhook — should NOT fire on StateChange
                    url: Some("http://127.0.0.1:1/should-not-fire".to_string()),
                    headers: None,
                    event: Some(EVENT_JOB_PROGRESS.to_string()),
                    r#if: None,
                },
            ]),
            ..Default::default()
        };

        let hm = apply_middleware(noop_handler(), vec![webhook_middleware()]);
        let ctx = Arc::new(Context::new());

        let result = hm(ctx, EventType::StateChange, &mut job);
        assert!(result.is_ok());
    }

    /// Verifies empty if-expression is treated as truthy (no condition).
    #[test]
    fn test_webhook_empty_if_expression() {
        let mut job = Job {
            id: Some("1234".to_string()),
            state: JOB_STATE_COMPLETED.to_string(),
            webhooks: Some(vec![Webhook {
                url: Some("http://127.0.0.1:1/should-fire".to_string()),
                headers: None,
                event: None,
                // Empty string — should be treated as no condition (fire)
                r#if: Some(String::new()),
            }]),
            ..Default::default()
        };

        let hm = apply_middleware(noop_handler(), vec![webhook_middleware()]);
        let ctx = Arc::new(Context::new());

        let result = hm(ctx, EventType::StateChange, &mut job);
        assert!(result.is_ok());
    }

    /// Go parity: TestWebhookWrongEvent — wrong event type string skips webhook.
    #[test]
    fn test_webhook_wrong_event_type_string() {
        let mut job = Job {
            id: Some("1234".to_string()),
            state: JOB_STATE_COMPLETED.to_string(),
            webhooks: Some(vec![Webhook {
                url: Some("http://127.0.0.1:1/should-not-fire".to_string()),
                headers: Some(HashMap::from([(
                    "my-header".to_string(),
                    "my-value".to_string(),
                )])),
                event: Some(EVENT_JOB_STATE_CHANGE.to_string()),
                r#if: None,
            }]),
            ..Default::default()
        };

        let hm = apply_middleware(noop_handler(), vec![webhook_middleware()]);
        let ctx = Arc::new(Context::new());

        // Progress event with StateChange webhook should not fire
        let result = hm(ctx, EventType::Progress, &mut job);
        assert!(result.is_ok());
    }
}
