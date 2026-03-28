use crate::job::JobDefaults;
use crate::task::Task;
use std::str::FromStr;
use std::time::Duration as StdDuration;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::task::{ParallelTask, TaskRetry};

    #[test]
    fn test_validate_cron_valid() {
        assert!(validate_cron("0 0 0 * * *").is_ok());
        assert!(validate_cron("0 */5 * * * *").is_ok());
        assert!(validate_cron("0 0 12 * * *").is_ok());
        assert!(validate_cron("0 0 0 1 * *").is_ok());
    }

    #[test]
    fn test_validate_cron_invalid() {
        assert!(validate_cron("").is_err());
        assert!(validate_cron("invalid").is_err());
        assert!(validate_cron("* * * * *").is_err());
    }

    #[test]
    fn test_validate_duration_valid() {
        assert!(validate_duration("5s").is_ok());
        assert!(validate_duration("30s").is_ok());
        assert!(validate_duration("1m").is_ok());
        assert!(validate_duration("5m").is_ok());
        assert!(validate_duration("1h").is_ok());
        assert!(validate_duration("2h").is_ok());
        assert!(validate_duration("1d").is_ok());
        assert!(validate_duration("1h30m").is_ok());
        assert!(validate_duration("1d2h30m15s").is_ok());
        assert!(validate_duration("0s").is_ok());
    }

    #[test]
    fn test_validate_duration_invalid() {
        assert!(validate_duration("").is_err());
        assert!(validate_duration("   ").is_err());
        assert!(validate_duration("abc").is_err());
        assert!(validate_duration("5x").is_err());
        assert!(validate_duration("-5s").is_err());
        assert!(validate_duration("5w").is_err());
        assert!(validate_duration("5xs").is_err());
    }

    #[test]
    fn test_validate_queue_name_valid() {
        assert!(validate_queue_name("default").is_ok());
        assert!(validate_queue_name("my-queue").is_ok());
        assert!(validate_queue_name("priority").is_ok());
        assert!(validate_queue_name("x-custom").is_ok());
    }

    #[test]
    fn test_validate_queue_name_invalid() {
        let r = validate_queue_name("x-exclusive.myqueue");
        assert!(r.is_err());
        assert!(r.unwrap_err().contains("x-exclusive"));

        let r = validate_queue_name("x-jobs");
        assert!(r.is_err());
        assert!(r.unwrap_err().contains("reserved"));
    }

    #[test]
    fn test_validate_retry_valid() {
        for i in 1..=10 {
            assert!(validate_retry(i).is_ok());
        }
    }

    #[test]
    fn test_validate_retry_invalid() {
        assert!(validate_retry(0).is_err());
        assert!(validate_retry(-1).is_err());
        assert!(validate_retry(11).is_err());
        assert!(validate_retry(100).is_err());
    }

    #[test]
    fn test_validate_priority_valid() {
        for i in 0..=9 {
            assert!(validate_priority(i).is_ok());
        }
    }

    #[test]
    fn test_validate_priority_invalid() {
        assert!(validate_priority(-1).is_err());
        assert!(validate_priority(10).is_err());
        assert!(validate_priority(100).is_err());
    }

    #[test]
    fn test_validate_job_valid() {
        let task = Task::default();
        let job_defaults = JobDefaults::default();
        assert!(validate_job(
            Some(&"test-job".to_string()),
            Some(&vec![task.clone()]),
            None,
            None
        )
        .is_ok());
        assert!(validate_job(
            Some(&"test-job".to_string()),
            Some(&vec![task]),
            Some(&job_defaults),
            None
        )
        .is_ok());
    }

    #[test]
    fn test_validate_job_missing_name() {
        let task = Task::default();
        let result = validate_job(None, Some(&vec![task.clone()]), None, None);
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .iter()
            .any(|e| e.contains("name is required")));

        let result = validate_job(Some(&"".to_string()), Some(&vec![task]), None, None);
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .iter()
            .any(|e| e.contains("name is required")));
    }

    #[test]
    fn test_validate_job_missing_tasks() {
        let result = validate_job(Some(&"test".to_string()), None, None, None);
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .iter()
            .any(|e| e.contains("at least one task")));

        let result = validate_job(Some(&"test".to_string()), Some(&vec![]), None, None);
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .iter()
            .any(|e| e.contains("at least one task")));
    }

    #[test]
    fn test_validate_job_invalid_defaults() {
        let task = Task::default();
        let mut defaults = JobDefaults::default();
        defaults.timeout = Some("invalid".to_string());
        let result = validate_job(
            Some(&"test".to_string()),
            Some(&vec![task.clone()]),
            Some(&defaults),
            None,
        );
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .iter()
            .any(|e| e.contains("invalid default timeout")));

        let mut defaults = JobDefaults::default();
        defaults.queue = Some("x-jobs".to_string());
        let result = validate_job(
            Some(&"test".to_string()),
            Some(&vec![task.clone()]),
            Some(&defaults),
            None,
        );
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .iter()
            .any(|e| e.contains("invalid default queue")));

        let mut defaults = JobDefaults::default();
        defaults.priority = 15;
        let result = validate_job(
            Some(&"test".to_string()),
            Some(&vec![task]),
            Some(&defaults),
            None,
        );
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .iter()
            .any(|e| e.contains("invalid default priority")));
    }

    #[test]
    fn test_validate_task_valid() {
        let task = Task {
            priority: 5,
            ..Default::default()
        };
        assert!(validate_task(&task).is_ok());

        let mut task = Task::default();
        task.timeout = Some("30s".to_string());
        task.queue = Some("default".to_string());
        task.retry = Some(TaskRetry {
            limit: 3,
            attempts: 0,
        });
        task.priority = 3;
        assert!(validate_task(&task).is_ok());
    }

    #[test]
    fn test_validate_task_invalid_timeout() {
        let mut task = Task::default();
        task.timeout = Some("invalid".to_string());
        let result = validate_task(&task);
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .iter()
            .any(|e| e.contains("invalid timeout")));
    }

    #[test]
    fn test_validate_task_invalid_queue() {
        let mut task = Task::default();
        task.queue = Some("x-jobs".to_string());
        let result = validate_task(&task);
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .iter()
            .any(|e| e.contains("invalid queue")));
    }

    #[test]
    fn test_validate_task_invalid_retry() {
        let mut task = Task::default();
        task.retry = Some(TaskRetry {
            limit: 15,
            attempts: 0,
        });
        let result = validate_task(&task);
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .iter()
            .any(|e| e.contains("invalid retry limit")));
    }

    #[test]
    fn test_validate_task_invalid_priority() {
        let mut task = Task::default();
        task.priority = 15;
        let result = validate_task(&task);
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .iter()
            .any(|e| e.contains("invalid priority")));
    }

    #[test]
    fn test_validate_task_parallel_empty() {
        let mut task = Task::default();
        task.parallel = Some(ParallelTask {
            tasks: Some(vec![]),
            completions: 0,
        });
        let result = validate_task(&task);
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .iter()
            .any(|e| e.contains("parallel tasks cannot be empty")));
    }

    #[test]
    fn test_validate_task_each_empty() {
        use crate::task::EachTask;
        let mut task = Task::default();
        task.each = Some(Box::new(EachTask {
            list: Some(String::new()),
            ..Default::default()
        }));
        let result = validate_task(&task);
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .iter()
            .any(|e| e.contains("each list cannot be empty")));
    }

    #[test]
    fn test_validate_task_multiple_errors() {
        let mut task = Task::default();
        task.timeout = Some("invalid".to_string());
        task.queue = Some("x-jobs".to_string());
        task.retry = Some(TaskRetry {
            limit: 15,
            attempts: 0,
        });
        task.priority = 15;
        let result = validate_task(&task);
        let errors = result.unwrap_err();
        assert!(errors.len() >= 4);
    }
}

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
