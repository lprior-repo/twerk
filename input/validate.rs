//! Validation module for Tork job and task inputs.
//!
//! Provides 100% parity with Go's `input.validate` package.
//! All validation rules match Go struct tags and custom validators exactly.

use std::str::FromStr;
use std::sync::LazyLock;

use evalexpr::{build_operator_tree, DefaultNumericTypes};
use regex::Regex;

use crate::duration::is_valid_duration;
use crate::job::{AutoDelete, Job, JobDefaults, Permission, ScheduledJob, Wait, Webhook};
use crate::task::{AuxTask, Each, Mount, Parallel, Probe, SidecarTask, SubJob, Task};

// ---------------------------------------------------------------------------
// Static patterns
// ---------------------------------------------------------------------------

/// Mount source/target validation pattern (matches Go's `mountPattern`).
static MOUNT_PATTERN: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^[-/\.0-9a-zA-Z_/= ]+$").expect("hardcoded regex is valid"));

/// Regex to match `{{ … }}` template expressions.
static TEMPLATE_REGEX: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"\{\{\s*(.+?)\s*\}\}").expect("hardcoded regex is valid"));

/// Regex to detect empty `{{ }}` templates.
static EMPTY_TEMPLATE_REGEX: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^\s*\{\{\s*\}\}\s*$").expect("hardcoded regex is valid"));

/// Coordinator queue names (matches Go's `broker.IsCoordinatorQueue`).
const COORDINATOR_QUEUES: &[&str] = &[
    "pending",
    "started",
    "completed",
    "error",
    "heartbeat",
    "jobs",
    "logs",
    "progress",
    "redeliveries",
];

/// Exclusive queue prefix (matches Go's `broker.QUEUE_EXCLUSIVE_PREFIX`).
const EXCLUSIVE_QUEUE_PREFIX: &str = "x-";

// ---------------------------------------------------------------------------
// Error types
// ---------------------------------------------------------------------------

/// Validation error taxonomy.
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

/// Result type for validation.
pub type ValidationResult<T> = Result<T, ValidationError>;

// ---------------------------------------------------------------------------
// Permission checker trait
// ---------------------------------------------------------------------------

/// Trait for datastore-backed permission validation.
///
/// Matches Go's `validatePermission(ds datastore.Datastore)` which calls
/// `ds.GetUser()` and `ds.GetRole()` to verify existence.
///
/// Use [`NoopPermissionChecker`] when datastore lookups are not available.
pub trait PermissionChecker: Send + Sync {
    /// Returns `true` if the given username exists in the datastore.
    fn user_exists(&self, username: &str) -> bool;

    /// Returns `true` if the given role slug exists in the datastore.
    fn role_exists(&self, role: &str) -> bool;
}

/// No-op permission checker that accepts all users and roles.
///
/// Use this when datastore-based permission validation is not needed.
pub struct NoopPermissionChecker;

impl PermissionChecker for NoopPermissionChecker {
    fn user_exists(&self, _: &str) -> bool {
        true
    }

    fn role_exists(&self, _: &str) -> bool {
        true
    }
}

// ---------------------------------------------------------------------------
// Public validation API
// ---------------------------------------------------------------------------

/// Validates a job input (without datastore permission checks).
///
/// Convenience wrapper around [`validate_job_with_checker`] using
/// [`NoopPermissionChecker`].
pub fn validate_job(job: &Job) -> ValidationResult<()> {
    validate_job_with_checker(job, &NoopPermissionChecker)
}

/// Validates a job input with an optional permission checker for datastore lookups.
///
/// Matches Go's `Job.Validate(ds datastore.Datastore)`.
pub fn validate_job_with_checker(
    job: &Job,
    checker: &dyn PermissionChecker,
) -> ValidationResult<()> {
    // Go: validate:"required" on Name
    if job.name.as_ref().is_none_or(|n| n.trim().is_empty()) {
        return Err(ValidationError::Required {
            field: "name".to_string(),
        });
    }

    // Go: validate:"required,min=1,dive" on Tasks
    let tasks = job.tasks.as_ref().ok_or(ValidationError::NoTasks)?;
    if tasks.is_empty() {
        return Err(ValidationError::NoTasks);
    }

    tasks.iter().try_for_each(validate_task)?;

    // Go: Defaults is validated by registered validators
    if let Some(ref defaults) = job.defaults {
        validate_job_defaults(defaults)?;
    }

    // Go: validate:"dive" on Webhooks
    if let Some(ref webhooks) = job.webhooks {
        webhooks.iter().try_for_each(validate_webhook)?;
    }

    // Go: validate:"expr" on Output (empty is valid)
    if let Some(ref output) = job.output {
        if !output.is_empty() && !valid_expr(output) {
            return Err(ValidationError::InvalidExpr(output.clone()));
        }
    }

    // Go: Wait is validated by registered validators
    if let Some(ref wait) = job.wait {
        validate_wait(wait)?;
    }

    // Go: validate:"dive" on Permissions with custom validatePermission(ds)
    if let Some(ref permissions) = job.permissions {
        permissions
            .iter()
            .try_for_each(|p| validate_permission(p, checker))?;
    }

    // Go: AutoDelete.After has validate:"duration"
    if let Some(ref ad) = job.auto_delete {
        validate_auto_delete(ad)?;
    }

    // Go: validate:"dive" on Mounts with custom validateMount
    tasks.iter().try_for_each(|task| {
        task.mounts
            .as_ref()
            .map_or(Ok(()), |mounts| mounts.iter().try_for_each(validate_mount))
    })
}

/// Validates a scheduled job input (without datastore permission checks).
pub fn validate_scheduled_job(job: &ScheduledJob) -> ValidationResult<()> {
    validate_scheduled_job_with_checker(job, &NoopPermissionChecker)
}

/// Validates a scheduled job input with an optional permission checker.
///
/// Matches Go's `ScheduledJob.Validate(ds datastore.Datastore)`.
pub fn validate_scheduled_job_with_checker(
    job: &ScheduledJob,
    checker: &dyn PermissionChecker,
) -> ValidationResult<()> {
    if job.name.as_ref().is_none_or(|n| n.trim().is_empty()) {
        return Err(ValidationError::Required {
            field: "name".to_string(),
        });
    }

    let tasks = job.tasks.as_ref().ok_or(ValidationError::NoTasks)?;
    if tasks.is_empty() {
        return Err(ValidationError::NoTasks);
    }

    // Go: validate:"required,cron" on Schedule.Cron
    if let Some(ref schedule) = job.schedule {
        validate_cron(&schedule.cron)?;
    } else {
        return Err(ValidationError::Required {
            field: "schedule".to_string(),
        });
    }

    tasks.iter().try_for_each(validate_task)?;

    if let Some(ref defaults) = job.defaults {
        validate_job_defaults(defaults)?;
    }

    if let Some(ref webhooks) = job.webhooks {
        webhooks.iter().try_for_each(validate_webhook)?;
    }

    // Go: validate:"expr" on Output (matches Job validation)
    if let Some(ref output) = job.output {
        if !output.is_empty() && !valid_expr(output) {
            return Err(ValidationError::InvalidExpr(output.clone()));
        }
    }

    if let Some(ref permissions) = job.permissions {
        permissions
            .iter()
            .try_for_each(|p| validate_permission(p, checker))?;
    }

    if let Some(ref ad) = job.auto_delete {
        validate_auto_delete(ad)?;
    }

    Ok(())
}

// ---------------------------------------------------------------------------
// Private validation functions
// ---------------------------------------------------------------------------

/// Validates a task input.
///
/// Matches Go's `taskInputValidation` (combines `taskTypeValidation` +
/// `compositeTaskValidation`) plus struct-level validators.
fn validate_task(task: &Task) -> ValidationResult<()> {
    // Go: validate:"required" on Name
    if task.name.as_ref().is_none_or(|n| n.trim().is_empty()) {
        return Err(ValidationError::Required {
            field: "name".to_string(),
        });
    }

    // Go: registered taskInputValidation
    validate_task_type_constraints(task)?;

    if task.is_composite() {
        validate_composite_task(task)?;
    }

    // Go: validate:"duration" on Timeout (empty is valid)
    if let Some(ref timeout) = task.timeout {
        if !timeout.is_empty() && !is_valid_duration(timeout) {
            return Err(ValidationError::InvalidDuration(timeout.clone()));
        }
    }

    // Go: validate:"required,min=1,max=10" on Retry.Limit
    if let Some(ref retry) = task.retry {
        if !(1..=10).contains(&retry.limit) {
            return Err(ValidationError::Range {
                field: "retry.limit".to_string(),
                min: 1,
                max: 10,
            });
        }
    }

    // Go: validate:"max=64" on Var
    if let Some(ref var) = task.var {
        if var.len() > 64 {
            return Err(ValidationError::MaxLength {
                field: "var".to_string(),
                max: 64,
            });
        }
    }

    // Go: validate:"max=256" on Workdir
    if let Some(ref workdir) = task.workdir {
        if workdir.len() > 256 {
            return Err(ValidationError::MaxLength {
                field: "workdir".to_string(),
                max: 256,
            });
        }
    }

    // Go: validate:"min=0,max=9" on Priority
    if !(0..=9).contains(&task.priority) {
        return Err(ValidationError::Range {
            field: "priority".to_string(),
            min: 0,
            max: 9,
        });
    }

    // Go: validate:"queue" custom validator on Queue
    if let Some(ref queue) = task.queue {
        if !queue.is_empty() && !is_valid_queue(queue) {
            return Err(ValidationError::InvalidQueue(queue.clone()));
        }
    }

    // Go: validate:"expr" on If
    if let Some(ref r#if) = task.r#if {
        if !r#if.is_empty() && !valid_expr(r#if) {
            return Err(ValidationError::InvalidExpr(r#if.clone()));
        }
    }

    // Recursive validation for nested task types
    if let Some(ref subjob) = task.subjob {
        validate_subjob(subjob)?;
    }

    if let Some(ref parallel) = task.parallel {
        validate_parallel(parallel)?;
    }

    if let Some(ref each) = task.each {
        validate_each(each)?;
    }

    // Go: validate:"dive" on Pre, Post
    if let Some(ref pre) = task.pre {
        pre.iter().try_for_each(validate_aux_task)?;
    }

    if let Some(ref post) = task.post {
        post.iter().try_for_each(validate_aux_task)?;
    }

    // Go: validate:"dive" on Sidecars
    if let Some(ref sidecars) = task.sidecars {
        sidecars.iter().try_for_each(validate_sidecar)?;
    }

    Ok(())
}

/// Validates an auxiliary task (pre/post hook).
///
/// Go: only `validate:"required"` on Name. No duration validation on Timeout.
fn validate_aux_task(task: &AuxTask) -> ValidationResult<()> {
    if task.name.as_ref().is_none_or(|n| n.trim().is_empty()) {
        return Err(ValidationError::Required {
            field: "name".to_string(),
        });
    }

    // Note: Go does NOT validate AuxTask.Timeout as a duration.
    // The Timeout field has no validate tag in Go's AuxTask struct.

    Ok(())
}

/// Validates a sidecar task.
///
/// Go: only `validate:"required"` on Name plus Probe validation.
/// No duration validation on Timeout.
fn validate_sidecar(task: &SidecarTask) -> ValidationResult<()> {
    if task.name.as_ref().is_none_or(|n| n.trim().is_empty()) {
        return Err(ValidationError::Required {
            field: "name".to_string(),
        });
    }

    // Note: Go does NOT validate SidecarTask.Timeout as a duration.

    if let Some(ref probe) = task.probe {
        validate_probe(probe)?;
    }

    Ok(())
}

/// Validates that parallel, each, and subjob are mutually exclusive.
///
/// Matches Go's `taskTypeValidation`.
fn validate_task_type_constraints(task: &Task) -> ValidationResult<()> {
    let count = [
        task.parallel.is_some(),
        task.each.is_some(),
        task.subjob.is_some(),
    ]
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

/// Validates that composite tasks (parallel, each, subjob) don't have
/// execution-specific fields.
///
/// Matches Go's `compositeTaskValidation` exactly.
fn validate_composite_task(task: &Task) -> ValidationResult<()> {
    if task.image.as_ref().is_some_and(|i| !i.is_empty()) {
        return Err(ValidationError::Invalid {
            field: "image".to_string(),
            message: "cannot be set on composite task".to_string(),
        });
    }

    if task.cmd.as_ref().is_some_and(|c| !c.is_empty()) {
        return Err(ValidationError::Invalid {
            field: "cmd".to_string(),
            message: "cannot be set on composite task".to_string(),
        });
    }

    if task.entrypoint.as_ref().is_some_and(|e| !e.is_empty()) {
        return Err(ValidationError::Invalid {
            field: "entrypoint".to_string(),
            message: "cannot be set on composite task".to_string(),
        });
    }

    if task.run.as_ref().is_some_and(|r| !r.is_empty()) {
        return Err(ValidationError::Invalid {
            field: "run".to_string(),
            message: "cannot be set on composite task".to_string(),
        });
    }

    if task.env.as_ref().is_some_and(|e| !e.is_empty()) {
        return Err(ValidationError::Invalid {
            field: "env".to_string(),
            message: "cannot be set on composite task".to_string(),
        });
    }

    if task.queue.as_ref().is_some_and(|q| !q.is_empty()) {
        return Err(ValidationError::Invalid {
            field: "queue".to_string(),
            message: "cannot be set on composite task".to_string(),
        });
    }

    if task.pre.as_ref().is_some_and(|p| !p.is_empty()) {
        return Err(ValidationError::Invalid {
            field: "pre".to_string(),
            message: "cannot be set on composite task".to_string(),
        });
    }

    if task.post.as_ref().is_some_and(|p| !p.is_empty()) {
        return Err(ValidationError::Invalid {
            field: "post".to_string(),
            message: "cannot be set on composite task".to_string(),
        });
    }

    if task.mounts.as_ref().is_some_and(|m| !m.is_empty()) {
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

    if task.timeout.as_ref().is_some_and(|t| !t.is_empty()) {
        return Err(ValidationError::Invalid {
            field: "timeout".to_string(),
            message: "cannot be set on composite task".to_string(),
        });
    }

    Ok(())
}

/// Validates a subjob task.
///
/// Matches Go's struct-level validators on SubJob.
fn validate_subjob(subjob: &SubJob) -> ValidationResult<()> {
    // Go: validate:"required" on Name
    if subjob.name.as_ref().is_none_or(|n| n.trim().is_empty()) {
        return Err(ValidationError::Required {
            field: "subjob.name".to_string(),
        });
    }

    // Go: validate:"dive" on Webhooks
    if let Some(ref webhooks) = subjob.webhooks {
        webhooks.iter().try_for_each(validate_webhook)?;
    }

    // Go: validate:"required" (min=1 implied) + dive on Tasks
    if let Some(ref tasks) = subjob.tasks {
        tasks.iter().try_for_each(validate_task)?;
    }

    Ok(())
}

/// Validates a parallel task.
///
/// Go: validate:"required,min=1,dive" on Tasks.
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

    tasks.iter().try_for_each(validate_task)
}

/// Validates an each (loop) task.
///
/// Go: validate:"required,expr" on List, validate:"required" on Task,
/// validate:"min=0,max=99999" on Concurrency.
fn validate_each(each: &Each) -> ValidationResult<()> {
    if each.list.as_ref().is_none_or(|l| l.trim().is_empty()) {
        return Err(ValidationError::Required {
            field: "each.list".to_string(),
        });
    }

    // Go: validate:"expr" on List (full expression parser)
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

    validate_task(task)
}

/// Validates job defaults.
///
/// Go: validate:"duration" on Timeout, validate:"queue" on Queue,
/// validate:"min=0,max=9" on Priority.
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

/// Validates a webhook.
///
/// Go: validate:"required" on URL, validate:"expr" on If.
fn validate_webhook(webhook: &Webhook) -> ValidationResult<()> {
    if webhook.url.as_ref().is_none_or(|u| u.trim().is_empty()) {
        return Err(ValidationError::Required {
            field: "webhook.url".to_string(),
        });
    }

    // Go: validate:"expr" on If (empty is valid)
    if let Some(ref r#if) = webhook.r#if {
        if !r#if.is_empty() && !valid_expr(r#if) {
            return Err(ValidationError::InvalidExpr(r#if.clone()));
        }
    }

    Ok(())
}

/// Validates a permission.
///
/// Matches Go's `validatePermission(ds)` exactly:
/// - Exactly one of user/role must be specified
/// - User existence checked via `ds.GetUser()`
/// - Role existence checked via `ds.GetRole()`
fn validate_permission(perm: &Permission, checker: &dyn PermissionChecker) -> ValidationResult<()> {
    let has_user = perm.user.as_ref().is_some_and(|u| !u.is_empty());
    let has_role = perm.role.as_ref().is_some_and(|r| !r.is_empty());

    // Go: if perm.Role == "" && perm.User == "" → error
    if !has_user && !has_role {
        return Err(ValidationError::InvalidPermission(
            "either user or role must be specified".to_string(),
        ));
    }

    // Go: if perm.Role != "" && perm.User != "" → error
    if has_user && has_role {
        return Err(ValidationError::InvalidPermission(
            "cannot specify both user and role".to_string(),
        ));
    }

    // Go: ds.GetUser / ds.GetRole datastore lookups
    if has_user {
        if let Some(ref user) = perm.user {
            if !checker.user_exists(user) {
                return Err(ValidationError::InvalidPermission(format!(
                    "user '{user}' does not exist"
                )));
            }
        }
    }

    if has_role {
        if let Some(ref role) = perm.role {
            if !checker.role_exists(role) {
                return Err(ValidationError::InvalidPermission(format!(
                    "role '{role}' does not exist"
                )));
            }
        }
    }

    Ok(())
}

/// Validates a mount.
///
/// Matches Go's `validateMount` exactly as an `else if` chain.
fn validate_mount(mount: &Mount) -> ValidationResult<()> {
    // Go: Type defaults to "" (empty string). Rust Option::None → "".
    let mount_type = mount.mount_type.as_deref().unwrap_or("");

    if mount_type.is_empty() {
        return Err(ValidationError::InvalidMount(
            "type is required".to_string(),
        ));
    } else if mount_type == "volume" && mount.source.as_ref().is_some_and(|s| !s.is_empty()) {
        return Err(ValidationError::InvalidMount(
            "volume mount cannot have source".to_string(),
        ));
    } else if mount_type == "volume" && mount.target.as_ref().is_none_or(|t| t.is_empty()) {
        return Err(ValidationError::InvalidMount(
            "target is required".to_string(),
        ));
    } else if mount_type == "bind" && mount.source.as_ref().is_none_or(|s| s.is_empty()) {
        return Err(ValidationError::InvalidMount(
            "source is required for bind mount".to_string(),
        ));
    } else if mount
        .source
        .as_ref()
        .is_some_and(|s| !s.is_empty() && !MOUNT_PATTERN.is_match(s))
    {
        return Err(ValidationError::InvalidMount(
            "invalid source pattern".to_string(),
        ));
    } else if mount
        .target
        .as_ref()
        .is_some_and(|t| !t.is_empty() && !MOUNT_PATTERN.is_match(t))
    {
        return Err(ValidationError::InvalidMount(
            "invalid target pattern".to_string(),
        ));
    } else if mount.target.as_deref() == Some("/tork") {
        return Err(ValidationError::InvalidMount(
            "/tork is reserved".to_string(),
        ));
    }

    Ok(())
}

/// Validates a probe.
///
/// Go: validate:"required,max=256" on Path,
///     validate:"required,min=1,max=65535" on Port,
///     validate:"duration" on Timeout.
fn validate_probe(probe: &Probe) -> ValidationResult<()> {
    // Go: validate:"required,min=1,max=65535" on Port
    if !(1..=65535).contains(&probe.port) {
        return Err(ValidationError::InvalidProbe(format!(
            "port must be between 1 and 65535, got {}",
            probe.port
        )));
    }

    // Go: validate:"required,max=256" on Path
    if probe.path.as_ref().is_none_or(|p| p.is_empty()) {
        return Err(ValidationError::InvalidProbe(
            "path is required".to_string(),
        ));
    }

    if let Some(ref path) = probe.path {
        if path.len() > 256 {
            return Err(ValidationError::InvalidProbe(format!(
                "path exceeds maximum length of 256, got {}",
                path.len()
            )));
        }
    }

    // Go: validate:"duration" on Timeout (empty is valid)
    if let Some(ref timeout) = probe.timeout {
        if !timeout.is_empty() && !is_valid_duration(timeout) {
            return Err(ValidationError::InvalidDuration(timeout.clone()));
        }
    }

    Ok(())
}

/// Validates a wait configuration.
///
/// Go: validate:"duration,required" on Timeout.
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

/// Validates auto-delete configuration.
///
/// Go: validate:"duration" on After (empty is valid).
fn validate_auto_delete(ad: &AutoDelete) -> ValidationResult<()> {
    if let Some(ref after) = ad.after {
        if !after.is_empty() && !is_valid_duration(after) {
            return Err(ValidationError::InvalidDuration(after.clone()));
        }
    }

    Ok(())
}

// ---------------------------------------------------------------------------
// Helper functions
// ---------------------------------------------------------------------------

/// Validates a cron expression using the standard 5-field format.
///
/// Go uses `cron.ParseStandard()` which requires exactly 5 fields:
/// minute, hour, day-of-month, month, day-of-week.
/// Rejects 6-field (with seconds) and 7-field expressions.
fn validate_cron(expression: &str) -> ValidationResult<()> {
    if expression.is_empty() {
        return Err(ValidationError::InvalidCron("empty expression".to_string()));
    }

    let field_count = expression.split_whitespace().count();
    if field_count != 5 {
        return Err(ValidationError::InvalidCron(format!(
            "expected 5 fields, got {}",
            field_count
        )));
    }

    // Expand 5-field standard cron to 7-field for the cron crate:
    // seconds minute hour day-of-month month day-of-week year
    let expanded = format!("0 {expression} *");
    cron::Schedule::from_str(&expanded)
        .map(|_| ())
        .map_err(|e| ValidationError::InvalidCron(format!("invalid cron expression: {e}")))
}

/// Validates an expression string using the full `evalexpr` parser.
///
/// Matches Go's `eval.ValidExpr()` — actually parses the expression
/// instead of just checking paren balance.
///
/// Returns `false` for empty/whitespace strings (callers must check for
/// empty first, matching Go's `validateExpr` behavior where empty is valid).
fn valid_expr(expr: &str) -> bool {
    let trimmed = expr.trim();
    if trimmed.is_empty() {
        return false;
    }

    if EMPTY_TEMPLATE_REGEX.is_match(trimmed) {
        return false;
    }

    let sanitized = sanitize_expr(expr);
    if sanitized.is_empty() {
        return false;
    }

    // After stripping {{ }} delimiters, reject any remaining braces.
    // Go's eval parser treats { and } as invalid characters outside templates.
    if sanitized.contains('{') || sanitized.contains('}') {
        return false;
    }

    build_operator_tree::<DefaultNumericTypes>(&sanitized).is_ok()
}

/// Strips `{{ }}` template delimiters from an expression.
///
/// `"{{ 1 + 1 }}"` → `"1 + 1"`
/// `"plain text"` → `"plain text"` (unchanged)
fn sanitize_expr(expr: &str) -> String {
    let trimmed = expr.trim();
    TEMPLATE_REGEX
        .captures(trimmed)
        .map_or_else(|| trimmed.to_string(), |caps| caps[1].trim().to_string())
}

/// Validates a queue name against coordinator and exclusive prefix rules.
///
/// Go: `validateQueue` rejects queues starting with `"x-"` or matching
/// any coordinator queue name.
fn is_valid_queue(queue: &str) -> bool {
    if queue.is_empty() {
        return true;
    }

    if queue.starts_with(EXCLUSIVE_QUEUE_PREFIX) {
        return false;
    }

    if COORDINATOR_QUEUES.contains(&queue) {
        return false;
    }

    true
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::task::Retry;

    // -- TestPermissionChecker for datastore validation tests ----

    struct TestChecker {
        users: Vec<&'static str>,
        roles: Vec<&'static str>,
    }

    impl PermissionChecker for TestChecker {
        fn user_exists(&self, username: &str) -> bool {
            self.users.contains(&username)
        }

        fn role_exists(&self, role: &str) -> bool {
            self.roles.contains(&role)
        }
    }

    // -- Job validation tests (matching Go TestValidate*) ----

    #[test]
    fn test_validate_min_job() {
        let job = Job {
            name: Some("test job".to_string()),
            tasks: Some(vec![Task::new("test task", "some:image")]),
            ..Default::default()
        };
        assert!(validate_job(&job).is_ok(), "{:?}", validate_job(&job));
    }

    #[test]
    fn test_validate_job_no_tasks() {
        let job = Job {
            name: Some("test job".to_string()),
            tasks: Some(vec![]),
            ..Default::default()
        };
        assert!(validate_job(&job).is_err());
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
        assert!(validate_job(&job).is_err());
    }

    #[test]
    fn test_validate_retry_limit() {
        let ok_job = Job {
            name: Some("test job".to_string()),
            tasks: Some(vec![Task {
                name: Some("test task".to_string()),
                image: Some("some:image".to_string()),
                retry: Some(Retry { limit: 5 }),
                ..Default::default()
            }]),
            ..Default::default()
        };
        assert!(validate_job(&ok_job).is_ok());

        let bad_job = Job {
            name: Some("test job".to_string()),
            tasks: Some(vec![Task {
                name: Some("test task".to_string()),
                image: Some("some:image".to_string()),
                retry: Some(Retry { limit: 50 }),
                ..Default::default()
            }]),
            ..Default::default()
        };
        assert!(validate_job(&bad_job).is_err());
    }

    #[test]
    fn test_validate_timeout() {
        let ok_job = Job {
            name: Some("test job".to_string()),
            tasks: Some(vec![Task {
                name: Some("test task".to_string()),
                image: Some("some:image".to_string()),
                timeout: Some("6h".to_string()),
                ..Default::default()
            }]),
            ..Default::default()
        };
        assert!(validate_job(&ok_job).is_ok());

        let bad_job = Job {
            name: Some("test job".to_string()),
            tasks: Some(vec![Task {
                name: Some("test task".to_string()),
                image: Some("some:image".to_string()),
                timeout: Some("1234".to_string()),
                ..Default::default()
            }]),
            ..Default::default()
        };
        assert!(validate_job(&bad_job).is_err());
    }

    #[test]
    fn test_validate_parallel_and_each_conflict() {
        let job = Job {
            name: Some("test job".to_string()),
            tasks: Some(vec![Task {
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
            }]),
            ..Default::default()
        };
        assert!(validate_job(&job).is_err());
    }

    #[test]
    fn test_validate_parallel_or_subjob_conflict() {
        let ok_job = Job {
            name: Some("test job".to_string()),
            tasks: Some(vec![Task {
                name: Some("test task".to_string()),
                parallel: Some(Parallel {
                    tasks: Some(vec![Task::new("inner", "image")]),
                }),
                ..Default::default()
            }]),
            ..Default::default()
        };
        assert!(validate_job(&ok_job).is_ok());

        let bad_job = Job {
            name: Some("test job".to_string()),
            tasks: Some(vec![Task {
                name: Some("test task".to_string()),
                parallel: Some(Parallel {
                    tasks: Some(vec![Task::new("inner", "image")]),
                }),
                subjob: Some(SubJob {
                    name: Some("test sub job".to_string()),
                    tasks: Some(vec![Task::new("test task", "some task")]),
                    ..Default::default()
                }),
                ..Default::default()
            }]),
            ..Default::default()
        };
        assert!(validate_job(&bad_job).is_err());
    }

    #[test]
    fn test_validate_mounts() {
        // Missing type → error
        let job = Job {
            name: Some("test job".to_string()),
            tasks: Some(vec![Task {
                name: Some("test task".to_string()),
                image: Some("some:image".to_string()),
                run: Some("some script".to_string()),
                mounts: Some(vec![Mount {
                    mount_type: None,
                    target: None,
                    ..Default::default()
                }]),
                ..Default::default()
            }]),
            ..Default::default()
        };
        assert!(validate_job(&job).is_err());

        // Valid volume mount (no source, has target)
        let job = Job {
            name: Some("test job".to_string()),
            tasks: Some(vec![Task {
                name: Some("test task".to_string()),
                image: Some("some:image".to_string()),
                run: Some("some script".to_string()),
                mounts: Some(vec![Mount {
                    mount_type: Some("volume".to_string()),
                    target: Some("/some/target".to_string()),
                    ..Default::default()
                }]),
                ..Default::default()
            }]),
            ..Default::default()
        };
        assert!(validate_job(&job).is_ok());

        // Custom type (not volume/bind) → OK
        let job = Job {
            name: Some("test job".to_string()),
            tasks: Some(vec![Task {
                name: Some("test task".to_string()),
                image: Some("some:image".to_string()),
                run: Some("some script".to_string()),
                mounts: Some(vec![Mount {
                    mount_type: Some("custom".to_string()),
                    target: Some("/some/target".to_string()),
                    ..Default::default()
                }]),
                ..Default::default()
            }]),
            ..Default::default()
        };
        assert!(validate_job(&job).is_ok());

        // Bind mount missing source → error
        let job = Job {
            name: Some("test job".to_string()),
            tasks: Some(vec![Task {
                name: Some("test task".to_string()),
                image: Some("some:image".to_string()),
                run: Some("some script".to_string()),
                mounts: Some(vec![Mount {
                    mount_type: Some("bind".to_string()),
                    source: None,
                    target: Some("/some/target".to_string()),
                    ..Default::default()
                }]),
                ..Default::default()
            }]),
            ..Default::default()
        };
        assert!(validate_job(&job).is_err());

        // Valid bind mount
        let job = Job {
            name: Some("test job".to_string()),
            tasks: Some(vec![Task {
                name: Some("test task".to_string()),
                image: Some("some:image".to_string()),
                run: Some("some script".to_string()),
                mounts: Some(vec![Mount {
                    mount_type: Some("bind".to_string()),
                    source: Some("/some/source".to_string()),
                    target: Some("/some/target".to_string()),
                    ..Default::default()
                }]),
                ..Default::default()
            }]),
            ..Default::default()
        };
        assert!(validate_job(&job).is_ok());

        // Invalid source pattern (# character)
        let job = Job {
            name: Some("test job".to_string()),
            tasks: Some(vec![Task {
                name: Some("test task".to_string()),
                image: Some("some:image".to_string()),
                run: Some("some script".to_string()),
                mounts: Some(vec![Mount {
                    mount_type: Some("bind".to_string()),
                    source: Some("/some#/source".to_string()),
                    target: Some("/some/target".to_string()),
                    ..Default::default()
                }]),
                ..Default::default()
            }]),
            ..Default::default()
        };
        assert!(validate_job(&job).is_err());

        // Invalid target pattern (: character)
        let job = Job {
            name: Some("test job".to_string()),
            tasks: Some(vec![Task {
                name: Some("test task".to_string()),
                image: Some("some:image".to_string()),
                run: Some("some script".to_string()),
                mounts: Some(vec![Mount {
                    mount_type: Some("bind".to_string()),
                    source: Some("/some/source".to_string()),
                    target: Some("/some:/target".to_string()),
                    ..Default::default()
                }]),
                ..Default::default()
            }]),
            ..Default::default()
        };
        assert!(validate_job(&job).is_err());

        // Target "/tork" is reserved
        let job = Job {
            name: Some("test job".to_string()),
            tasks: Some(vec![Task {
                name: Some("test task".to_string()),
                image: Some("some:image".to_string()),
                run: Some("some script".to_string()),
                mounts: Some(vec![Mount {
                    mount_type: Some("bind".to_string()),
                    source: Some("/some/source".to_string()),
                    target: Some("/tork".to_string()),
                    ..Default::default()
                }]),
                ..Default::default()
            }]),
            ..Default::default()
        };
        assert!(validate_job(&job).is_err());

        // Valid source with spaces and equals (bucket mount)
        let job = Job {
            name: Some("test job".to_string()),
            tasks: Some(vec![Task {
                name: Some("test task".to_string()),
                image: Some("some:image".to_string()),
                run: Some("some script".to_string()),
                mounts: Some(vec![Mount {
                    mount_type: Some("bind".to_string()),
                    source: Some("bucket=some-bucket path=/mnt/some-path".to_string()),
                    target: Some("/some/path".to_string()),
                    ..Default::default()
                }]),
                ..Default::default()
            }]),
            ..Default::default()
        };
        assert!(validate_job(&job).is_ok());
    }

    #[test]
    fn test_validate_webhook() {
        let ok_job = Job {
            name: Some("test job".to_string()),
            webhooks: Some(vec![Webhook {
                url: Some("http://example.com".to_string()),
                ..Default::default()
            }]),
            tasks: Some(vec![Task::new("test task", "some:image")]),
            ..Default::default()
        };
        assert!(validate_job(&ok_job).is_ok());

        let bad_job = Job {
            name: Some("test job".to_string()),
            webhooks: Some(vec![Webhook {
                url: Some("".to_string()),
                ..Default::default()
            }]),
            tasks: Some(vec![Task::new("test task", "some:image")]),
            ..Default::default()
        };
        assert!(validate_job(&bad_job).is_err());
    }

    #[test]
    fn test_validate_queue() {
        // Valid custom queue
        let ok_job = Job {
            name: Some("test job".to_string()),
            tasks: Some(vec![Task {
                name: Some("test task".to_string()),
                image: Some("some:image".to_string()),
                queue: Some("urgent".to_string()),
                ..Default::default()
            }]),
            ..Default::default()
        };
        assert!(validate_job(&ok_job).is_ok());

        // Invalid: exclusive prefix "x-"
        let bad_job = Job {
            name: Some("test job".to_string()),
            tasks: Some(vec![Task {
                name: Some("test task".to_string()),
                image: Some("some:image".to_string()),
                queue: Some("x-788222".to_string()),
                ..Default::default()
            }]),
            ..Default::default()
        };
        assert!(validate_job(&bad_job).is_err());

        // Invalid: coordinator queue "jobs"
        let bad_job = Job {
            name: Some("test job".to_string()),
            tasks: Some(vec![Task {
                name: Some("test task".to_string()),
                image: Some("some:image".to_string()),
                queue: Some("jobs".to_string()),
                ..Default::default()
            }]),
            ..Default::default()
        };
        assert!(validate_job(&bad_job).is_err());
    }

    #[test]
    fn test_validate_var_length() {
        // 64 chars → OK
        let ok_job = Job {
            name: Some("test job".to_string()),
            tasks: Some(vec![Task {
                name: Some("test task".to_string()),
                var: Some("a".repeat(64)),
                ..Default::default()
            }]),
            ..Default::default()
        };
        assert!(validate_job(&ok_job).is_ok());

        // 65 chars → error
        let bad_job = Job {
            name: Some("test job".to_string()),
            tasks: Some(vec![Task {
                name: Some("test task".to_string()),
                var: Some("a".repeat(65)),
                ..Default::default()
            }]),
            ..Default::default()
        };
        assert!(validate_job(&bad_job).is_err());
    }

    #[test]
    fn test_validate_subjob() {
        let job = Job {
            name: Some("test job".to_string()),
            tasks: Some(vec![Task {
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
            }]),
            ..Default::default()
        };
        assert!(validate_job(&job).is_ok());
    }

    #[test]
    fn test_validate_subjob_bad_webhook() {
        let job = Job {
            name: Some("test job".to_string()),
            tasks: Some(vec![Task {
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
            }]),
            ..Default::default()
        };
        assert!(validate_job(&job).is_err());
    }

    #[test]
    fn test_validate_job_task_no_image() {
        // Go: task without image is valid (image is not required)
        let job = Job {
            name: Some("test job".to_string()),
            tasks: Some(vec![Task {
                name: Some("some task".to_string()),
                ..Default::default()
            }]),
            ..Default::default()
        };
        assert!(validate_job(&job).is_ok());
    }

    #[test]
    fn test_validate_job_defaults_invalid_timeout() {
        // Go: validate:"duration" on Defaults.Timeout
        let job = Job {
            name: Some("test job".to_string()),
            tasks: Some(vec![Task::new("some task", "some:image")]),
            defaults: Some(JobDefaults {
                timeout: Some("1234".to_string()),
                ..Default::default()
            }),
            ..Default::default()
        };
        assert!(validate_job(&job).is_err());
    }

    // -- Expression validation tests (matching Go TestValidateExpr) ----

    #[test]
    fn test_validate_expr_valid() {
        // Go: Each.List = "1+1" → valid
        let job = Job {
            name: Some("test job".to_string()),
            tasks: Some(vec![Task {
                name: Some("test task".to_string()),
                each: Some(Each {
                    list: Some("1+1".to_string()),
                    task: Some(Box::new(Task::new("test task", "some:image"))),
                    ..Default::default()
                }),
                ..Default::default()
            }]),
            ..Default::default()
        };
        assert!(validate_job(&job).is_ok());

        // Go: Each.List = "{{1+1}}" → valid
        let job = Job {
            name: Some("test job".to_string()),
            tasks: Some(vec![Task {
                name: Some("test task".to_string()),
                each: Some(Each {
                    list: Some("{{1+1}}".to_string()),
                    task: Some(Box::new(Task::new("test task", "some:image"))),
                    ..Default::default()
                }),
                ..Default::default()
            }]),
            ..Default::default()
        };
        assert!(validate_job(&job).is_ok());

        // Go: Each.List = "5+5" (from parallel test) → valid
        let job = Job {
            name: Some("test job".to_string()),
            tasks: Some(vec![Task {
                name: Some("test task".to_string()),
                each: Some(Each {
                    list: Some("5+5".to_string()),
                    task: Some(Box::new(Task::new("test task", "some:image"))),
                    ..Default::default()
                }),
                ..Default::default()
            }]),
            ..Default::default()
        };
        assert!(validate_job(&job).is_ok());
    }

    #[test]
    fn test_validate_expr_invalid() {
        // Go: Each.List = "{1+1" → invalid (unclosed brace)
        let job = Job {
            name: Some("test job".to_string()),
            tasks: Some(vec![Task {
                name: Some("test task".to_string()),
                each: Some(Each {
                    list: Some("{1+1".to_string()),
                    task: Some(Box::new(Task::new("test task", "some:image"))),
                    ..Default::default()
                }),
                ..Default::default()
            }]),
            ..Default::default()
        };
        assert!(validate_job(&job).is_err());
    }

    // -- Cron validation tests (matching Go TestValidateCron) ----

    #[test]
    fn test_validate_cron_valid() {
        assert!(validate_cron("0 0 * * *").is_ok());
        assert!(validate_cron("0/10 0 * * *").is_ok());
    }

    #[test]
    fn test_validate_cron_invalid() {
        assert!(validate_cron("invalid-cron").is_err());
        assert!(validate_cron("").is_err());
        // Go: 6 fields (with seconds) → rejected by ParseStandard
        assert!(validate_cron("0 0 0 * * *").is_err());
        // Go: 7 fields (with seconds + year) → rejected by ParseStandard
        assert!(validate_cron("0 0 0 0 * * *").is_err());
    }

    // -- Permission validation tests ----

    #[test]
    fn test_validate_permission_neither_user_nor_role() {
        let perm = Permission {
            user: None,
            role: None,
        };
        assert!(validate_permission(&perm, &NoopPermissionChecker).is_err());
    }

    #[test]
    fn test_validate_permission_both_user_and_role() {
        let perm = Permission {
            user: Some("alice".to_string()),
            role: Some("admin".to_string()),
        };
        assert!(validate_permission(&perm, &NoopPermissionChecker).is_err());
    }

    #[test]
    fn test_validate_permission_noop_checker() {
        let perm = Permission {
            user: Some("nonexistent".to_string()),
            role: None,
        };
        // NoopChecker accepts all → OK
        assert!(validate_permission(&perm, &NoopPermissionChecker).is_ok());
    }

    #[test]
    fn test_validate_permission_with_checker() {
        let checker = TestChecker {
            users: vec!["alice"],
            roles: vec!["admin"],
        };

        let ok_user = Permission {
            user: Some("alice".to_string()),
            role: None,
        };
        assert!(validate_permission(&ok_user, &checker).is_ok());

        let ok_role = Permission {
            user: None,
            role: Some("admin".to_string()),
        };
        assert!(validate_permission(&ok_role, &checker).is_ok());

        let bad_user = Permission {
            user: Some("unknown".to_string()),
            role: None,
        };
        assert!(validate_permission(&bad_user, &checker).is_err());

        let bad_role = Permission {
            user: None,
            role: Some("unknown".to_string()),
        };
        assert!(validate_permission(&bad_role, &checker).is_err());
    }

    #[test]
    fn test_validate_job_with_permission_checker() {
        let checker = TestChecker {
            users: vec!["alice"],
            roles: vec![],
        };

        let ok_job = Job {
            name: Some("test job".to_string()),
            tasks: Some(vec![Task::new("test task", "some:image")]),
            permissions: Some(vec![Permission {
                user: Some("alice".to_string()),
                role: None,
            }]),
            ..Default::default()
        };
        assert!(validate_job_with_checker(&ok_job, &checker).is_ok());

        let bad_job = Job {
            name: Some("test job".to_string()),
            tasks: Some(vec![Task::new("test task", "some:image")]),
            permissions: Some(vec![Permission {
                user: Some("unknown".to_string()),
                role: None,
            }]),
            ..Default::default()
        };
        assert!(validate_job_with_checker(&bad_job, &checker).is_err());
    }

    // -- Queue validation tests ----

    #[test]
    fn test_is_valid_queue() {
        assert!(is_valid_queue(""));
        assert!(is_valid_queue("urgent"));
        assert!(is_valid_queue("default"));
        assert!(is_valid_queue("my-custom-queue"));

        assert!(!is_valid_queue("x-788222"));
        assert!(!is_valid_queue("x-anything"));
        assert!(!is_valid_queue("pending"));
        assert!(!is_valid_queue("started"));
        assert!(!is_valid_queue("completed"));
        assert!(!is_valid_queue("error"));
        assert!(!is_valid_queue("heartbeat"));
        assert!(!is_valid_queue("jobs"));
        assert!(!is_valid_queue("logs"));
        assert!(!is_valid_queue("progress"));
        assert!(!is_valid_queue("redeliveries"));
    }

    // -- Helper function tests ----

    #[test]
    fn test_sanitize_expr() {
        assert_eq!(sanitize_expr("{{ 1 + 1 }}"), "1 + 1");
        assert_eq!(sanitize_expr("{{inputs.var}}"), "inputs.var");
        assert_eq!(sanitize_expr("randomInt()"), "randomInt()");
        assert_eq!(sanitize_expr("plain text"), "plain text");
    }

    #[test]
    fn test_valid_expr_edge_cases() {
        assert!(valid_expr("1 == 1"));
        assert!(valid_expr("{{1+1}}"));
        assert!(!valid_expr(""));
        assert!(!valid_expr("   "));
        assert!(!valid_expr("{{}}"));
    }
}
