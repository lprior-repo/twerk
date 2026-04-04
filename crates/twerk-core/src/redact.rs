//! Redaction functionality for masking sensitive data.
//!
//! This module provides redaction for jobs, tasks, and log parts,
//! following the functional-rust principles with zero mutability in core logic.

use std::collections::HashMap;

pub mod tests;

const REDACTED_STR: &str = "[REDACTED]";

/// Default sensitive keys that trigger redaction when found in variable names.
const DEFAULT_KEYS: &[&str] = &["SECRET", "PASSWORD", "ACCESS_KEY"];

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
    ///
    /// # Arguments
    ///
    /// * `input` - The string to check for sensitive keys.
    ///
    /// # Returns
    ///
    /// `true` if any sensitive key is found in the input, `false` otherwise.
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
    ///
    /// # Arguments
    ///
    /// * `input` - The string to redact.
    ///
    /// # Returns
    ///
    /// A new string with all sensitive values replaced by `[REDACTED]`.
    #[must_use]
    pub fn wildcard(&self, input: &str) -> String {
        let mut result = input.to_string();

        for key in &self.keys {
            // Skip empty keys to avoid infinite loop
            if key.is_empty() {
                continue;
            }

            let upper_key = key.to_uppercase();

            // Replace all occurrences of this key (case-insensitive)
            // Keep replacing until no more matches found
            loop {
                let upper_result = result.to_uppercase();
                match upper_result.find(&upper_key) {
                    Some(pos) => {
                        result = format!(
                            "{}{}{}",
                            &result[..pos],
                            REDACTED_STR,
                            &result[pos + key.len()..]
                        );
                    }
                    None => break,
                }
            }
        }

        result
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
/// # Arguments
///
/// * `m` - The map of string key-value pairs to redact
/// * `secrets` - A map of secret names to secret values
///
/// # Returns
///
/// A new `HashMap` with sensitive values redacted
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

/// Redacts a task and all its nested tasks recursively.
///
/// # Arguments
///
/// * `task` - The task to redact (mutated in place)
/// * `secrets` - A map of secret names to secret values
#[allow(clippy::implicit_hasher)]
pub fn redact_task(task: &mut crate::task::Task, secrets: &HashMap<String, String>) {
    redact_task_internal(task, secrets);
}

/// Internal helper for recursive task redaction.
fn redact_task_internal(task: &mut crate::task::Task, secrets: &HashMap<String, String>) {
    // Redact env
    if let Some(ref mut env) = task.env {
        *env = redact_vars(env, secrets);
    }

    // Redact mounts
    if let Some(ref mut mounts) = task.mounts {
        for m in mounts {
            if let Some(ref mut opts) = m.opts {
                *opts = redact_vars(opts, secrets);
            }
        }
    }

    // Redact pre/post/sidecars
    if let Some(ref mut pre) = task.pre {
        for t in pre {
            redact_task_internal(t, secrets);
        }
    }
    if let Some(ref mut post) = task.post {
        for t in post {
            redact_task_internal(t, secrets);
        }
    }
    if let Some(ref mut sidecars) = task.sidecars {
        for t in sidecars {
            redact_task_internal(t, secrets);
        }
    }

    // Redact parallel tasks
    if let Some(ref mut parallel) = task.parallel {
        if let Some(ref mut tasks) = parallel.tasks {
            for t in tasks {
                redact_task_internal(t, secrets);
            }
        }
    }

    // Registry creds
    if let Some(ref mut registry) = task.registry {
        if registry.password.is_some() {
            registry.password = Some(REDACTED_STR.to_string());
        }
    }

    // Redact subjob
    if let Some(ref mut subjob) = task.subjob {
        if let Some(ref mut subjob_secrets) = subjob.secrets {
            for v in subjob_secrets.values_mut() {
                *v = REDACTED_STR.to_string();
            }
        }
        if let Some(ref mut webhooks) = subjob.webhooks {
            for w in webhooks {
                if let Some(ref mut headers) = w.headers {
                    *headers = redact_vars(headers, secrets);
                }
            }
        }
    }
}

/// Redacts task log parts by replacing secret values in contents.
///
/// # Arguments
///
/// * `parts` - The log parts to redact (mutated in place)
/// * `secrets` - A map of secret names to secret values
#[allow(clippy::implicit_hasher)]
pub fn redact_task_log_parts(
    parts: &mut [crate::task::TaskLogPart],
    secrets: &HashMap<String, String>,
) {
    if secrets.is_empty() {
        return;
    }
    for part in parts.iter_mut() {
        if let Some(ref mut contents) = part.contents {
            for secret_val in secrets.values() {
                if !secret_val.is_empty() {
                    *contents = contents.replace(secret_val, REDACTED_STR);
                }
            }
        }
    }
}

/// Redacts a job and all its tasks.
///
/// # Arguments
///
/// * `job` - The job to redact (mutated in place)
pub fn redact_job(job: &mut crate::job::Job) {
    let secrets = job.secrets.clone().unwrap_or_default();

    // Redact inputs
    if let Some(ref mut inputs) = job.inputs {
        *inputs = redact_vars(inputs, &secrets);
    }

    // Redact webhooks
    if let Some(ref mut webhooks) = job.webhooks {
        for w in webhooks {
            if let Some(ref mut headers) = w.headers {
                *headers = redact_vars(headers, &secrets);
            }
        }
    }

    // Redact context
    if let Some(ref mut context) = job.context {
        if let Some(ref mut inputs) = context.inputs {
            *inputs = redact_vars(inputs, &secrets);
        }
        if let Some(ref mut context_secrets) = context.secrets {
            *context_secrets = redact_vars(context_secrets, &secrets);
        }
        if let Some(ref mut tasks) = context.tasks {
            *tasks = redact_vars(tasks, &secrets);
        }
    }

    // Redact tasks
    if let Some(ref mut tasks) = job.tasks {
        for t in tasks {
            redact_task_internal(t, &secrets);
        }
    }

    // Redact execution
    if let Some(ref mut execution) = job.execution {
        for t in execution {
            redact_task_internal(t, &secrets);
        }
    }

    // Redact secrets themselves
    if let Some(ref mut job_secrets) = job.secrets {
        for v in job_secrets.values_mut() {
            *v = REDACTED_STR.to_string();
        }
    }
}
