//! Tests for the validation module

use twerk_core::job::JobDefaults;
use twerk_core::mount::Mount;
use twerk_core::task::{EachTask, ParallelTask, SubJobTask, Task, TaskRetry};
use twerk_core::validation::{
    validate_cron, validate_duration, validate_job, validate_mounts, validate_priority,
    validate_queue_name, validate_retry, validate_task, validate_webhooks,
};
use twerk_core::webhook::Webhook;

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
    defaults.queue = Some("x-exclusive.myqueue".to_string());
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
    task.queue = Some("x-exclusive.myqueue".to_string());
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
    task.queue = Some("x-exclusive.queue".to_string());
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
    let webhooks = vec![Webhook {
        url: Some("".to_string()),
        ..Default::default()
    }];
    let result = validate_webhooks(Some(&webhooks), None);
    assert!(result.is_err());
    assert!(result
        .unwrap_err()
        .iter()
        .any(|e| e.contains("webhook URL cannot be empty")));
}

#[test]
fn validation_webhook_passes_when_url_valid() {
    let webhooks = vec![Webhook {
        url: Some("http://example.com".to_string()),
        ..Default::default()
    }];
    let result = validate_webhooks(Some(&webhooks), None);
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
    let mounts = vec![Mount {
        mount_type: Some("".to_string()),
        target: Some("".to_string()),
        ..Default::default()
    }];
    let result = validate_mounts(&Some(mounts));
    assert!(result.is_err());
    assert!(result
        .unwrap_err()
        .iter()
        .any(|e| e.contains("mount type is required") || e.contains("target is required")));
}

#[test]
fn validation_mount_passes_when_type_custom() {
    let mounts = vec![Mount {
        mount_type: Some("custom".to_string()),
        target: Some("/some/target".to_string()),
        ..Default::default()
    }];
    assert!(validate_mounts(&Some(mounts)).is_ok());
}

#[test]
fn validation_mount_fails_when_bind_type_missing_source() {
    let mounts = vec![Mount {
        mount_type: Some("bind".to_string()),
        source: Some("".to_string()),
        target: Some("/some/target".to_string()),
        ..Default::default()
    }];
    let result = validate_mounts(&Some(mounts));
    assert!(result.is_err());
    assert!(result
        .unwrap_err()
        .iter()
        .any(|e| e.contains("source is required for bind mount")));
}

#[test]
fn validation_mount_passes_when_bind_has_source_and_target() {
    let mounts = vec![Mount {
        mount_type: Some("bind".to_string()),
        source: Some("/some/source".to_string()),
        target: Some("/some/target".to_string()),
        ..Default::default()
    }];
    assert!(validate_mounts(&Some(mounts)).is_ok());
}

#[test]
fn validation_mount_fails_when_source_contains_hash() {
    let mounts = vec![Mount {
        mount_type: Some("bind".to_string()),
        source: Some("/some#/source".to_string()),
        target: Some("/some/target".to_string()),
        ..Default::default()
    }];
    let result = validate_mounts(&Some(mounts));
    assert!(result.is_err());
    assert!(result
        .unwrap_err()
        .iter()
        .any(|e| e.contains("invalid source path")));
}

#[test]
fn validation_mount_fails_when_target_contains_colon() {
    let mounts = vec![Mount {
        mount_type: Some("bind".to_string()),
        source: Some("/some/source".to_string()),
        target: Some("/some:/target".to_string()),
        ..Default::default()
    }];
    let result = validate_mounts(&Some(mounts));
    assert!(result.is_err());
    assert!(result
        .unwrap_err()
        .iter()
        .any(|e| e.contains("invalid target path")));
}

#[test]
fn validation_mount_fails_when_target_is_tork() {
    let mounts = vec![Mount {
        mount_type: Some("bind".to_string()),
        source: Some("/some/source".to_string()),
        target: Some("/tork".to_string()),
        ..Default::default()
    }];
    let result = validate_mounts(&Some(mounts));
    assert!(result.is_err());
    assert!(result
        .unwrap_err()
        .iter()
        .any(|e| e.contains("target path cannot be /tork")));
}

#[test]
fn validation_mount_passes_when_bind_with_options() {
    let mounts = vec![Mount {
        mount_type: Some("bind".to_string()),
        source: Some("bucket=some-bucket path=/mnt/some-path".to_string()),
        target: Some("/some/path".to_string()),
        ..Default::default()
    }];
    assert!(validate_mounts(&Some(mounts)).is_ok());
}
