//! Validation and parsing for domain types.
//!
//! This module follows the **Parse, Don't Validate** principle: every `parse_*`
//! function returns a *validated* newtype from [`domain`](crate::domain),
//! so callers receive a value that is correct by construction.
//!
//! The legacy `validate_*` functions are retained for backwards compatibility;
//! they simply delegate to the new parsers and discard the typed return value.

// --- internal submodules ---
pub mod go_duration;
pub mod job;
pub mod mount;
pub mod primitives;
pub mod task;
pub mod webhook;

// --- re-exports from domain (must match original lines 18-20) ---
pub use crate::domain::{
    CronExpression, DomainParseError, GoDuration, Priority, QueueName, QueueNameError,
};

// ---------------------------------------------------------------------------
// Typed error accumulation (shared across all validators)
// ---------------------------------------------------------------------------

/// A single validation failure, carrying a machine-readable kind and context.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValidationFault {
    pub kind: ValidationKind,
    pub message: String,
}

/// Categorises the kind of validation failure.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ValidationKind {
    Cron,
    Duration,
    QueueName,
    RetryLimit,
    Priority,
    JobName,
    TaskName,
    TaskField,
    WebhookUrl,
    MountType,
    MountTarget,
    MountSource,
    Parallel,
    Each,
    Var,
    Expression,
    Subjob,
}

impl std::fmt::Display for ValidationFault {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.message)
    }
}

/// Helper: convert a `Vec<ValidationFault>` into a `Vec<String>` for the
/// backwards-compatible API.
pub(super) fn fault_messages(faults: &[ValidationFault]) -> Vec<String> {
    faults.iter().map(|f| f.message.clone()).collect()
}

/// Collect validation results: push a fault if `Err`.
pub(super) fn push_fault(
    faults: &mut Vec<ValidationFault>,
    kind: ValidationKind,
    prefix: &str,
    result: Result<(), DomainParseError>,
) {
    if let Err(e) = result {
        faults.push(ValidationFault {
            kind,
            message: if prefix.is_empty() {
                e.to_string()
            } else {
                format!("{prefix}: {e}")
            },
        });
    }
}

// --- public API passthrough (unchanged surface) ---
pub use self::go_duration::parse_go_duration;
pub use self::job::validate_job;
pub use self::mount::validate_mounts;
pub use self::primitives::{
    parse_cron, parse_duration, parse_priority, parse_queue_name, parse_retry, validate_cron,
    validate_duration, validate_priority, validate_queue_name, validate_retry,
};
pub use self::task::validate_task;
pub use self::webhook::validate_webhooks;
