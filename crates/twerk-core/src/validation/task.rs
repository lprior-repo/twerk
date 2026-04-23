//! Task-domain validators.
//!
//! Validates task fields (timeout, queue, retry, priority), structure
//! (parallel/each/subjob constraints), and expressions (var names, each lists).

use super::primitives::{parse_duration, parse_priority, parse_queue_name, parse_retry};
use super::webhook::check_subjob_webhooks;
use super::{fault_messages, push_fault, ValidationFault, ValidationKind};
use crate::task::Task;

/// Validate a task's timeout, queue, retry, and priority fields.
pub(super) fn check_task_fields(task: &Task) -> Vec<ValidationFault> {
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
pub(super) fn check_task_structure(task: &Task) -> Vec<ValidationFault> {
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
pub(super) fn check_task_expressions(task: &Task) -> Vec<ValidationFault> {
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
