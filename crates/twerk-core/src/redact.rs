//! Redaction functionality for masking sensitive data.
//!
//! This module provides redaction for jobs, tasks, and log parts,
//! using pure functions with zero mutability - every function takes
//! values by-value and returns new instances.

use std::collections::HashMap;

pub mod tests;

const REDACTED_STR: &str = "[REDACTED]";

/// Default sensitive keys that trigger redaction when found in variable names.
const DEFAULT_KEYS: &[&str] = &["SECRET", "PASSWORD", "ACCESS_KEY"];

/// Replaces all case-insensitive occurrences of `key` in `input` with `[REDACTED]`
/// using recursion to iterate until no more matches exist (fixed-point iteration).
fn replace_key_until_fixed_point(input: String, upper_key: &str, key_len: usize) -> String {
    match input.to_uppercase().find(upper_key) {
        Some(pos) => replace_key_until_fixed_point(
            format!(
                "{}{}{}",
                &input[..pos],
                REDACTED_STR,
                &input[pos + key_len..]
            ),
            upper_key,
            key_len,
        ),
        None => input,
    }
}

/// Redacter masks sensitive data in jobs and tasks.
#[derive(Debug, Clone)]
pub struct Redacter {
    keys: Vec<String>,
}

impl Redacter {
    /// Creates a new Redacter with the given sensitive keys.
    ///
    /// # Arguments
    ///
    /// * `keys` - A vector of strings representing sensitive key patterns.
    ///   Keys are matched case-insensitively.
    #[must_use]
    pub fn new(keys: Vec<String>) -> Self {
        Self { keys }
    }

    /// Creates a new Redacter with default sensitive keys (SECRET, PASSWORD, `ACCESS_KEY`).
    #[must_use]
    pub fn default_redacter() -> Self {
        Self {
            keys: DEFAULT_KEYS.iter().map(|s| (*s).to_string()).collect(),
        }
    }

    /// Checks if the input string contains any sensitive key.
    ///
    /// Matching is case-insensitive.
    #[must_use]
    pub fn contains(&self, input: &str) -> bool {
        self.keys
            .iter()
            .any(|key| input.to_uppercase().contains(&key.to_uppercase()))
    }

    /// Redacts all occurrences of sensitive values in the input string.
    ///
    /// Each key is treated as a wildcard pattern. All occurrences in the input
    /// that match any key pattern are replaced with `[REDACTED]`.
    ///
    /// Matching is case-insensitive.
    #[must_use]
    pub fn wildcard(&self, input: &str) -> String {
        self.keys.iter().fold(input.to_string(), |acc, key| {
            if key.is_empty() {
                return acc;
            }
            let upper_key = key.to_uppercase();
            let key_len = key.len();
            replace_key_until_fixed_point(acc, &upper_key, key_len)
        })
    }

    /// Returns the list of sensitive keys.
    #[must_use]
    pub fn keys(&self) -> &[String] {
        &self.keys
    }
}

impl Default for Redacter {
    fn default() -> Self {
        Self::default_redacter()
    }
}

/// Checks if a key name indicates a sensitive variable.
///
/// A key is considered sensitive if it contains any of:
/// SECRET, PASSWORD, or `ACCESS_KEY` (case-insensitive).
#[must_use]
pub fn is_secret_key(key: &str) -> bool {
    let k = key.to_uppercase();
    k.contains("SECRET") || k.contains("PASSWORD") || k.contains("ACCESS_KEY")
}

/// Redacts variables in a map.
///
/// For each key-value pair:
/// - If the key is sensitive (matches SECRET, PASSWORD, `ACCESS_KEY`), the value is redacted
/// - Otherwise, all secret values are replaced with `[REDACTED]`
///
/// # Returns
///
/// A new `HashMap` with sensitive values redacted.
#[must_use]
#[allow(clippy::implicit_hasher)]
pub fn redact_vars(
    m: &HashMap<String, String>,
    secrets: &HashMap<String, String>,
) -> HashMap<String, String> {
    m.iter()
        .map(|(k, v)| {
            let redacted_value = if is_secret_key(k) {
                REDACTED_STR.to_string()
            } else {
                secrets
                    .values()
                    .filter(|sv| !sv.is_empty())
                    .fold(v.clone(), |acc, sv| acc.replace(sv, REDACTED_STR))
            };
            (k.clone(), redacted_value)
        })
        .collect()
}

// ---------------------------------------------------------------------------
// Task-level pure redaction helpers (each under 25 lines)
// ---------------------------------------------------------------------------

/// Redact environment variables on a task.
fn redact_task_env(
    task: crate::task::Task,
    secrets: &HashMap<String, String>,
) -> crate::task::Task {
    let env = task.env.as_ref().map(|e| redact_vars(e, secrets));
    crate::task::Task { env, ..task }
}

/// Redact mount options on a task.
fn redact_task_mounts(
    task: crate::task::Task,
    secrets: &HashMap<String, String>,
) -> crate::task::Task {
    let mounts = task.mounts.as_ref().map(|mounts| {
        mounts
            .iter()
            .map(|m| crate::mount::Mount {
                opts: m.opts.as_ref().map(|o| redact_vars(o, secrets)),
                ..m.clone()
            })
            .collect()
    });
    crate::task::Task { mounts, ..task }
}

/// Redact registry password on a task.
fn redact_task_registry(task: crate::task::Task) -> crate::task::Task {
    let registry = task.registry.as_ref().map(|r| crate::task::Registry {
        password: r.password.as_ref().map(|_| REDACTED_STR.to_string()),
        ..r.clone()
    });
    crate::task::Task { registry, ..task }
}

/// Redact subjob secrets and webhook headers on a task.
fn redact_task_subjob(
    task: crate::task::Task,
    secrets: &HashMap<String, String>,
) -> crate::task::Task {
    let subjob = task.subjob.as_ref().map(|sj| {
        let secrets_map = sj.secrets.as_ref().map(|s| {
            s.keys()
                .map(|k| (k.clone(), REDACTED_STR.to_string()))
                .collect()
        });
        let webhooks = sj.webhooks.as_ref().map(|whs| {
            whs.iter()
                .map(|w| crate::webhook::Webhook {
                    headers: w.headers.as_ref().map(|h| redact_vars(h, secrets)),
                    ..w.clone()
                })
                .collect()
        });
        crate::task::SubJobTask {
            secrets: secrets_map,
            webhooks,
            ..sj.clone()
        }
    });
    crate::task::Task { subjob, ..task }
}

/// Recursively redact a vector of tasks.
fn redact_nested_tasks(
    tasks: Option<Vec<crate::task::Task>>,
    secrets: &HashMap<String, String>,
) -> Option<Vec<crate::task::Task>> {
    tasks.map(|ts| ts.into_iter().map(|t| redact_task(t, secrets)).collect())
}

/// Redact parallel tasks on a task.
fn redact_task_parallel(
    task: crate::task::Task,
    secrets: &HashMap<String, String>,
) -> crate::task::Task {
    let parallel = task.parallel.as_ref().map(|p| crate::task::ParallelTask {
        tasks: redact_nested_tasks(p.tasks.clone(), secrets),
        ..p.clone()
    });
    crate::task::Task { parallel, ..task }
}

/// Redact pre, post, and sidecar nested tasks.
fn redact_task_pre_post_sidecars(
    task: crate::task::Task,
    secrets: &HashMap<String, String>,
) -> crate::task::Task {
    let pre = redact_nested_tasks(task.pre.clone(), secrets);
    let post = redact_nested_tasks(task.post.clone(), secrets);
    let sidecars = redact_nested_tasks(task.sidecars.clone(), secrets);
    crate::task::Task {
        pre,
        post,
        sidecars,
        ..task
    }
}

// ---------------------------------------------------------------------------
// Public API - pure functions, take by-value, return new instance
// ---------------------------------------------------------------------------

/// Redacts a task and all its nested tasks recursively.
///
/// Takes ownership and returns a new redacted task.
#[must_use]
#[allow(clippy::implicit_hasher)]
pub fn redact_task(
    task: crate::task::Task,
    secrets: &HashMap<String, String>,
) -> crate::task::Task {
    let task = redact_task_env(task, secrets);
    let task = redact_task_mounts(task, secrets);
    let task = redact_task_pre_post_sidecars(task, secrets);
    let task = redact_task_parallel(task, secrets);
    let task = redact_task_registry(task);
    redact_task_subjob(task, secrets)
}

/// Redacts task log parts by replacing secret values in contents.
///
/// Takes ownership of the vec and returns a new redacted vec.
#[must_use]
#[allow(clippy::implicit_hasher)]
pub fn redact_task_log_parts(
    parts: Vec<crate::task::TaskLogPart>,
    secrets: &HashMap<String, String>,
) -> Vec<crate::task::TaskLogPart> {
    if secrets.is_empty() {
        return parts;
    }
    parts
        .into_iter()
        .map(|part| {
            let contents = part.contents.as_ref().map(|c| {
                secrets
                    .values()
                    .filter(|sv| !sv.is_empty())
                    .fold(c.clone(), |acc, sv| acc.replace(sv, REDACTED_STR))
            });
            crate::task::TaskLogPart { contents, ..part }
        })
        .collect()
}

/// Redact job inputs, webhooks, context, and tasks.
fn redact_job_inputs_webhooks_context(
    job: crate::job::Job,
    secrets: &HashMap<String, String>,
) -> crate::job::Job {
    let inputs = job.inputs.as_ref().map(|i| redact_vars(i, secrets));

    let webhooks = job.webhooks.as_ref().map(|whs| {
        whs.iter()
            .map(|w| crate::webhook::Webhook {
                headers: w.headers.as_ref().map(|h| redact_vars(h, secrets)),
                ..w.clone()
            })
            .collect()
    });

    let context = job.context.as_ref().map(|ctx| crate::job::JobContext {
        inputs: ctx.inputs.as_ref().map(|i| redact_vars(i, secrets)),
        secrets: ctx.secrets.as_ref().map(|s| redact_vars(s, secrets)),
        tasks: ctx.tasks.as_ref().map(|t| redact_vars(t, secrets)),
        ..ctx.clone()
    });

    crate::job::Job {
        inputs,
        context,
        webhooks,
        ..job
    }
}

/// Redact all tasks and execution entries in a job.
fn redact_job_tasks(job: crate::job::Job, secrets: &HashMap<String, String>) -> crate::job::Job {
    let tasks = redact_nested_tasks(job.tasks, secrets);
    let execution = redact_nested_tasks(job.execution, secrets);
    crate::job::Job {
        tasks,
        execution,
        ..job
    }
}

/// Redact the job's own secrets map (values become `\[REDACTED\]`).
fn redact_job_secrets(job: crate::job::Job) -> crate::job::Job {
    let secrets = job.secrets.as_ref().map(|s| {
        s.keys()
            .map(|k| (k.clone(), REDACTED_STR.to_string()))
            .collect()
    });
    crate::job::Job { secrets, ..job }
}

/// Redacts a job and all its tasks.
///
/// Takes ownership and returns a new redacted job.
#[must_use]
pub fn redact_job(job: crate::job::Job) -> crate::job::Job {
    let secrets = job.secrets.clone().unwrap_or_default();
    let job = redact_job_inputs_webhooks_context(job, &secrets);
    let job = redact_job_tasks(job, &secrets);
    redact_job_secrets(job)
}
