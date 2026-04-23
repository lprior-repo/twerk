//! Webhook-domain validators.
//!
//! Validates webhook URLs at both the top-level and inside subjobs.
//! Contains `check_subjob_webhooks` to eliminate bidirectional coupling
//! — it validates webhook URLs, so it belongs in the webhook domain.

use super::{fault_messages, ValidationFault, ValidationKind};
use crate::task::Task;
use crate::webhook::Webhook;

/// Check that every webhook in a slice has a non-empty URL.
pub(super) fn check_webhook_urls(webhooks: &[Webhook]) -> Vec<ValidationFault> {
    webhooks
        .iter()
        .filter(|w| w.url.as_ref().is_none_or(|u| u.trim().is_empty()))
        .map(|_| ValidationFault {
            kind: ValidationKind::WebhookUrl,
            message: "webhook URL cannot be empty".into(),
        })
        .collect()
}

/// Validate webhook URLs inside a subjob.
///
/// This is `pub(super)` so that the task submodule can call it
/// from `collect_task_faults`.
pub(super) fn check_subjob_webhooks(task: &Task) -> Vec<ValidationFault> {
    task.subjob
        .as_ref()
        .and_then(|sj| sj.webhooks.as_ref())
        .map(|whs| check_webhook_urls(whs))
        .map_or_else(Vec::new, std::convert::identity)
}

/// Validates webhook configurations.
///
/// # Errors
/// Returns a list of validation errors if any webhook URLs are empty.
pub fn validate_webhooks(
    webhooks: Option<&Vec<Webhook>>,
    tasks: Option<&Vec<Task>>,
) -> Result<(), Vec<String>> {
    let faults = collect_webhook_faults(webhooks, tasks);
    if faults.is_empty() {
        Ok(())
    } else {
        Err(fault_messages(&faults))
    }
}

/// Collect webhook faults from top-level webhooks and subjob webhooks in tasks.
fn collect_webhook_faults(
    webhooks: Option<&Vec<Webhook>>,
    tasks: Option<&Vec<Task>>,
) -> Vec<ValidationFault> {
    let top: Vec<ValidationFault> = webhooks.map_or_else(Vec::new, |w| check_webhook_urls(w));
    let sub: Vec<ValidationFault> = tasks.map_or_else(Vec::new, |ts| {
        ts.iter().flat_map(check_subjob_webhooks).collect()
    });
    top.into_iter().chain(sub).collect()
}
