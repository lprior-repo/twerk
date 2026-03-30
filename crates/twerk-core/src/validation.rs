use crate::job::JobDefaults;
use crate::mount::Mount;
use crate::task::Task;
use crate::webhook::Webhook;
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

    if let Some(tasks) = tasks {
        for (i, task) in tasks.iter().enumerate() {
            if task.name.as_ref().is_none_or(|n| n.trim().is_empty()) {
                errors.push(format!("task at index {} has no name", i));
            }
        }
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
                errors.push(format!("invalid expression: {}", list));
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

pub fn validate_mounts(mounts: &Option<Vec<Mount>>) -> Result<(), Vec<String>> {
    let mut errors = Vec::new();

    let Some(mounts) = mounts else {
        return Ok(());
    };

    for mount in mounts {
        if mount.mount_type.as_ref().is_some_and(|mt| mt.is_empty()) {
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

#[cfg(test)]
#[allow(clippy::field_reassign_with_default)]
mod tests {
    use super::*;
    use crate::task::{EachTask, ParallelTask, SubJobTask, TaskRetry};

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
        let task = Task {
            name: Some("test-task".to_string()),
            ..Default::default()
        };
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

    #[test]
    fn validation_job_passes_when_minimal_valid() {
        let task = Task {
            name: Some("test task".to_string()),
            image: Some("some:image".to_string()),
            ..Default::default()
        };
        let result = validate_job(Some(&"test job".to_string()), Some(&vec![task]), None, None);
        assert!(result.is_ok());
    }

    #[test]
    fn validation_job_fails_when_tasks_empty() {
        let result = validate_job(Some(&"test job".to_string()), Some(&vec![]), None, None);
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .iter()
            .any(|e| e.contains("at least one task")));
    }

    #[test]
    fn validation_job_fails_when_name_missing() {
        let task = Task {
            image: Some("some:image".to_string()),
            ..Default::default()
        };
        let result = validate_job(None, Some(&vec![task]), None, None);
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .iter()
            .any(|e| e.contains("name is required")));
    }

    #[test]
    fn validation_job_fails_when_name_empty() {
        let task = Task {
            image: Some("some:image".to_string()),
            ..Default::default()
        };
        let result = validate_job(Some(&"".to_string()), Some(&vec![task]), None, None);
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .iter()
            .any(|e| e.contains("name is required")));
    }

    #[test]
    fn validation_queue_passes_when_valid_name() {
        assert!(validate_queue_name("urgent").is_ok());
        assert!(validate_queue_name("default").is_ok());
        assert!(validate_queue_name("x-custom").is_ok());
    }

    #[test]
    fn validation_queue_fails_when_x_jobs() {
        let r = validate_queue_name("x-jobs");
        assert!(r.is_err());
        assert!(r.unwrap_err().contains("reserved"));
    }

    #[test]
    fn validation_task_passes_when_retry_limit_1() {
        let mut task = Task::default();
        task.retry = Some(TaskRetry {
            limit: 1,
            attempts: 0,
        });
        assert!(validate_task(&task).is_ok());
    }

    #[test]
    fn validation_task_passes_when_retry_limit_10() {
        let mut task = Task::default();
        task.retry = Some(TaskRetry {
            limit: 10,
            attempts: 0,
        });
        assert!(validate_task(&task).is_ok());
    }

    #[test]
    fn validation_task_fails_when_retry_limit_50() {
        let mut task = Task::default();
        task.retry = Some(TaskRetry {
            limit: 50,
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
    fn validation_task_passes_when_timeout_6h() {
        let mut task = Task::default();
        task.timeout = Some("6h".to_string());
        assert!(validate_task(&task).is_ok());
    }

    #[test]
    fn validation_job_task_fails_when_name_missing() {
        let task = Task {
            image: Some("some:image".to_string()),
            ..Default::default()
        };
        let result = validate_job(Some(&"test job".to_string()), Some(&vec![task]), None, None);
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .iter()
            .any(|e| e.contains("has no name")));
    }

    #[test]
    fn validation_job_task_passes_when_image_missing() {
        let task = Task {
            name: Some("some task".to_string()),
            ..Default::default()
        };
        let result = validate_job(Some(&"test job".to_string()), Some(&vec![task]), None, None);
        assert!(result.is_ok());
    }

    #[test]
    fn validation_job_defaults_fails_when_timeout_invalid() {
        let task = Task {
            name: Some("some task".to_string()),
            image: Some("some:image".to_string()),
            ..Default::default()
        };
        let mut defaults = JobDefaults::default();
        defaults.timeout = Some("invalid".to_string());
        let result = validate_job(
            Some(&"test job".to_string()),
            Some(&vec![task]),
            Some(&defaults),
            None,
        );
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .iter()
            .any(|e| e.contains("invalid default timeout")));
    }

    #[test]
    fn validation_var_fails_when_too_long() {
        let long_var = "a".repeat(65);
        let mut task = Task::default();
        task.var = Some(long_var);
        let result = validate_task(&task);
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .iter()
            .any(|e| e.contains("variable name exceeds 64 characters")));
    }

    #[test]
    fn validation_var_passes_when_64_chars() {
        let var_64 = "a".repeat(64);
        let mut task = Task::default();
        task.var = Some(var_64);
        assert!(validate_task(&task).is_ok());
    }

    #[test]
    fn validation_var_passes_when_shorter() {
        let mut task = Task::default();
        task.var = Some("somevar".to_string());
        assert!(validate_task(&task).is_ok());
    }

    #[test]
    fn validation_expr_fails_when_invalid_syntax() {
        let mut task = Task::default();
        task.each = Some(Box::new(EachTask {
            list: Some("{1+1".to_string()),
            task: Some(Box::new(Task {
                name: Some("test task".to_string()),
                image: Some("some:image".to_string()),
                ..Default::default()
            })),
            ..Default::default()
        }));
        let result = validate_task(&task);
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .iter()
            .any(|e| e.contains("invalid expression")));
    }

    #[test]
    fn validation_expr_passes_when_valid_arithmetic() {
        let mut task = Task::default();
        task.each = Some(Box::new(EachTask {
            list: Some("1+1".to_string()),
            task: Some(Box::new(Task {
                name: Some("test task".to_string()),
                image: Some("some:image".to_string()),
                ..Default::default()
            })),
            ..Default::default()
        }));
        assert!(validate_task(&task).is_ok());
    }

    #[test]
    fn validation_expr_passes_when_valid_template() {
        let mut task = Task::default();
        task.each = Some(Box::new(EachTask {
            list: Some("{{1+1}}".to_string()),
            task: Some(Box::new(Task {
                name: Some("test task".to_string()),
                image: Some("some:image".to_string()),
                ..Default::default()
            })),
            ..Default::default()
        }));
        assert!(validate_task(&task).is_ok());
    }

    #[test]
    fn validation_webhook_fails_when_url_empty() {
        let webhooks = Some(vec![Webhook {
            url: Some("".to_string()),
            ..Default::default()
        }]);
        let result = validate_webhooks(webhooks.as_ref(), None);
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .iter()
            .any(|e| e.contains("webhook URL cannot be empty")));
    }

    #[test]
    fn validation_webhook_passes_when_url_valid() {
        let webhooks = Some(vec![Webhook {
            url: Some("http://example.com".to_string()),
            ..Default::default()
        }]);
        let result = validate_webhooks(webhooks.as_ref(), None);
        assert!(result.is_ok());
    }

    #[test]
    fn validation_cron_fails_when_invalid_expression() {
        assert!(validate_cron("invalid-cron").is_err());
        assert!(validate_cron("").is_err());
    }

    #[test]
    fn validation_cron_fails_when_too_many_fields() {
        assert!(validate_cron("0 0 0 0 * * *").is_err());
    }

    #[test]
    fn validation_parallel_passes_when_single_task() {
        let mut task = Task::default();
        task.parallel = Some(ParallelTask {
            tasks: Some(vec![Task {
                name: Some("test task".to_string()),
                image: Some("some task".to_string()),
                ..Default::default()
            }]),
            completions: 0,
        });
        assert!(validate_task(&task).is_ok());
    }

    #[test]
    fn validation_each_passes_when_expression_valid() {
        let mut task = Task::default();
        task.each = Some(Box::new(EachTask {
            list: Some("5+5".to_string()),
            task: Some(Box::new(Task {
                name: Some("test task".to_string()),
                image: Some("some task".to_string()),
                ..Default::default()
            })),
            ..Default::default()
        }));
        assert!(validate_task(&task).is_ok());
    }

    #[test]
    fn validation_task_fails_when_parallel_and_each_both_set() {
        let mut task = Task::default();
        task.each = Some(Box::new(EachTask {
            list: Some("some expression".to_string()),
            task: Some(Box::new(Task {
                name: Some("test task".to_string()),
                image: Some("some task".to_string()),
                ..Default::default()
            })),
            ..Default::default()
        }));
        task.parallel = Some(ParallelTask {
            tasks: Some(vec![Task {
                name: Some("test task".to_string()),
                image: Some("some task".to_string()),
                ..Default::default()
            }]),
            completions: 0,
        });
        let result = validate_task(&task);
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .iter()
            .any(|e| e.contains("parallel") && e.contains("each")));
    }

    #[test]
    fn validation_subjob_passes_when_webhook_valid() {
        let mut task = Task::default();
        task.subjob = Some(SubJobTask {
            name: Some("test sub job".to_string()),
            webhooks: Some(vec![Webhook {
                url: Some("http://example.com".to_string()),
                ..Default::default()
            }]),
            tasks: Some(vec![Task {
                name: Some("test task".to_string()),
                image: Some("some task".to_string()),
                ..Default::default()
            }]),
            ..Default::default()
        });
        assert!(validate_task(&task).is_ok());
    }

    #[test]
    fn validation_subjob_fails_when_webhook_url_empty() {
        let mut task = Task::default();
        task.subjob = Some(SubJobTask {
            name: Some("test sub job".to_string()),
            webhooks: Some(vec![Webhook {
                url: Some("".to_string()),
                ..Default::default()
            }]),
            tasks: Some(vec![Task {
                name: Some("test task".to_string()),
                image: Some("some task".to_string()),
                ..Default::default()
            }]),
            ..Default::default()
        });
        let result = validate_task(&task);
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .iter()
            .any(|e| e.contains("webhook URL cannot be empty")));
    }

    #[test]
    fn validation_task_fails_when_parallel_and_subjob_both_set() {
        let mut task = Task::default();
        task.parallel = Some(ParallelTask {
            tasks: Some(vec![Task {
                name: Some("test task".to_string()),
                image: Some("some task".to_string()),
                ..Default::default()
            }]),
            completions: 0,
        });
        task.subjob = Some(SubJobTask {
            name: Some("test sub job".to_string()),
            tasks: Some(vec![Task {
                name: Some("test task".to_string()),
                image: Some("some task".to_string()),
                ..Default::default()
            }]),
            ..Default::default()
        });
        let result = validate_task(&task);
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .iter()
            .any(|e| e.contains("parallel") && e.contains("subjob")));
    }

    #[test]
    fn validation_mount_fails_when_type_and_target_missing() {
        let mounts = Some(vec![Mount {
            mount_type: Some("".to_string()),
            target: Some("".to_string()),
            ..Default::default()
        }]);
        let result = validate_mounts(&mounts);
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .iter()
            .any(|e| e.contains("mount type is required") || e.contains("target is required")));
    }

    #[test]
    fn validation_mount_passes_when_type_custom() {
        let mounts = Some(vec![Mount {
            mount_type: Some("custom".to_string()),
            target: Some("/some/target".to_string()),
            ..Default::default()
        }]);
        assert!(validate_mounts(&mounts).is_ok());
    }

    #[test]
    fn validation_mount_fails_when_bind_type_missing_source() {
        let mounts = Some(vec![Mount {
            mount_type: Some("bind".to_string()),
            source: Some("".to_string()),
            target: Some("/some/target".to_string()),
            ..Default::default()
        }]);
        let result = validate_mounts(&mounts);
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .iter()
            .any(|e| e.contains("source is required for bind mount")));
    }

    #[test]
    fn validation_mount_passes_when_bind_has_source_and_target() {
        let mounts = Some(vec![Mount {
            mount_type: Some("bind".to_string()),
            source: Some("/some/source".to_string()),
            target: Some("/some/target".to_string()),
            ..Default::default()
        }]);
        assert!(validate_mounts(&mounts).is_ok());
    }

    #[test]
    fn validation_mount_fails_when_source_contains_hash() {
        let mounts = Some(vec![Mount {
            mount_type: Some("bind".to_string()),
            source: Some("/some#/source".to_string()),
            target: Some("/some/target".to_string()),
            ..Default::default()
        }]);
        let result = validate_mounts(&mounts);
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .iter()
            .any(|e| e.contains("invalid source path")));
    }

    #[test]
    fn validation_mount_fails_when_target_contains_colon() {
        let mounts = Some(vec![Mount {
            mount_type: Some("bind".to_string()),
            source: Some("/some/source".to_string()),
            target: Some("/some:/target".to_string()),
            ..Default::default()
        }]);
        let result = validate_mounts(&mounts);
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .iter()
            .any(|e| e.contains("invalid target path")));
    }

    #[test]
    fn validation_mount_fails_when_target_is_tork() {
        let mounts = Some(vec![Mount {
            mount_type: Some("bind".to_string()),
            source: Some("/some/source".to_string()),
            target: Some("/tork".to_string()),
            ..Default::default()
        }]);
        let result = validate_mounts(&mounts);
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .iter()
            .any(|e| e.contains("target path cannot be /tork")));
    }

    #[test]
    fn validation_mount_passes_when_bind_with_options() {
        let mounts = Some(vec![Mount {
            mount_type: Some("bind".to_string()),
            source: Some("bucket=some-bucket path=/mnt/some-path".to_string()),
            target: Some("/some/path".to_string()),
            ..Default::default()
        }]);
        assert!(validate_mounts(&mounts).is_ok());
    }
}
