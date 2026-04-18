use std::collections::HashMap;
use twerk_core::job::{Job, JobSummary, ScheduledJob, ScheduledJobSummary};
use twerk_core::task::{Task, TaskLogPart};

const REDACTED_STR: &str = "[REDACTED]";

pub fn redact_job(job: &mut Job) {
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

pub fn redact_job_summary(summary: &mut JobSummary) {
    // JobSummary doesn't have secrets field, but it has inputs
    // We don't have the job's secrets map here unless we fetch it.
    // The Go code only redacts Job/Task.
    // If JobSummary is returned, it might also need redaction if it has inputs.
    // But JobSummary.inputs usually doesn't contain secrets unless they were passed as inputs.

    // For now, let's just redact based on keys for JobSummary if needed.
    if let Some(ref mut inputs) = summary.inputs {
        *inputs = redact_vars_by_key_only(inputs);
    }
}

pub fn redact_scheduled_job(sj: &mut ScheduledJob) {
    let secrets = sj.secrets.clone().unwrap_or_default();

    if let Some(ref mut inputs) = sj.inputs {
        *inputs = redact_vars(inputs, &secrets);
    }

    if let Some(ref mut webhooks) = sj.webhooks {
        for w in webhooks {
            if let Some(ref mut headers) = w.headers {
                *headers = redact_vars(headers, &secrets);
            }
        }
    }

    if let Some(ref mut tasks) = sj.tasks {
        for t in tasks {
            redact_task_internal(t, &secrets);
        }
    }

    if let Some(ref mut sj_secrets) = sj.secrets {
        for v in sj_secrets.values_mut() {
            *v = REDACTED_STR.to_string();
        }
    }
}

pub fn redact_scheduled_job_summary(summary: &mut ScheduledJobSummary) {
    if let Some(ref mut inputs) = summary.inputs {
        *inputs = redact_vars_by_key_only(inputs);
    }
}

pub fn redact_task<S: std::hash::BuildHasher>(
    task: &mut Task,
    secrets: &HashMap<String, String, S>,
) {
    redact_task_internal(task, secrets);
}

fn redact_task_internal<S: std::hash::BuildHasher>(
    task: &mut Task,
    secrets: &HashMap<String, String, S>,
) {
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

fn is_secret_key(key: &str) -> bool {
    let k = key.to_uppercase();
    k.contains("SECRET")
        || k.contains("PASSWORD")
        || k.contains("ACCESS_KEY")
        || k.contains("TOKEN")
        || k.contains("API_KEY")
}

fn redact_vars<S: std::hash::BuildHasher>(
    m: &HashMap<String, String>,
    secrets: &HashMap<String, String, S>,
) -> HashMap<String, String> {
    let mut redacted = HashMap::new();
    for (k, v) in m {
        let mut val = v.clone();
        if is_secret_key(k) {
            val = REDACTED_STR.to_string();
        }

        // Also redact if value matches any known secret value, regardless of key
        for secret_val in secrets.values() {
            if !secret_val.is_empty() {
                val = val.replace(secret_val, REDACTED_STR);
            }
        }

        redacted.insert(k.clone(), val);
    }
    redacted
}

pub fn redact_task_log_parts<S: std::hash::BuildHasher>(
    parts: &mut [TaskLogPart],
    secrets: &HashMap<String, String, S>,
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

fn redact_vars_by_key_only(m: &HashMap<String, String>) -> HashMap<String, String> {
    let mut redacted = HashMap::new();
    for (k, v) in m {
        let val = if is_secret_key(k) {
            REDACTED_STR.to_string()
        } else {
            v.clone()
        };
        redacted.insert(k.clone(), val);
    }
    redacted
}
