use crate::job::JobDefaults;
use crate::mount::Mount;
use crate::task::Task;
use crate::webhook::Webhook;
use std::str::FromStr;
use std::time::Duration as StdDuration;

/// Validates a cron expression.
///
/// # Arguments
/// * `cron` - The cron expression to validate
///
/// # Errors
/// Returns an error if the cron expression is invalid.
pub fn validate_cron(cron: &str) -> Result<(), String> {
    cron::Schedule::from_str(cron)
        .map(|_| ())
        .map_err(|e| format!("invalid cron expression: {e}"))
}

/// Validates a duration string (e.g., "1h30m", "30s", "2d").
///
/// # Arguments
/// * `duration` - The duration string to validate
///
/// # Errors
/// Returns an error if the duration string is invalid.
pub fn validate_duration(duration: &str) -> Result<(), String> {
    parse_go_duration(duration).map(|_| ())
}

fn parse_go_duration(s: &str) -> Result<StdDuration, String> {
    let s = s.trim();
    if s.is_empty() {
        return Err("empty duration".into());
    }

    let mut total_secs: i64 = 0;
    let mut current_num: i64 = 0;

    for c in s.chars() {
        match c {
            '0'..='9' => {
                current_num = current_num * 10 + (c as i64 - '0' as i64);
            }
            's' => {
                total_secs += current_num;
                current_num = 0;
            }
            'm' => {
                total_secs += current_num * 60;
                current_num = 0;
            }
            'h' => {
                total_secs += current_num * 3600;
                current_num = 0;
            }
            'd' => {
                total_secs += current_num * 86400;
                current_num = 0;
            }
            _ => return Err(format!("invalid duration character: {c}")),
        }
    }

    if current_num > 0 {
        total_secs += current_num;
    }

    #[allow(clippy::cast_sign_loss)]
    let secs = usize::try_from(total_secs).map_err(|_| "duration overflow")?;
    Ok(StdDuration::from_secs(secs as u64))
}

/// Validates a queue name.
///
/// # Arguments
/// * `queue` - The queue name to validate
///
/// # Errors
/// Returns an error if the queue name starts with "x-exclusive." or is "x-jobs".
pub fn validate_queue_name(queue: &str) -> Result<(), String> {
    if queue.starts_with("x-exclusive.") {
        return Err("queue cannot start with x-exclusive.".into());
    }
    if queue == "x-jobs" {
        return Err("queue x-jobs is reserved".into());
    }
    Ok(())
}

/// Validates a retry limit.
///
/// # Arguments
/// * `limit` - The retry limit to validate
///
/// # Errors
/// Returns an error if the retry limit is not between 1 and 10.
pub fn validate_retry(limit: i64) -> Result<(), String> {
    if !(1..=10).contains(&limit) {
        return Err("retry limit must be between 1 and 10".into());
    }
    Ok(())
}

/// Validates a priority value.
///
/// # Arguments
/// * `priority` - The priority to validate
///
/// # Errors
/// Returns an error if the priority is not between 0 and 9.
pub fn validate_priority(priority: i64) -> Result<(), String> {
    if !(0..=9).contains(&priority) {
        return Err("priority must be between 0 and 9".into());
    }
    Ok(())
}

/// Validates a job configuration.
///
/// # Arguments
/// * `name` - Optional job name
/// * `tasks` - Optional task list
/// * `defaults` - Optional job defaults
/// * `_output` - Optional output (unused)
///
/// # Errors
/// Returns a list of validation errors if any fields are invalid.
pub fn validate_job(
    name: Option<&String>,
    tasks: Option<&Vec<Task>>,
    defaults: Option<&JobDefaults>,
    _output: Option<&String>,
) -> Result<(), Vec<String>> {
    let mut errors = Vec::new();

    if name.as_ref().is_none_or(|s| s.trim().is_empty()) {
        errors.push("job name is required".into());
    }

    if tasks.as_ref().is_none_or(|t: &&Vec<Task>| t.is_empty()) {
        errors.push("at least one task is required".into());
    }

    if let Some(tasks) = tasks {
        for (i, task) in tasks.iter().enumerate() {
            if task.name.as_ref().is_none_or(|n| n.trim().is_empty()) {
                errors.push(format!("task at index {i} has no name"));
            }
        }
    }

    if let Some(defaults) = defaults {
        if let Some(timeout) = &defaults.timeout {
            if validate_duration(timeout).is_err() {
                errors.push(format!("invalid default timeout: {timeout}"));
            }
        }
        if let Some(queue) = &defaults.queue {
            if validate_queue_name(queue).is_err() {
                errors.push(format!("invalid default queue: {queue}"));
            }
        }
        if validate_priority(defaults.priority).is_err() {
            errors.push(format!("invalid default priority: {}", defaults.priority));
        }
    }

    if errors.is_empty() {
        Ok(())
    } else {
        Err(errors)
    }
}

/// Validates a task configuration.
///
/// # Arguments
/// * `task` - The task to validate
///
/// # Errors
/// Returns a list of validation errors if any fields are invalid.
pub fn validate_task(task: &Task) -> Result<(), Vec<String>> {
    let mut errors = Vec::new();

    if let Some(timeout) = &task.timeout {
        if validate_duration(timeout).is_err() {
            errors.push(format!("invalid timeout: {timeout}"));
        }
    }

    if let Some(queue) = &task.queue {
        if validate_queue_name(queue).is_err() {
            errors.push(format!("invalid queue: {queue}"));
        }
    }

    if let Some(retry) = &task.retry {
        if validate_retry(retry.limit).is_err() {
            errors.push(format!("invalid retry limit: {}", retry.limit));
        }
    }

    if validate_priority(task.priority).is_err() {
        errors.push(format!("invalid priority: {}", task.priority));
    }

    if let Some(parallel) = &task.parallel {
        if parallel.tasks.as_ref().is_none_or(Vec::is_empty) {
            errors.push("parallel tasks cannot be empty".into());
        }
    }
    if let Some(each) = &task.each {
        if each.list.as_ref().is_none_or(String::is_empty) {
            errors.push("each list cannot be empty".into());
        }
    }

    if let Some(var) = &task.var {
        if var.len() > 64 {
            errors.push(format!(
                "variable name exceeds 64 characters: {}",
                var.len()
            ));
        }
    }

    if let Some(each) = &task.each {
        if let Some(list) = &each.list {
            if !list.is_empty() && !crate::eval::valid_expr(list) {
                errors.push(format!("invalid expression: {list}"));
            }
        }
    }

    if task.parallel.is_some() && task.each.is_some() {
        errors.push("task cannot have both parallel and each".to_string());
    }

    if task.parallel.is_some() && task.subjob.is_some() {
        errors.push("task cannot have both parallel and subjob".to_string());
    }

    if let Some(subjob) = &task.subjob {
        if let Some(webhooks) = &subjob.webhooks {
            for webhook in webhooks {
                if webhook.url.as_ref().is_none_or(|u| u.trim().is_empty()) {
                    errors.push("webhook URL cannot be empty".to_string());
                }
            }
        }
    }

    if errors.is_empty() {
        Ok(())
    } else {
        Err(errors)
    }
}

/// Validates webhook configurations.
///
/// # Arguments
/// * `webhooks` - Optional list of webhooks
/// * `tasks` - Optional list of tasks (to check subjob webhooks)
///
/// # Errors
/// Returns a list of validation errors if any webhook URLs are empty.
pub fn validate_webhooks(
    webhooks: Option<&Vec<Webhook>>,
    tasks: Option<&Vec<Task>>,
) -> Result<(), Vec<String>> {
    let mut errors = Vec::new();

    if let Some(webhooks) = webhooks {
        for webhook in webhooks {
            if webhook.url.as_ref().is_none_or(|u| u.trim().is_empty()) {
                errors.push("webhook URL cannot be empty".to_string());
            }
        }
    }

    if let Some(tasks) = tasks {
        for task in tasks {
            if let Some(subjob) = &task.subjob {
                if let Some(subjob_webhooks) = &subjob.webhooks {
                    for webhook in subjob_webhooks {
                        if webhook.url.as_ref().is_none_or(|u| u.trim().is_empty()) {
                            errors.push("webhook URL cannot be empty".to_string());
                        }
                    }
                }
            }
        }
    }

    if errors.is_empty() {
        Ok(())
    } else {
        Err(errors)
    }
}

/// Validates mount configurations.
///
/// # Arguments
/// * `mounts` - Optional list of mounts
///
/// # Errors
/// Returns a list of validation errors if any mounts are invalid.
pub fn validate_mounts(mounts: &Option<Vec<Mount>>) -> Result<(), Vec<String>> {
    let mut errors = Vec::new();

    let Some(mounts) = mounts else {
        return Ok(());
    };

    for mount in mounts {
        if mount.mount_type.as_ref().is_some_and(String::is_empty) {
            errors.push("mount type is required".to_string());
        }

        if let Some(target) = &mount.target {
            if target.is_empty() {
                errors.push("target is required".to_string());
            } else if target.contains(':') {
                errors.push("invalid target path: cannot contain colon".to_string());
            } else if target == "/tork" {
                errors.push("target path cannot be /tork".to_string());
            }
        }

        if mount.mount_type.as_deref() == Some("bind") {
            if let Some(source) = &mount.source {
                if source.is_empty() {
                    errors.push("source is required for bind mount".to_string());
                } else if source.contains('#') {
                    errors.push("invalid source path: cannot contain hash".to_string());
                }
            } else {
                errors.push("source is required for bind mount".to_string());
            }
        }
    }

    if errors.is_empty() {
        Ok(())
    } else {
        Err(errors)
    }
}
