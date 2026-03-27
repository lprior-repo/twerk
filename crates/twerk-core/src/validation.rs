use crate::job::JobDefaults;
use crate::task::Task;
use std::str::FromStr;
use std::time::Duration as StdDuration;

pub fn validate_cron(cron: &str) -> Result<(), String> {
    cron::Schedule::from_str(cron)
        .map(|_| ())
        .map_err(|e| format!("invalid cron expression: {}", e))
}

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
            _ => return Err(format!("invalid duration character: {}", c)),
        }
    }

    if current_num > 0 {
        total_secs += current_num;
    }

    Ok(StdDuration::from_secs(total_secs as u64))
}

pub fn validate_queue_name(queue: &str) -> Result<(), String> {
    if queue.starts_with("x-exclusive.") {
        return Err("queue cannot start with x-exclusive.".into());
    }
    if queue == "x-jobs" {
        return Err("queue x-jobs is reserved".into());
    }
    Ok(())
}

pub fn validate_retry(limit: i64) -> Result<(), String> {
    if !(1..=10).contains(&limit) {
        return Err("retry limit must be between 1 and 10".into());
    }
    Ok(())
}

pub fn validate_priority(priority: i64) -> Result<(), String> {
    if !(0..=9).contains(&priority) {
        return Err("priority must be between 0 and 9".into());
    }
    Ok(())
}

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

    if let Some(defaults) = defaults {
        if let Some(timeout) = &defaults.timeout {
            if validate_duration(timeout).is_err() {
                errors.push(format!("invalid default timeout: {}", timeout));
            }
        }
        if let Some(queue) = &defaults.queue {
            if validate_queue_name(queue).is_err() {
                errors.push(format!("invalid default queue: {}", queue));
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

pub fn validate_task(task: &Task) -> Result<(), Vec<String>> {
    let mut errors = Vec::new();

    if let Some(timeout) = &task.timeout {
        if validate_duration(timeout).is_err() {
            errors.push(format!("invalid timeout: {}", timeout));
        }
    }

    if let Some(queue) = &task.queue {
        if validate_queue_name(queue).is_err() {
            errors.push(format!("invalid queue: {}", queue));
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
        if parallel.tasks.as_ref().is_none_or(|t| t.is_empty()) {
            errors.push("parallel tasks cannot be empty".into());
        }
    }
    if let Some(each) = &task.each {
        if each.list.as_ref().is_none_or(|l| l.is_empty()) {
            errors.push("each list cannot be empty".into());
        }
    }

    if errors.is_empty() {
        Ok(())
    } else {
        Err(errors)
    }
}
