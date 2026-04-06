//! Validation and parsing for domain types.
//!
//! This module follows the **Parse, Don't Validate** principle: every `parse_*`
//! function returns a *validated* newtype from [`domain_types`](crate::domain_types),
//! so callers receive a value that is correct by construction.
//!
//! The legacy `validate_*` functions are retained for backwards compatibility;
//! they simply delegate to the new parsers and discard the typed return value.

use crate::job::JobDefaults;
use crate::mount::Mount;
use crate::task::Task;
use crate::webhook::Webhook;
use std::time::Duration as StdDuration;

pub use crate::domain_types::{
    CronExpression, DomainParseError, GoDuration, Priority, QueueName, RetryLimit,
};

// ---------------------------------------------------------------------------
// Typed error accumulation
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
fn fault_messages(faults: &[ValidationFault]) -> Vec<String> {
    faults.iter().map(|f| f.message.clone()).collect()
}

/// Collect validation results: push a fault if `Err`.
fn push_fault(
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

// ===================================================================
// Parse-Don't-Validate: new parser functions returning validated types
// ===================================================================

/// Parse a cron expression into a validated [`CronExpression`].
///
/// # Errors
/// Returns [`DomainParseError::Cron`] on invalid syntax.
pub fn parse_cron(cron: &str) -> Result<CronExpression, DomainParseError> {
    CronExpression::new(cron).map_err(DomainParseError::Cron)
}

/// Parse a Go-style duration into a validated [`GoDuration`].
///
/// # Errors
/// Returns [`DomainParseError::Duration`] on invalid syntax.
pub fn parse_duration(duration: &str) -> Result<GoDuration, DomainParseError> {
    GoDuration::new(duration).map_err(DomainParseError::Duration)
}

/// Parse a queue name into a validated [`QueueName`].
///
/// # Errors
/// Returns [`DomainParseError::QueueName`] if the name is reserved or malformed.
pub fn parse_queue_name(name: &str) -> Result<QueueName, DomainParseError> {
    // Check for reserved names
    if name == "x-jobs" || name.starts_with("x-exclusive.") {
        return Err(DomainParseError::QueueName(
            crate::domain_types::QueueNameError::Reserved(name.to_string()),
        ));
    }
    QueueName::new(name).map_err(DomainParseError::QueueName)
}

/// Parse a retry limit into a validated [`RetryLimit`].
///
/// # Errors
/// Returns [`DomainParseError::RetryLimit`] if not in 1..=10.
pub fn parse_retry(limit: i64) -> Result<RetryLimit, DomainParseError> {
    RetryLimit::new(limit).map_err(DomainParseError::RetryLimit)
}

/// Parse a priority value into a validated [`Priority`].
///
/// # Errors
/// Returns [`DomainParseError::Priority`] if not in 0..=9.
pub fn parse_priority(priority: i64) -> Result<Priority, DomainParseError> {
    Priority::new(priority).map_err(DomainParseError::Priority)
}

// ===================================================================
// Backwards-compatible public API (validate_* returning Result<(), _>)
// ===================================================================

/// Validates a cron expression.
///
/// # Errors
/// Returns an error if the cron expression is invalid.
pub fn validate_cron(cron: &str) -> Result<(), String> {
    parse_cron(cron).map(|_| ()).map_err(|e| e.to_string())
}

/// Validates a duration string (e.g., "1h30m", "30s", "2d").
///
/// # Errors
/// Returns an error if the duration string is invalid.
pub fn validate_duration(duration: &str) -> Result<(), String> {
    parse_duration(duration)
        .map(|_| ())
        .map_err(|e| e.to_string())
}

/// Validates a queue name.
///
/// # Errors
/// Returns an error if the queue name starts with "x-exclusive." or is "x-jobs".
pub fn validate_queue_name(queue: &str) -> Result<(), String> {
    parse_queue_name(queue)
        .map(|_| ())
        .map_err(|e| e.to_string())
}

/// Validates a retry limit.
///
/// # Errors
/// Returns an error if the retry limit is not between 1 and 10.
pub fn validate_retry(limit: i64) -> Result<(), String> {
    parse_retry(limit).map(|_| ()).map_err(|e| e.to_string())
}

/// Validates a priority value.
///
/// # Errors
/// Returns an error if the priority is not between 0 and 9.
pub fn validate_priority(priority: i64) -> Result<(), String> {
    parse_priority(priority)
        .map(|_| ())
        .map_err(|e| e.to_string())
}

// ===================================================================
// Go-duration parser (pure, zero mut)
// ===================================================================

/// Parse a Go-style duration string into `std::time::Duration`.
///
/// Supports: `s` (seconds), `m` (minutes), `h` (hours), `d` (days).
///
/// # Errors
/// Returns a descriptive `String` on empty input, invalid characters, or overflow.
pub fn parse_go_duration(s: &str) -> Result<StdDuration, String> {
    let trimmed = s.trim();
    if trimmed.is_empty() {
        return Err("empty duration".into());
    }
    let (total_secs, trailing) = accumulate_duration_units(trimmed)?;
    let total_secs = total_secs + trailing;
    to_unsigned_duration(total_secs)
}

/// Accumulate named time units (`s`/`m`/`h`/`d`) and return total seconds
/// plus any trailing (unlabelled) numeric remainder.
fn accumulate_duration_units(s: &str) -> Result<(i64, i64), String> {
    s.chars().try_fold((0i64, 0i64), |(total, num), c| match c {
        '0'..='9' => Ok((total, num * 10 + i64::from(c as u32 - '0' as u32))),
        's' => Ok((total + num, 0)),
        'm' => Ok((total + num * 60, 0)),
        'h' => Ok((total + num * 3600, 0)),
        'd' => Ok((total + num * 86400, 0)),
        _ => Err(format!("invalid duration character: {c}")),
    })
}

/// Convert signed seconds into an unsigned `Duration`, rejecting negatives.
fn to_unsigned_duration(total_secs: i64) -> Result<StdDuration, String> {
    usize::try_from(total_secs)
        .map(|s| StdDuration::from_secs(s as u64))
        .map_err(|_| "duration overflow".into())
}

// ===================================================================
// Job validation helpers (each < 25 lines)
// ===================================================================

/// Check that a job name is present, non-empty, and not too long.
fn check_job_name(name: Option<&String>) -> Vec<ValidationFault> {
    let mut faults = Vec::new();
    match name {
        None => {
            faults.push(ValidationFault {
                kind: ValidationKind::JobName,
                message: "job name is required".into(),
            });
        }
        Some(s) if s.trim().is_empty() => {
            faults.push(ValidationFault {
                kind: ValidationKind::JobName,
                message: "job name is required".into(),
            });
        }
        Some(s) if s.len() > 256 => {
            faults.push(ValidationFault {
                kind: ValidationKind::JobName,
                message: format!("job name exceeds 256 characters (got {})", s.len()),
            });
        }
        _ => {}
    }
    faults
}

/// Check that tasks are present and non-empty, collecting per-task name faults.
fn check_job_tasks(tasks: Option<&Vec<Task>>) -> Vec<ValidationFault> {
    let empty_fault = tasks
        .as_ref()
        .is_none_or(|t| t.is_empty())
        .then(|| ValidationFault {
            kind: ValidationKind::TaskName,
            message: "at least one task is required".into(),
        });
    let task_name_faults = tasks
        .map(|ts| {
            ts.iter()
                .enumerate()
                .filter(|(_, t)| t.name.as_ref().is_none_or(|n| n.trim().is_empty()))
                .map(|(i, _)| ValidationFault {
                    kind: ValidationKind::TaskName,
                    message: format!("task at index {i} has no name"),
                })
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    empty_fault.into_iter().chain(task_name_faults).collect()
}

/// Validate job defaults (timeout, queue, priority).
fn check_job_defaults(defaults: &JobDefaults) -> Vec<ValidationFault> {
    let mut faults = Vec::new();
    if let Some(timeout) = &defaults.timeout {
        push_fault(
            &mut faults,
            ValidationKind::Duration,
            &format!("invalid default timeout: {timeout}"),
            parse_duration(timeout).map(|_| ()),
        );
    }
    if let Some(queue) = &defaults.queue {
        push_fault(
            &mut faults,
            ValidationKind::QueueName,
            &format!("invalid default queue: {queue}"),
            parse_queue_name(queue).map(|_| ()),
        );
    }
    push_fault(
        &mut faults,
        ValidationKind::Priority,
        &format!("invalid default priority: {}", defaults.priority),
        parse_priority(defaults.priority).map(|_| ()),
    );
    faults
}

/// Validates a job configuration.
///
/// # Errors
/// Returns a list of validation errors if any fields are invalid.
pub fn validate_job(
    name: Option<&String>,
    tasks: Option<&Vec<Task>>,
    defaults: Option<&JobDefaults>,
    _output: Option<&String>,
) -> Result<(), Vec<String>> {
    let faults = collect_job_faults(name, tasks, defaults);
    if faults.is_empty() {
        Ok(())
    } else {
        Err(fault_messages(&faults))
    }
}

/// Collect all faults for a job configuration.
fn collect_job_faults(
    name: Option<&String>,
    tasks: Option<&Vec<Task>>,
    defaults: Option<&JobDefaults>,
) -> Vec<ValidationFault> {
    check_job_name(name)
        .into_iter()
        .chain(check_job_tasks(tasks))
        .chain(defaults.map(check_job_defaults).unwrap_or_default())
        .collect()
}

// ===================================================================
// Task validation helpers (each < 25 lines)
// ===================================================================

/// Validate a task's timeout, queue, retry, and priority fields.
fn check_task_fields(task: &Task) -> Vec<ValidationFault> {
    let mut faults = Vec::new();
    if let Some(timeout) = &task.timeout {
        push_fault(
            &mut faults,
            ValidationKind::Duration,
            &format!("invalid timeout: {timeout}"),
            parse_duration(timeout).map(|_| ()),
        );
    }
    if let Some(queue) = &task.queue {
        push_fault(
            &mut faults,
            ValidationKind::QueueName,
            &format!("invalid queue: {queue}"),
            parse_queue_name(queue).map(|_| ()),
        );
    }
    if let Some(retry) = &task.retry {
        push_fault(
            &mut faults,
            ValidationKind::RetryLimit,
            &format!("invalid retry limit: {}", retry.limit),
            parse_retry(retry.limit).map(|_| ()),
        );
    }
    push_fault(
        &mut faults,
        ValidationKind::Priority,
        &format!("invalid priority: {}", task.priority),
        parse_priority(task.priority).map(|_| ()),
    );
    faults
}

/// Validate parallel/each/subjob structural constraints.
fn check_task_structure(task: &Task) -> Vec<ValidationFault> {
    let mut faults = Vec::new();
    if let Some(p) = &task.parallel {
        if p.tasks.as_ref().is_none_or(Vec::is_empty) {
            faults.push(ValidationFault {
                kind: ValidationKind::Parallel,
                message: "parallel tasks cannot be empty".into(),
            });
        }
    }
    if let Some(e) = &task.each {
        if e.list.as_ref().is_none_or(String::is_empty) {
            faults.push(ValidationFault {
                kind: ValidationKind::Each,
                message: "each list cannot be empty".into(),
            });
        }
    }
    if task.parallel.is_some() && task.each.is_some() {
        faults.push(ValidationFault {
            kind: ValidationKind::TaskField,
            message: "task cannot have both parallel and each".into(),
        });
    }
    if task.parallel.is_some() && task.subjob.is_some() {
        faults.push(ValidationFault {
            kind: ValidationKind::TaskField,
            message: "task cannot have both parallel and subjob".into(),
        });
    }
    faults
}

/// Validate `var` name length and `each` expression validity.
fn check_task_expressions(task: &Task) -> Vec<ValidationFault> {
    let mut faults = Vec::new();
    if let Some(var) = &task.var {
        if var.len() > 64 {
            faults.push(ValidationFault {
                kind: ValidationKind::Var,
                message: format!("variable name exceeds 64 characters: {}", var.len()),
            });
        }
    }
    if let Some(each) = &task.each {
        if let Some(list) = &each.list {
            if !list.is_empty() && !crate::eval::valid_expr(list) {
                faults.push(ValidationFault {
                    kind: ValidationKind::Expression,
                    message: format!("invalid expression: {list}"),
                });
            }
        }
    }
    faults
}

/// Validate webhook URLs inside a subjob.
fn check_subjob_webhooks(task: &Task) -> Vec<ValidationFault> {
    task.subjob
        .as_ref()
        .and_then(|sj| sj.webhooks.as_ref())
        .map(|whs| check_webhook_urls(whs))
        .unwrap_or_default()
}

/// Check that every webhook in a slice has a non-empty URL.
fn check_webhook_urls(webhooks: &[Webhook]) -> Vec<ValidationFault> {
    webhooks
        .iter()
        .filter(|w| w.url.as_ref().is_none_or(|u| u.trim().is_empty()))
        .map(|_| ValidationFault {
            kind: ValidationKind::WebhookUrl,
            message: "webhook URL cannot be empty".into(),
        })
        .collect()
}

/// Validates a task configuration.
///
/// # Errors
/// Returns a list of validation errors if any fields are invalid.
pub fn validate_task(task: &Task) -> Result<(), Vec<String>> {
    let faults = collect_task_faults(task);
    if faults.is_empty() {
        Ok(())
    } else {
        Err(fault_messages(&faults))
    }
}

/// Collect all faults for a task configuration.
fn collect_task_faults(task: &Task) -> Vec<ValidationFault> {
    check_task_fields(task)
        .into_iter()
        .chain(check_task_structure(task))
        .chain(check_task_expressions(task))
        .chain(check_subjob_webhooks(task))
        .collect()
}

// ===================================================================
// Webhook validation
// ===================================================================

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
    let top: Vec<ValidationFault> = webhooks
        .map(|ws| check_webhook_urls(ws))
        .unwrap_or_default();
    let sub: Vec<ValidationFault> = tasks
        .map(|ts| {
            ts.iter()
                .flat_map(check_subjob_webhooks)
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    top.into_iter().chain(sub).collect()
}

// ===================================================================
// Mount validation
// ===================================================================

/// Validate a single mount.
fn check_mount(mount: &Mount) -> Vec<ValidationFault> {
    let mut faults = Vec::new();
    if mount.mount_type.as_ref().is_some_and(String::is_empty) {
        faults.push(ValidationFault {
            kind: ValidationKind::MountType,
            message: "mount type is required".into(),
        });
    }
    faults.extend(check_mount_target(mount));
    faults.extend(check_bind_source(mount));
    faults
}

/// Validate a mount's target path.
fn check_mount_target(mount: &Mount) -> Vec<ValidationFault> {
    mount.target.as_ref().map_or_else(Vec::new, |target| {
        if target.is_empty() {
            vec![ValidationFault {
                kind: ValidationKind::MountTarget,
                message: "target is required".into(),
            }]
        } else if target.contains(':') {
            vec![ValidationFault {
                kind: ValidationKind::MountTarget,
                message: "invalid target path: cannot contain colon".into(),
            }]
        } else if target == "/tork" {
            vec![ValidationFault {
                kind: ValidationKind::MountTarget,
                message: "target path cannot be /tork".into(),
            }]
        } else {
            Vec::new()
        }
    })
}

/// Validate source path for bind mounts.
fn check_bind_source(mount: &Mount) -> Vec<ValidationFault> {
    if mount.mount_type.as_deref() != Some("bind") {
        return Vec::new();
    }
    match &mount.source {
        None => vec![ValidationFault {
            kind: ValidationKind::MountSource,
            message: "source is required for bind mount".into(),
        }],
        Some(src) if src.is_empty() => vec![ValidationFault {
            kind: ValidationKind::MountSource,
            message: "source is required for bind mount".into(),
        }],
        Some(src) if src.contains('#') => vec![ValidationFault {
            kind: ValidationKind::MountSource,
            message: "invalid source path: cannot contain hash".into(),
        }],
        _ => Vec::new(),
    }
}

/// Validates mount configurations.
///
/// # Errors
/// Returns a list of validation errors if any mounts are invalid.
pub fn validate_mounts(mounts: &Option<Vec<Mount>>) -> Result<(), Vec<String>> {
    let faults = mounts
        .as_ref()
        .map(|ms| ms.iter().flat_map(check_mount).collect::<Vec<_>>())
        .unwrap_or_default();
    if faults.is_empty() {
        Ok(())
    } else {
        Err(fault_messages(&faults))
    }
}
