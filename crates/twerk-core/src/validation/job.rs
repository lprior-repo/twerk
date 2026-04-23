//! Job-domain validators.
//!
//! Validates job name, tasks, and defaults (timeout, queue, priority).

use super::primitives::{parse_duration, parse_priority, parse_queue_name};
use super::{fault_messages, push_fault, ValidationFault, ValidationKind};
use crate::job::JobDefaults;
use crate::task::Task;

/// Check that a job name is present, non-empty, and not too long.
pub(super) fn check_job_name(name: Option<&String>) -> Vec<ValidationFault> {
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
pub(super) fn check_job_tasks(tasks: Option<&Vec<Task>>) -> Vec<ValidationFault> {
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
pub(super) fn check_job_defaults(defaults: &JobDefaults) -> Vec<ValidationFault> {
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
