//! Validation module for Tork job and task inputs
//!
//! This module provides validation for job and task input structures.

use regex::Regex;
use std::sync::LazyLock;

use crate::duration::is_valid_duration;
use crate::job::{Job, JobDefaults, Permission, ScheduledJob, Wait, Webhook};
use crate::task::{AuxTask, Each, Mount, Parallel, Probe, SidecarTask, SubJob, Task};

/// Mount pattern for validation
static MOUNT_PATTERN: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^[-/\.0-9a-zA-Z_/= ]+$").expect("invalid regex"));

/// Error types for validation
#[derive(Debug, Clone, thiserror::Error)]
pub enum ValidationError {
    #[error("field '{field}' is required")]
    Required { field: String },

    #[error("field '{field}' has invalid value: {message}")]
    Invalid { field: String, message: String },

    #[error("field '{field}' exceeds maximum length: {max}")]
    MaxLength { field: String, max: usize },

    #[error("field '{field}' must be between {min} and {max}")]
    Range { field: String, min: i64, max: i64 },

    #[error("tasks are required (at least one)")]
    NoTasks,

    #[error("invalid duration format: {0}")]
    InvalidDuration(String),

    #[error("invalid cron expression: {0}")]
    InvalidCron(String),

    #[error("invalid queue name: {0}")]
    InvalidQueue(String),

    #[error("invalid expression: {0}")]
    InvalidExpr(String),

    #[error("mount validation failed: {0}")]
    InvalidMount(String),

    #[error("permission validation failed: {0}")]
    InvalidPermission(String),

    #[error("task type conflict: {0}")]
    TaskTypeConflict(String),

    #[error("probe validation failed: {0}")]
    InvalidProbe(String),

    #[error("wait validation failed: {0}")]
    InvalidWait(String),
}

/// Result type for validation
pub type ValidationResult<T> = Result<T, ValidationError>;

/// Validates a job input
pub fn validate_job(job: &Job) -> ValidationResult<()> {
    // Check required fields
    if job.name.as_ref().map_or(true, |n| n.trim().is_empty()) {
        return Err(ValidationError::Required {
            field: "name".to_string(),
        });
    }

    // Check tasks
    let tasks = job.tasks.as_ref().ok_or(ValidationError::NoTasks)?;
    if tasks.is_empty() {
        return Err(ValidationError::NoTasks);
    }

    // Validate each task
    for task in tasks {
        validate_task(task)?;
    }

    // Validate defaults if present
    if let Some(ref defaults) = job.defaults {
        validate_job_defaults(defaults)?;
    }

    // Validate webhooks if present
    if let Some(ref webhooks) = job.webhooks {
        for webhook in webhooks {
            validate_webhook(webhook)?;
        }
    }

    // Validate output expression
    if let Some(ref output) = job.output {
        if !output.is_empty() && !valid_expr(output) {
            return Err(ValidationError::InvalidExpr(output.clone()));
        }
    }

    // Validate wait configuration
    if let Some(ref wait) = job.wait {
        validate_wait(wait)?;
    }

    // Validate permissions if present
    if let Some(ref permissions) = job.permissions {
        for perm in permissions {
            validate_permission(perm)?;
        }
    }

    // Validate mounts in tasks
    for task in tasks {
        if let Some(ref mounts) = task.mounts {
            for mount in mounts {
                validate_mount(mount)?;
            }
        }
    }

    Ok(())
}

/// Validates a scheduled job input
pub fn validate_scheduled_job(job: &ScheduledJob) -> ValidationResult<()> {
    // Check required fields
    if job.name.as_ref().map_or(true, |n| n.trim().is_empty()) {
        return Err(ValidationError::Required {
            field: "name".to_string(),
        });
    }

    // Check tasks
    let tasks = job.tasks.as_ref().ok_or(ValidationError::NoTasks)?;
    if tasks.is_empty() {
        return Err(ValidationError::NoTasks);
    }

    // Validate schedule
    if let Some(ref schedule) = job.schedule {
        validate_cron(&schedule.cron)?;
    } else {
        return Err(ValidationError::Required {
            field: "schedule".to_string(),
        });
    }

    // Validate each task
    for task in tasks {
        validate_task(task)?;
    }

    // Validate defaults if present
    if let Some(ref defaults) = job.defaults {
        validate_job_defaults(defaults)?;
    }

    // Validate webhooks if present
    if let Some(ref webhooks) = job.webhooks {
        for webhook in webhooks {
            validate_webhook(webhook)?;
        }
    }

    // Validate permissions if present
    if let Some(ref permissions) = job.permissions {
        for perm in permissions {
            validate_permission(perm)?;
        }
    }

    Ok(())
}

/// Validates a task input
fn validate_task(task: &Task) -> ValidationResult<()> {
    // Check for name
    if task.name.as_ref().map_or(true, |n| n.trim().is_empty()) {
        return Err(ValidationError::Required {
            field: "name".to_string(),
        });
    }

    // Validate task type constraints
    validate_task_type_constraints(task)?;

    // Validate composite task rules
    if task.is_composite() {
        validate_composite_task(task)?;
    }

    // Validate timeout if present
    if let Some(ref timeout) = task.timeout {
        if !timeout.is_empty() && !is_valid_duration(timeout) {
            return Err(ValidationError::InvalidDuration(timeout.clone()));
        }
    }

    // Validate retry if present
    if let Some(ref retry) = task.retry {
        if !(1..=10).contains(&retry.limit) {
            return Err(ValidationError::Range {
                field: "retry.limit".to_string(),
                min: 1,
                max: 10,
            });
        }
    }

    // Validate var length
    if let Some(ref var) = task.var {
        if var.len() > 64 {
            return Err(ValidationError::MaxLength {
                field: "var".to_string(),
                max: 64,
            });
        }
    }

    // Validate workdir length
    if let Some(ref workdir) = task.workdir {
        if workdir.len() > 256 {
            return Err(ValidationError::MaxLength {
                field: "workdir".to_string(),
                max: 256,
            });
        }
    }

    // Validate priority range
    if !(0..=9).contains(&task.priority) {
        return Err(ValidationError::Range {
            field: "priority".to_string(),
            min: 0,
            max: 9,
        });
    }

    // Validate queue if present
    if let Some(ref queue) = task.queue {
        if !queue.is_empty() && !is_valid_queue(queue) {
            return Err(ValidationError::InvalidQueue(queue.clone()));
        }
    }

    // Validate if expression
    if let Some(ref r#if) = task.r#if {
        if !r#if.is_empty() && !valid_expr(r#if) {
            return Err(ValidationError::InvalidExpr(r#if.clone()));
        }
    }

    // Validate subjob if present
    if let Some(ref subjob) = task.subjob {
        validate_subjob(subjob)?;
    }

    // Validate parallel if present
    if let Some(ref parallel) = task.parallel {
        validate_parallel(parallel)?;
    }

    // Validate each if present
    if let Some(ref each) = task.each {
        validate_each(each)?;
    }

    // Validate pre tasks (dive)
    if let Some(ref pre) = task.pre {
        pre.iter().try_for_each(validate_aux_task)?;
    }

    // Validate post tasks (dive)
    if let Some(ref post) = task.post {
        post.iter().try_for_each(validate_aux_task)?;
    }

    // Validate sidecars (dive)
    if let Some(ref sidecars) = task.sidecars {
        sidecars.iter().try_for_each(validate_sidecar)?;
    }

    Ok(())
}

/// Validates an auxiliary task (pre/post)
fn validate_aux_task(task: &AuxTask) -> ValidationResult<()> {
    // Check for name
    if task.name.as_ref().map_or(true, |n| n.trim().is_empty()) {
        return Err(ValidationError::Required {
            field: "name".to_string(),
        });
    }

    // Validate timeout if present
    if let Some(ref timeout) = task.timeout {
        if !timeout.is_empty() && !is_valid_duration(timeout) {
            return Err(ValidationError::InvalidDuration(timeout.clone()));
        }
    }

    Ok(())
}

/// Validates a sidecar task
fn validate_sidecar(task: &SidecarTask) -> ValidationResult<()> {
    // Check for name
    if task.name.as_ref().map_or(true, |n| n.trim().is_empty()) {
        return Err(ValidationError::Required {
            field: "name".to_string(),
        });
    }

    // Validate timeout if present
    if let Some(ref timeout) = task.timeout {
        if !timeout.is_empty() && !is_valid_duration(timeout) {
            return Err(ValidationError::InvalidDuration(timeout.clone()));
        }
    }

    // Validate probe if present
    if let Some(ref probe) = task.probe {
        validate_probe(probe)?;
    }

    Ok(())
}

/// Validates task type constraints (mutually exclusive)
fn validate_task_type_constraints(task: &Task) -> ValidationResult<()> {
    let has_parallel = task.parallel.is_some();
    let has_each = task.each.is_some();
    let has_subjob = task.subjob.is_some();

    let count = [has_parallel, has_each, has_subjob]
        .iter()
        .filter(|&&x| x)
        .count();

    if count > 1 {
        return Err(ValidationError::TaskTypeConflict(
            "task can only have one of parallel, each, or subjob".to_string(),
        ));
    }

    Ok(())
}

/// Validates composite task rules
fn validate_composite_task(task: &Task) -> ValidationResult<()> {
    // Composite tasks cannot have image, cmd, entrypoint, run, env, queue, pre, post, mounts, retry, limits, timeout
    if task.image.is_some() && !task.image.as_ref().unwrap().is_empty() {
        return Err(ValidationError::Invalid {
            field: "image".to_string(),
            message: "cannot be set on composite task".to_string(),
        });
    }

    if task.cmd.is_some() && !task.cmd.as_ref().unwrap().is_empty() {
        return Err(ValidationError::Invalid {
            field: "cmd".to_string(),
            message: "cannot be set on composite task".to_string(),
        });
    }

    if task.entrypoint.is_some() && !task.entrypoint.as_ref().unwrap().is_empty() {
        return Err(ValidationError::Invalid {
            field: "entrypoint".to_string(),
            message: "cannot be set on composite task".to_string(),
        });
    }

    if task.run.is_some() && !task.run.as_ref().unwrap().is_empty() {
        return Err(ValidationError::Invalid {
            field: "run".to_string(),
            message: "cannot be set on composite task".to_string(),
        });
    }

    if task.env.is_some() && !task.env.as_ref().unwrap().is_empty() {
        return Err(ValidationError::Invalid {
            field: "env".to_string(),
            message: "cannot be set on composite task".to_string(),
        });
    }

    if task.queue.is_some() && !task.queue.as_ref().unwrap().is_empty() {
        return Err(ValidationError::Invalid {
            field: "queue".to_string(),
            message: "cannot be set on composite task".to_string(),
        });
    }

    if task.pre.is_some() && !task.pre.as_ref().unwrap().is_empty() {
        return Err(ValidationError::Invalid {
            field: "pre".to_string(),
            message: "cannot be set on composite task".to_string(),
        });
    }

    if task.post.is_some() && !task.post.as_ref().unwrap().is_empty() {
        return Err(ValidationError::Invalid {
            field: "post".to_string(),
            message: "cannot be set on composite task".to_string(),
        });
    }

    if task.mounts.is_some() && !task.mounts.as_ref().unwrap().is_empty() {
        return Err(ValidationError::Invalid {
            field: "mounts".to_string(),
            message: "cannot be set on composite task".to_string(),
        });
    }

    if task.retry.is_some() {
        return Err(ValidationError::Invalid {
            field: "retry".to_string(),
            message: "cannot be set on composite task".to_string(),
        });
    }

    if task.limits.is_some() {
        return Err(ValidationError::Invalid {
            field: "limits".to_string(),
            message: "cannot be set on composite task".to_string(),
        });
    }

    if task.timeout.is_some() && !task.timeout.as_ref().unwrap().is_empty() {
        return Err(ValidationError::Invalid {
            field: "timeout".to_string(),
            message: "cannot be set on composite task".to_string(),
        });
    }

    Ok(())
}

/// Validates a subjob
fn validate_subjob(subjob: &SubJob) -> ValidationResult<()> {
    if subjob.name.as_ref().map_or(true, |n| n.trim().is_empty()) {
        return Err(ValidationError::Required {
            field: "subjob.name".to_string(),
        });
    }

    if let Some(ref webhooks) = subjob.webhooks {
        for webhook in webhooks {
            validate_webhook(webhook)?;
        }
    }

    // Recursively validate each task in subjob (dive behavior)
    if let Some(ref tasks) = subjob.tasks {
        tasks.iter().try_for_each(validate_task)?;
    }

    Ok(())
}

/// Validates a parallel task
fn validate_parallel(parallel: &Parallel) -> ValidationResult<()> {
    let tasks = parallel.tasks.as_ref().ok_or(ValidationError::Required {
        field: "parallel.tasks".to_string(),
    })?;

    if tasks.is_empty() {
        return Err(ValidationError::Range {
            field: "parallel.tasks".to_string(),
            min: 1,
            max: i64::MAX,
        });
    }

    // Recursively validate each task in parallel (dive behavior)
    tasks.iter().try_for_each(validate_task)
}

/// Validates an each task
fn validate_each(each: &Each) -> ValidationResult<()> {
    if each.list.as_ref().map_or(true, |l| l.trim().is_empty()) {
        return Err(ValidationError::Required {
            field: "each.list".to_string(),
        });
    }

    // Validate list expression using expression engine
    if let Some(ref list) = each.list {
        if !list.is_empty() && !valid_expr(list) {
            return Err(ValidationError::InvalidExpr(list.clone()));
        }
    }

    let task = each.task.as_ref().ok_or(ValidationError::Required {
        field: "each.task".to_string(),
    })?;

    if !(0..=99999).contains(&each.concurrency) {
        return Err(ValidationError::Range {
            field: "each.concurrency".to_string(),
            min: 0,
            max: 99999,
        });
    }

    // Recursively validate the inner task (dive behavior)
    validate_task(task)
}

/// Validates job defaults
fn validate_job_defaults(defaults: &JobDefaults) -> ValidationResult<()> {
    if let Some(ref timeout) = defaults.timeout {
        if !timeout.is_empty() && !is_valid_duration(timeout) {
            return Err(ValidationError::InvalidDuration(timeout.clone()));
        }
    }

    if let Some(ref queue) = defaults.queue {
        if !queue.is_empty() && !is_valid_queue(queue) {
            return Err(ValidationError::InvalidQueue(queue.clone()));
        }
    }

    if !(0..=9).contains(&defaults.priority) {
        return Err(ValidationError::Range {
            field: "defaults.priority".to_string(),
            min: 0,
            max: 9,
        });
    }

    Ok(())
}

/// Validates a webhook
fn validate_webhook(webhook: &Webhook) -> ValidationResult<()> {
    if webhook.url.as_ref().map_or(true, |u| u.trim().is_empty()) {
        return Err(ValidationError::Required {
            field: "webhook.url".to_string(),
        });
    }

    // Validate if expression
    if let Some(ref r#if) = webhook.r#if {
        if !r#if.is_empty() && !valid_expr(r#if) {
            return Err(ValidationError::InvalidExpr(r#if.clone()));
        }
    }

    Ok(())
}

/// Validates a permission
fn validate_permission(perm: &Permission) -> ValidationResult<()> {
    let has_user = perm.user.as_ref().map_or(false, |u| !u.is_empty());
    let has_role = perm.role.as_ref().map_or(false, |r| !r.is_empty());

    if !has_user && !has_role {
        return Err(ValidationError::InvalidPermission(
            "either user or role must be specified".to_string(),
        ));
    }

    if has_user && has_role {
        return Err(ValidationError::InvalidPermission(
            "cannot specify both user and role".to_string(),
        ));
    }

    Ok(())
}

/// Validates a mount
fn validate_mount(mount: &Mount) -> ValidationResult<()> {
    let mount_type = mount.mount_type_or_default();

    if mount_type.is_empty() {
        return Err(ValidationError::InvalidMount(
            "type is required".to_string(),
        ));
    }

    if mount_type == "volume"
        && mount.source.is_some()
        && !mount.source.as_ref().unwrap().is_empty()
    {
        return Err(ValidationError::InvalidMount(
            "volume mount cannot have source".to_string(),
        ));
    }

    if mount_type == "volume" && mount.target.as_ref().map_or(true, |t| t.is_empty()) {
        return Err(ValidationError::InvalidMount(
            "target is required".to_string(),
        ));
    }

    if mount_type == "bind" && mount.source.as_ref().map_or(true, |s| s.is_empty()) {
        return Err(ValidationError::InvalidMount(
            "source is required for bind mount".to_string(),
        ));
    }

    // Validate source pattern if present
    if let Some(ref source) = mount.source {
        if !source.is_empty() && !MOUNT_PATTERN.is_match(source) {
            return Err(ValidationError::InvalidMount(
                "invalid source pattern".to_string(),
            ));
        }
    }

    // Validate target pattern if present
    if let Some(ref target) = mount.target {
        if !target.is_empty() && !MOUNT_PATTERN.is_match(target) {
            return Err(ValidationError::InvalidMount(
                "invalid target pattern".to_string(),
            ));
        }

        if target == "/tork" {
            return Err(ValidationError::InvalidMount(
                "/tork is reserved".to_string(),
            ));
        }
    }

    Ok(())
}

/// Validates an expression string using basic syntax checking
fn valid_expr(expr: &str) -> bool {
    let trimmed = expr.trim();
    if trimmed.is_empty() {
        return false;
    }

    // Basic syntax validation:
    // 1. Check for balanced parentheses
    // 2. Check for obviously invalid characters

    let parens = trimmed.chars().fold(0i32, |acc, c| match c {
        '(' => acc + 1,
        ')' => acc - 1,
        _ => acc,
    });
    if parens != 0 {
        return false;
    }

    // Check for invalid characters (basic ASCII validation)
    for c in trimmed.chars() {
        if !c.is_ascii_graphic() && !c.is_whitespace() && c != '_' && c != '.' {
            return false;
        }
    }

    true
}

/// Validates a queue name
fn is_valid_queue(queue: &str) -> bool {
    if queue.is_empty() {
        return true;
    }

    // Cannot start with x- (exclusive prefix)
    if queue.starts_with("x-") {
        return false;
    }

    // Cannot be a coordinator queue
    if matches!(
        queue,
        "pending"
            | "started"
            | "completed"
            | "error"
            | "heartbeat"
            | "jobs"
            | "logs"
            | "progress"
            | "redeliveries"
    ) {
        return false;
    }

    true
}

/// Validates a cron expression
fn validate_cron(cron: &str) -> ValidationResult<()> {
    if cron.is_empty() {
        return Err(ValidationError::InvalidCron("empty expression".to_string()));
    }

    // Basic validation - check for at least 5 fields
    let parts: Vec<&str> = cron.split_whitespace().collect();
    if parts.len() < 5 {
        return Err(ValidationError::InvalidCron(
            "too few fields (expected 5)".to_string(),
        ));
    }

    // TODO: Use a proper cron parser library
    // For now, just do basic structural validation
    Ok(())
}

/// Validates a wait configuration
fn validate_wait(wait: &Wait) -> ValidationResult<()> {
    if wait.timeout.is_empty() {
        return Err(ValidationError::InvalidWait(
            "timeout is required".to_string(),
        ));
    }

    if !is_valid_duration(&wait.timeout) {
        return Err(ValidationError::InvalidDuration(wait.timeout.clone()));
    }

    Ok(())
}

/// Validates a probe configuration
fn validate_probe(probe: &Probe) -> ValidationResult<()> {
    // Port must be in range 1-65535
    if !(1..=65535).contains(&probe.port) {
        return Err(ValidationError::InvalidProbe(format!(
            "port must be between 1 and 65535, got {}",
            probe.port
        )));
    }

    // Timeout must be valid duration if present
    if let Some(ref timeout) = probe.timeout {
        if !timeout.is_empty() && !is_valid_duration(timeout) {
            return Err(ValidationError::InvalidDuration(timeout.clone()));
        }
    }

    // Path must be non-empty if present
    if let Some(ref path) = probe.path {
        if path.is_empty() {
            return Err(ValidationError::InvalidProbe(
                "path must be non-empty if present".to_string(),
            ));
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::task::Retry;

    #[test]
    fn test_validate_min_job() {
        let job = Job {
            name: Some("test job".to_string()),
            tasks: Some(vec![Task::new("test task", "some:image")]),
            ..Default::default()
        };

        let result = validate_job(&job);
        assert!(result.is_ok(), "{:?}", result);
    }

    #[test]
    fn test_validate_job_no_tasks() {
        let job = Job {
            name: Some("test job".to_string()),
            tasks: Some(vec![]),
            ..Default::default()
        };

        let result = validate_job(&job);
        assert!(result.is_err());
    }

    #[test]
    fn test_validate_job_no_name() {
        let job = Job {
            name: Some("test job".to_string()),
            tasks: Some(vec![Task {
                name: None,
                image: Some("some:image".to_string()),
                ..Default::default()
            }]),
            ..Default::default()
        };

        let result = validate_job(&job);
        assert!(result.is_err());
    }

    #[test]
    fn test_validate_retry_limit() {
        let task = Task {
            name: Some("test task".to_string()),
            image: Some("some:image".to_string()),
            retry: Some(Retry { limit: 5 }),
            ..Default::default()
        };

        let job = Job {
            name: Some("test job".to_string()),
            tasks: Some(vec![task]),
            ..Default::default()
        };

        assert!(validate_job(&job).is_ok());

        let task = Task {
            name: Some("test task".to_string()),
            image: Some("some:image".to_string()),
            retry: Some(Retry { limit: 50 }),
            ..Default::default()
        };

        let job = Job {
            name: Some("test job".to_string()),
            tasks: Some(vec![task]),
            ..Default::default()
        };

        assert!(validate_job(&job).is_err());
    }

    #[test]
    fn test_validate_timeout() {
        let task = Task {
            name: Some("test task".to_string()),
            image: Some("some:image".to_string()),
            timeout: Some("6h".to_string()),
            ..Default::default()
        };

        let job = Job {
            name: Some("test job".to_string()),
            tasks: Some(vec![task]),
            ..Default::default()
        };

        assert!(validate_job(&job).is_ok());

        let task = Task {
            name: Some("test task".to_string()),
            image: Some("some:image".to_string()),
            timeout: Some("1234".to_string()), // invalid - no units
            ..Default::default()
        };

        let job = Job {
            name: Some("test job".to_string()),
            tasks: Some(vec![task]),
            ..Default::default()
        };

        assert!(validate_job(&job).is_err());
    }

    #[test]
    fn test_validate_parallel_and_each() {
        // Both parallel and each - should fail
        let task = Task {
            name: Some("test task".to_string()),
            each: Some(Each {
                list: Some("1+1".to_string()),
                task: Some(Box::new(Task::new("inner", "image"))),
                ..Default::default()
            }),
            parallel: Some(Parallel {
                tasks: Some(vec![Task::new("inner", "image")]),
            }),
            ..Default::default()
        };

        let job = Job {
            name: Some("test job".to_string()),
            tasks: Some(vec![task]),
            ..Default::default()
        };

        assert!(validate_job(&job).is_err());
    }

    #[test]
    fn test_validate_mounts() {
        // Missing type and target
        let task = Task {
            name: Some("test task".to_string()),
            image: Some("some:image".to_string()),
            run: Some("some script".to_string()),
            mounts: Some(vec![Mount {
                mount_type: None,
                target: None,
                ..Default::default()
            }]),
            ..Default::default()
        };

        let job = Job {
            name: Some("test job".to_string()),
            tasks: Some(vec![task]),
            ..Default::default()
        };

        assert!(validate_job(&job).is_err());

        // Valid volume mount
        let task = Task {
            name: Some("test task".to_string()),
            image: Some("some:image".to_string()),
            run: Some("some script".to_string()),
            mounts: Some(vec![Mount {
                mount_type: Some("volume".to_string()),
                target: Some("/some/target".to_string()),
                ..Default::default()
            }]),
            ..Default::default()
        };

        let job = Job {
            name: Some("test job".to_string()),
            tasks: Some(vec![task]),
            ..Default::default()
        };

        assert!(validate_job(&job).is_ok());
    }

    #[test]
    fn test_validate_webhook() {
        let job = Job {
            name: Some("test job".to_string()),
            webhooks: Some(vec![Webhook {
                url: Some("http://example.com".to_string()),
                ..Default::default()
            }]),
            tasks: Some(vec![Task::new("test task", "some:image")]),
            ..Default::default()
        };

        assert!(validate_job(&job).is_ok());

        let job = Job {
            name: Some("test job".to_string()),
            webhooks: Some(vec![Webhook {
                url: Some("".to_string()),
                ..Default::default()
            }]),
            tasks: Some(vec![Task::new("test task", "some:image")]),
            ..Default::default()
        };

        assert!(validate_job(&job).is_err());
    }

    #[test]
    fn test_validate_queue() {
        let task = Task {
            name: Some("test task".to_string()),
            image: Some("some:image".to_string()),
            queue: Some("urgent".to_string()),
            ..Default::default()
        };

        let job = Job {
            name: Some("test job".to_string()),
            tasks: Some(vec![task]),
            ..Default::default()
        };

        assert!(validate_job(&job).is_ok());

        // Invalid queue - starts with x-
        let task = Task {
            name: Some("test task".to_string()),
            image: Some("some:image".to_string()),
            queue: Some("x-788222".to_string()),
            ..Default::default()
        };

        let job = Job {
            name: Some("test job".to_string()),
            tasks: Some(vec![task]),
            ..Default::default()
        };

        assert!(validate_job(&job).is_err());

        // Invalid queue - coordinator queue
        let task = Task {
            name: Some("test task".to_string()),
            image: Some("some:image".to_string()),
            queue: Some("jobs".to_string()),
            ..Default::default()
        };

        let job = Job {
            name: Some("test job".to_string()),
            tasks: Some(vec![task]),
            ..Default::default()
        };

        assert!(validate_job(&job).is_err());
    }

    #[test]
    fn test_validate_var_length() {
        let task = Task {
            name: Some("test task".to_string()),
            var: Some("somevar".to_string()),
            ..Default::default()
        };

        let job = Job {
            name: Some("test job".to_string()),
            tasks: Some(vec![task]),
            ..Default::default()
        };

        assert!(validate_job(&job).is_ok());

        // 64 chars - should pass
        let task = Task {
            name: Some("test task".to_string()),
            var: Some("a".repeat(64)),
            ..Default::default()
        };

        let job = Job {
            name: Some("test job".to_string()),
            tasks: Some(vec![task]),
            ..Default::default()
        };

        assert!(validate_job(&job).is_ok());

        // 65 chars - should fail
        let task = Task {
            name: Some("test task".to_string()),
            var: Some("a".repeat(65)),
            ..Default::default()
        };

        let job = Job {
            name: Some("test job".to_string()),
            tasks: Some(vec![task]),
            ..Default::default()
        };

        assert!(validate_job(&job).is_err());
    }

    #[test]
    fn test_validate_subjob() {
        let task = Task {
            name: Some("test task".to_string()),
            subjob: Some(SubJob {
                name: Some("test sub job".to_string()),
                webhooks: Some(vec![Webhook {
                    url: Some("http://example.com".to_string()),
                    ..Default::default()
                }]),
                tasks: Some(vec![Task::new("test task", "some task")]),
                ..Default::default()
            }),
            ..Default::default()
        };

        let job = Job {
            name: Some("test job".to_string()),
            tasks: Some(vec![task]),
            ..Default::default()
        };

        assert!(validate_job(&job).is_ok());
    }

    #[test]
    fn test_validate_subjob_bad_webhook() {
        let task = Task {
            name: Some("test task".to_string()),
            subjob: Some(SubJob {
                name: Some("test sub job".to_string()),
                webhooks: Some(vec![Webhook {
                    url: Some("".to_string()),
                    ..Default::default()
                }]),
                tasks: Some(vec![Task::new("test task", "some task")]),
                ..Default::default()
            }),
            ..Default::default()
        };

        let job = Job {
            name: Some("test job".to_string()),
            tasks: Some(vec![task]),
            ..Default::default()
        };

        assert!(validate_job(&job).is_err());
    }
}
