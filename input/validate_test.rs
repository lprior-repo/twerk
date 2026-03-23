//! Tests for the input validation module.
//!
//! These tests provide 100% parity with Go's `input.validate` package tests.
//! All test cases mirror the Go test file structure and coverage.

use tork_input::job::{AutoDelete, Job, JobDefaults, Permission, ScheduledJob, Schedule, Wait, Webhook};
use tork_input::task::{AuxTask, Each, Limits, Mount, Parallel, Probe, Registry, Retry, SidecarTask, SubJob, Task};
use tork_input::validate::{
    validate_auto_delete, validate_cron, validate_each, validate_job, validate_job_with_checker,
    validate_parallel, validate_permission, validate_probe, validate_scheduled_job,
    validate_scheduled_job_with_checker, validate_subjob, validate_wait, validate_webhook,
    is_valid_queue, valid_expr, sanitize_expr, NoopPermissionChecker, PermissionChecker,
    ValidationError,
};

// ---------------------------------------------------------------------------
// TestPermissionChecker for datastore validation tests
// ---------------------------------------------------------------------------

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

// ---------------------------------------------------------------------------
// Job validation tests (matching Go TestValidate*)
// ---------------------------------------------------------------------------

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
fn test_validate_job_empty_name() {
    let job = Job {
        name: Some("   ".to_string()),
        tasks: Some(vec![Task::new("test task", "some:image")]),
        ..Default::default()
    };
    assert!(validate_job(&job).is_err());
}

#[test]
fn test_validate_job_missing_tasks() {
    let job = Job {
        name: Some("test job".to_string()),
        tasks: None,
        ..Default::default()
    };
    let result = validate_job(&job);
    assert!(result.is_err());
    match result {
        Err(ValidationError::NoTasks) => {}
        other => panic!("expected NoTasks error, got {:?}", other),
    }
}

// ---------------------------------------------------------------------------
// Retry validation tests
// ---------------------------------------------------------------------------

#[test]
fn test_validate_retry_limit_valid() {
    let job = Job {
        name: Some("test job".to_string()),
        tasks: Some(vec![Task {
            name: Some("test task".to_string()),
            image: Some("some:image".to_string()),
            retry: Some(Retry { limit: 5 }),
            ..Default::default()
        }]),
        ..Default::default()
    };
    assert!(validate_job(&job).is_ok());
}

#[test]
fn test_validate_retry_limit_zero() {
    let job = Job {
        name: Some("test job".to_string()),
        tasks: Some(vec![Task {
            name: Some("test task".to_string()),
            image: Some("some:image".to_string()),
            retry: Some(Retry { limit: 0 }),
            ..Default::default()
        }]),
        ..Default::default()
    };
    assert!(validate_job(&job).is_err());
}

#[test]
fn test_validate_retry_limit_eleven() {
    let job = Job {
        name: Some("test job".to_string()),
        tasks: Some(vec![Task {
            name: Some("test task".to_string()),
            image: Some("some:image".to_string()),
            retry: Some(Retry { limit: 11 }),
            ..Default::default()
        }]),
        ..Default::default()
    };
    assert!(validate_job(&job).is_err());
}

#[test]
fn test_validate_retry_limit_max() {
    let job = Job {
        name: Some("test job".to_string()),
        tasks: Some(vec![Task {
            name: Some("test task".to_string()),
            image: Some("some:image".to_string()),
            retry: Some(Retry { limit: 10 }),
            ..Default::default()
        }]),
        ..Default::default()
    };
    assert!(validate_job(&job).is_ok());
}

// ---------------------------------------------------------------------------
// Timeout validation tests
// ---------------------------------------------------------------------------

#[test]
fn test_validate_timeout_valid() {
    let job = Job {
        name: Some("test job".to_string()),
        tasks: Some(vec![Task {
            name: Some("test task".to_string()),
            image: Some("some:image".to_string()),
            timeout: Some("6h".to_string()),
            ..Default::default()
        }]),
        ..Default::default()
    };
    assert!(validate_job(&job).is_ok());
}

#[test]
fn test_validate_timeout_invalid() {
    let job = Job {
        name: Some("test job".to_string()),
        tasks: Some(vec![Task {
            name: Some("test task".to_string()),
            image: Some("some:image".to_string()),
            timeout: Some("1234".to_string()),
            ..Default::default()
        }]),
        ..Default::default()
    };
    assert!(validate_job(&job).is_err());
}

#[test]
fn test_validate_timeout_empty() {
    // Empty timeout is valid
    let job = Job {
        name: Some("test job".to_string()),
        tasks: Some(vec![Task {
            name: Some("test task".to_string()),
            image: Some("some:image".to_string()),
            timeout: Some("".to_string()),
            ..Default::default()
        }]),
        ..Default::default()
    };
    assert!(validate_job(&job).is_ok());
}

// ---------------------------------------------------------------------------
// Priority validation tests
// ---------------------------------------------------------------------------

#[test]
fn test_validate_priority_valid() {
    for priority in 0..=9 {
        let job = Job {
            name: Some("test job".to_string()),
            tasks: Some(vec![Task {
                name: Some("test task".to_string()),
                image: Some("some:image".to_string()),
                priority,
                ..Default::default()
            }]),
            ..Default::default()
        };
        assert!(validate_job(&job).is_ok(), "priority {} should be valid", priority);
    }
}

#[test]
fn test_validate_priority_negative() {
    let job = Job {
        name: Some("test job".to_string()),
        tasks: Some(vec![Task {
            name: Some("test task".to_string()),
            image: Some("some:image".to_string()),
            priority: -1,
            ..Default::default()
        }]),
        ..Default::default()
    };
    assert!(validate_job(&job).is_err());
}

#[test]
fn test_validate_priority_too_high() {
    let job = Job {
        name: Some("test job".to_string()),
        tasks: Some(vec![Task {
            name: Some("test task".to_string()),
            image: Some("some:image".to_string()),
            priority: 10,
            ..Default::default()
        }]),
        ..Default::default()
    };
    assert!(validate_job(&job).is_err());
}

// ---------------------------------------------------------------------------
// Var length validation tests
// ---------------------------------------------------------------------------

#[test]
fn test_validate_var_length_valid() {
    // 64 chars → OK
    let job = Job {
        name: Some("test job".to_string()),
        tasks: Some(vec![Task {
            name: Some("test task".to_string()),
            var: Some("a".repeat(64)),
            ..Default::default()
        }]),
        ..Default::default()
    };
    assert!(validate_job(&job).is_ok());
}

#[test]
fn test_validate_var_length_too_long() {
    // 65 chars → error
    let job = Job {
        name: Some("test job".to_string()),
        tasks: Some(vec![Task {
            name: Some("test task".to_string()),
            var: Some("a".repeat(65)),
            ..Default::default()
        }]),
        ..Default::default()
    };
    assert!(validate_job(&job).is_err());
}

#[test]
fn test_validate_var_length_empty() {
    // Empty var is valid
    let job = Job {
        name: Some("test job".to_string()),
        tasks: Some(vec![Task {
            name: Some("test task".to_string()),
            var: Some("".to_string()),
            ..Default::default()
        }]),
        ..Default::default()
    };
    assert!(validate_job(&job).is_ok());
}

// ---------------------------------------------------------------------------
// Workdir length validation tests
// ---------------------------------------------------------------------------

#[test]
fn test_validate_workdir_length_valid() {
    // 256 chars → OK
    let job = Job {
        name: Some("test job".to_string()),
        tasks: Some(vec![Task {
            name: Some("test task".to_string()),
            workdir: Some("/".to_string() + &"a".repeat(255)),
            ..Default::default()
        }]),
        ..Default::default()
    };
    assert!(validate_job(&job).is_ok());
}

#[test]
fn test_validate_workdir_length_too_long() {
    // 257 chars → error
    let job = Job {
        name: Some("test job".to_string()),
        tasks: Some(vec![Task {
            name: Some("test task".to_string()),
            workdir: Some("/".to_string() + &"a".repeat(256)),
            ..Default::default()
        }]),
        ..Default::default()
    };
    assert!(validate_job(&job).is_err());
}

// ---------------------------------------------------------------------------
// Task type conflict tests (matching Go taskTypeValidation)
// ---------------------------------------------------------------------------

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
fn test_validate_parallel_and_subjob_conflict() {
    let job = Job {
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
    assert!(validate_job(&job).is_err());
}

#[test]
fn test_validate_each_and_subjob_conflict() {
    let job = Job {
        name: Some("test job".to_string()),
        tasks: Some(vec![Task {
            name: Some("test task".to_string()),
            each: Some(Each {
                list: Some("1+1".to_string()),
                task: Some(Box::new(Task::new("inner", "image"))),
                ..Default::default()
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
    assert!(validate_job(&job).is_err());
}

#[test]
fn test_validate_all_three_conflict() {
    let job = Job {
        name: Some("test job".to_string()),
        tasks: Some(vec![Task {
            name: Some("test task".to_string()),
            parallel: Some(Parallel {
                tasks: Some(vec![Task::new("inner", "image")]),
            }),
            each: Some(Each {
                list: Some("1+1".to_string()),
                task: Some(Box::new(Task::new("inner", "image"))),
                ..Default::default()
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
    assert!(validate_job(&job).is_err());
}

// ---------------------------------------------------------------------------
// Composite task validation tests (matching Go compositeTaskValidation)
// ---------------------------------------------------------------------------

#[test]
fn test_validate_composite_task_no_image() {
    let job = Job {
        name: Some("test job".to_string()),
        tasks: Some(vec![Task {
            name: Some("test task".to_string()),
            image: Some("some:image".to_string()),
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
fn test_validate_composite_task_no_cmd() {
    let job = Job {
        name: Some("test job".to_string()),
        tasks: Some(vec![Task {
            name: Some("test task".to_string()),
            cmd: Some("some command".to_string()),
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
fn test_validate_composite_task_no_entrypoint() {
    let job = Job {
        name: Some("test job".to_string()),
        tasks: Some(vec![Task {
            name: Some("test task".to_string()),
            entrypoint: Some(" Entrypoint ".to_string()),
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
fn test_validate_composite_task_no_run() {
    let job = Job {
        name: Some("test job".to_string()),
        tasks: Some(vec![Task {
            name: Some("test task".to_string()),
            run: Some("some script".to_string()),
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
fn test_validate_composite_task_no_env() {
    let job = Job {
        name: Some("test job".to_string()),
        tasks: Some(vec![Task {
            name: Some("test task".to_string()),
            env: Some(std::collections::HashMap::from([("FOO".to_string(), "bar".to_string())])),
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
fn test_validate_composite_task_no_queue() {
    let job = Job {
        name: Some("test job".to_string()),
        tasks: Some(vec![Task {
            name: Some("test task".to_string()),
            queue: Some("urgent".to_string()),
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
fn test_validate_composite_task_no_pre() {
    let job = Job {
        name: Some("test job".to_string()),
        tasks: Some(vec![Task {
            name: Some("test task".to_string()),
            pre: Some(vec![AuxTask {
                name: Some("pre task".to_string()),
                ..Default::default()
            }]),
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
fn test_validate_composite_task_no_post() {
    let job = Job {
        name: Some("test job".to_string()),
        tasks: Some(vec![Task {
            name: Some("test task".to_string()),
            post: Some(vec![AuxTask {
                name: Some("post task".to_string()),
                ..Default::default()
            }]),
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
fn test_validate_composite_task_no_mounts() {
    let job = Job {
        name: Some("test job".to_string()),
        tasks: Some(vec![Task {
            name: Some("test task".to_string()),
            mounts: Some(vec![Mount {
                mount_type: Some("volume".to_string()),
                target: Some("/some/target".to_string()),
                ..Default::default()
            }]),
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
fn test_validate_composite_task_no_retry() {
    let job = Job {
        name: Some("test job".to_string()),
        tasks: Some(vec![Task {
            name: Some("test task".to_string()),
            retry: Some(Retry { limit: 3 }),
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
fn test_validate_composite_task_no_limits() {
    let job = Job {
        name: Some("test job".to_string()),
        tasks: Some(vec![Task {
            name: Some("test task".to_string()),
            limits: Some(Limits {
                memory: Some("1Gi".to_string()),
                cpu: Some("1.0".to_string()),
                gpu: Some("1".to_string()),
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
fn test_validate_composite_task_no_timeout() {
    let job = Job {
        name: Some("test job".to_string()),
        tasks: Some(vec![Task {
            name: Some("test task".to_string()),
            timeout: Some("1h".to_string()),
            parallel: Some(Parallel {
                tasks: Some(vec![Task::new("inner", "image")]),
            }),
            ..Default::default()
        }]),
        ..Default::default()
    };
    assert!(validate_job(&job).is_err());
}

// ---------------------------------------------------------------------------
// Mount validation tests (matching Go validateMount)
// ---------------------------------------------------------------------------

#[test]
fn test_validate_mount_missing_type() {
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
}

#[test]
fn test_validate_mount_volume_with_source() {
    let job = Job {
        name: Some("test job".to_string()),
        tasks: Some(vec![Task {
            name: Some("test task".to_string()),
            image: Some("some:image".to_string()),
            run: Some("some script".to_string()),
            mounts: Some(vec![Mount {
                mount_type: Some("volume".to_string()),
                source: Some("/some/source".to_string()),
                target: Some("/some/target".to_string()),
                ..Default::default()
            }]),
            ..Default::default()
        }]),
        ..Default::default()
    };
    assert!(validate_job(&job).is_err());
}

#[test]
fn test_validate_mount_volume_missing_target() {
    let job = Job {
        name: Some("test job".to_string()),
        tasks: Some(vec![Task {
            name: Some("test task".to_string()),
            image: Some("some:image".to_string()),
            run: Some("some script".to_string()),
            mounts: Some(vec![Mount {
                mount_type: Some("volume".to_string()),
                source: None,
                target: None,
                ..Default::default()
            }]),
            ..Default::default()
        }]),
        ..Default::default()
    };
    assert!(validate_job(&job).is_err());
}

#[test]
fn test_validate_mount_volume_valid() {
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
}

#[test]
fn test_validate_mount_bind_missing_source() {
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
}

#[test]
fn test_validate_mount_bind_valid() {
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
}

#[test]
fn test_validate_mount_invalid_source_pattern() {
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
}

#[test]
fn test_validate_mount_invalid_target_pattern() {
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
}

#[test]
fn test_validate_mount_reserved_target() {
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
}

#[test]
fn test_validate_mount_valid_patterns() {
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

    // Valid pattern with numbers, dots, hyphens
    let job = Job {
        name: Some("test job".to_string()),
        tasks: Some(vec![Task {
            name: Some("test task".to_string()),
            image: Some("some:image".to_string()),
            run: Some("some script".to_string()),
            mounts: Some(vec![Mount {
                mount_type: Some("bind".to_string()),
                source: Some("/var/data/app-1.0.0/config.json".to_string()),
                target: Some("/app/config.json".to_string()),
                ..Default::default()
            }]),
            ..Default::default()
        }]),
        ..Default::default()
    };
    assert!(validate_job(&job).is_ok());
}

#[test]
fn test_validate_mount_custom_type() {
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
}

// ---------------------------------------------------------------------------
// Webhook validation tests (matching Go validateWebhook)
// ---------------------------------------------------------------------------

#[test]
fn test_validate_webhook_valid() {
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
}

#[test]
fn test_validate_webhook_empty_url() {
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
fn test_validate_webhook_whitespace_url() {
    let job = Job {
        name: Some("test job".to_string()),
        webhooks: Some(vec![Webhook {
            url: Some("   ".to_string()),
            ..Default::default()
        }]),
        tasks: Some(vec![Task::new("test task", "some:image")]),
        ..Default::default()
    };
    assert!(validate_job(&job).is_err());
}

#[test]
fn test_validate_webhook_invalid_expr() {
    let job = Job {
        name: Some("test job".to_string()),
        webhooks: Some(vec![Webhook {
            url: Some("http://example.com".to_string()),
            r#if: Some("{invalid expr".to_string()),
            ..Default::default()
        }]),
        tasks: Some(vec![Task::new("test task", "some:image")]),
        ..Default::default()
    };
    assert!(validate_job(&job).is_err());
}

#[test]
fn test_validate_webhook_valid_expr() {
    let job = Job {
        name: Some("test job".to_string()),
        webhooks: Some(vec![Webhook {
            url: Some("http://example.com".to_string()),
            r#if: Some("{{1+1}}".to_string()),
            ..Default::default()
        }]),
        tasks: Some(vec![Task::new("test task", "some:image")]),
        ..Default::default()
    };
    assert!(validate_job(&job).is_ok());
}

// ---------------------------------------------------------------------------
// Queue validation tests (matching Go validateQueue)
// ---------------------------------------------------------------------------

#[test]
fn test_validate_queue_valid() {
    // Valid custom queue
    let job = Job {
        name: Some("test job".to_string()),
        tasks: Some(vec![Task {
            name: Some("test task".to_string()),
            image: Some("some:image".to_string()),
            queue: Some("urgent".to_string()),
            ..Default::default()
        }]),
        ..Default::default()
    };
    assert!(validate_job(&job).is_ok());
}

#[test]
fn test_validate_queue_exclusive_prefix() {
    // Invalid: exclusive prefix "x-"
    let job = Job {
        name: Some("test job".to_string()),
        tasks: Some(vec![Task {
            name: Some("test task".to_string()),
            image: Some("some:image".to_string()),
            queue: Some("x-788222".to_string()),
            ..Default::default()
        }]),
        ..Default::default()
    };
    assert!(validate_job(&job).is_err());
}

#[test]
fn test_validate_queue_coordinator_name() {
    // Invalid: coordinator queue "jobs"
    let job = Job {
        name: Some("test job".to_string()),
        tasks: Some(vec![Task {
            name: Some("test task".to_string()),
            image: Some("some:image".to_string()),
            queue: Some("jobs".to_string()),
            ..Default::default()
        }]),
        ..Default::default()
    };
    assert!(validate_job(&job).is_err());
}

#[test]
fn test_validate_queue_all_coordinator_queues() {
    let coordinator_queues = [
        "pending", "started", "completed", "error", "heartbeat", "jobs", "logs", "progress",
        "redeliveries",
    ];
    for queue_name in coordinator_queues {
        let job = Job {
            name: Some("test job".to_string()),
            tasks: Some(vec![Task {
                name: Some("test task".to_string()),
                image: Some("some:image".to_string()),
                queue: Some(queue_name.to_string()),
                ..Default::default()
            }]),
            ..Default::default()
        };
        assert!(validate_job(&job).is_err(), "queue '{}' should be invalid", queue_name);
    }
}

#[test]
fn test_validate_queue_empty() {
    // Empty queue is valid
    let job = Job {
        name: Some("test job".to_string()),
        tasks: Some(vec![Task {
            name: Some("test task".to_string()),
            image: Some("some:image".to_string()),
            queue: Some("".to_string()),
            ..Default::default()
        }]),
        ..Default::default()
    };
    assert!(validate_job(&job).is_ok());
}

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

// ---------------------------------------------------------------------------
// Subjob validation tests
// ---------------------------------------------------------------------------

#[test]
fn test_validate_subjob_valid() {
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
fn test_validate_subjob_no_name() {
    let job = Job {
        name: Some("test job".to_string()),
        tasks: Some(vec![Task {
            name: Some("test task".to_string()),
            subjob: Some(SubJob {
                name: None,
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
fn test_validate_subjob_nested_tasks() {
    let job = Job {
        name: Some("test job".to_string()),
        tasks: Some(vec![Task {
            name: Some("test task".to_string()),
            subjob: Some(SubJob {
                name: Some("test sub job".to_string()),
                tasks: Some(vec![
                    Task {
                        name: Some("nested task 1".to_string()),
                        image: Some("some:image".to_string()),
                        ..Default::default()
                    },
                    Task {
                        name: Some("nested task 2".to_string()),
                        image: Some("other:image".to_string()),
                        ..Default::default()
                    },
                ]),
                ..Default::default()
            }),
            ..Default::default()
        }]),
        ..Default::default()
    };
    assert!(validate_job(&job).is_ok());
}

// ---------------------------------------------------------------------------
// Parallel validation tests
// ---------------------------------------------------------------------------

#[test]
fn test_validate_parallel_valid() {
    let job = Job {
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
    assert!(validate_job(&job).is_ok());
}

#[test]
fn test_validate_parallel_no_tasks() {
    let job = Job {
        name: Some("test job".to_string()),
        tasks: Some(vec![Task {
            name: Some("test task".to_string()),
            parallel: Some(Parallel { tasks: None }),
            ..Default::default()
        }]),
        ..Default::default()
    };
    assert!(validate_job(&job).is_err());
}

#[test]
fn test_validate_parallel_empty_tasks() {
    let job = Job {
        name: Some("test job".to_string()),
        tasks: Some(vec![Task {
            name: Some("test task".to_string()),
            parallel: Some(Parallel { tasks: Some(vec![]) }),
            ..Default::default()
        }]),
        ..Default::default()
    };
    assert!(validate_job(&job).is_err());
}

#[test]
fn test_validate_parallel_multiple_tasks() {
    let job = Job {
        name: Some("test job".to_string()),
        tasks: Some(vec![Task {
            name: Some("test task".to_string()),
            parallel: Some(Parallel {
                tasks: Some(vec![
                    Task::new("task 1", "image1"),
                    Task::new("task 2", "image2"),
                    Task::new("task 3", "image3"),
                ]),
            }),
            ..Default::default()
        }]),
        ..Default::default()
    };
    assert!(validate_job(&job).is_ok());
}

// ---------------------------------------------------------------------------
// Each validation tests
// ---------------------------------------------------------------------------

#[test]
fn test_validate_each_valid() {
    let job = Job {
        name: Some("test job".to_string()),
        tasks: Some(vec![Task {
            name: Some("test task".to_string()),
            each: Some(Each {
                list: Some("1+1".to_string()),
                task: Some(Box::new(Task::new("inner", "image"))),
                ..Default::default()
            }),
            ..Default::default()
        }]),
        ..Default::default()
    };
    assert!(validate_job(&job).is_ok());
}

#[test]
fn test_validate_each_no_list() {
    let job = Job {
        name: Some("test job".to_string()),
        tasks: Some(vec![Task {
            name: Some("test task".to_string()),
            each: Some(Each {
                list: None,
                task: Some(Box::new(Task::new("inner", "image"))),
                ..Default::default()
            }),
            ..Default::default()
        }]),
        ..Default::default()
    };
    assert!(validate_job(&job).is_err());
}

#[test]
fn test_validate_each_empty_list() {
    let job = Job {
        name: Some("test job".to_string()),
        tasks: Some(vec![Task {
            name: Some("test task".to_string()),
            each: Some(Each {
                list: Some("".to_string()),
                task: Some(Box::new(Task::new("inner", "image"))),
                ..Default::default()
            }),
            ..Default::default()
        }]),
        ..Default::default()
    };
    assert!(validate_job(&job).is_err());
}

#[test]
fn test_validate_each_no_task() {
    let job = Job {
        name: Some("test job".to_string()),
        tasks: Some(vec![Task {
            name: Some("test task".to_string()),
            each: Some(Each {
                list: Some("1+1".to_string()),
                task: None,
                ..Default::default()
            }),
            ..Default::default()
        }]),
        ..Default::default()
    };
    assert!(validate_job(&job).is_err());
}

#[test]
fn test_validate_each_invalid_list_expr() {
    let job = Job {
        name: Some("test job".to_string()),
        tasks: Some(vec![Task {
            name: Some("test task".to_string()),
            each: Some(Each {
                list: Some("{1+1".to_string()),
                task: Some(Box::new(Task::new("inner", "image"))),
                ..Default::default()
            }),
            ..Default::default()
        }]),
        ..Default::default()
    };
    assert!(validate_job(&job).is_err());
}

#[test]
fn test_validate_each_concurrency_valid() {
    for concurrency in [0, 1, 50, 99999] {
        let job = Job {
            name: Some("test job".to_string()),
            tasks: Some(vec![Task {
                name: Some("test task".to_string()),
                each: Some(Each {
                    list: Some("items".to_string()),
                    task: Some(Box::new(Task::new("inner", "image"))),
                    concurrency,
                    ..Default::default()
                }),
                ..Default::default()
            }]),
            ..Default::default()
        };
        assert!(validate_job(&job).is_ok(), "concurrency {} should be valid", concurrency);
    }
}

#[test]
fn test_validate_each_concurrency_too_high() {
    let job = Job {
        name: Some("test job".to_string()),
        tasks: Some(vec![Task {
            name: Some("test task".to_string()),
            each: Some(Each {
                list: Some("items".to_string()),
                task: Some(Box::new(Task::new("inner", "image"))),
                concurrency: 100000,
                ..Default::default()
            }),
            ..Default::default()
        }]),
        ..Default::default()
    };
    assert!(validate_job(&job).is_err());
}

// ---------------------------------------------------------------------------
// Job defaults validation tests
// ---------------------------------------------------------------------------

#[test]
fn test_validate_job_defaults_valid() {
    let job = Job {
        name: Some("test job".to_string()),
        tasks: Some(vec![Task::new("some task", "some:image")]),
        defaults: Some(JobDefaults {
            timeout: Some("1h".to_string()),
            queue: Some("default".to_string()),
            priority: 5,
            ..Default::default()
        }),
        ..Default::default()
    };
    assert!(validate_job(&job).is_ok());
}

#[test]
fn test_validate_job_defaults_invalid_timeout() {
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

#[test]
fn test_validate_job_defaults_invalid_queue() {
    let job = Job {
        name: Some("test job".to_string()),
        tasks: Some(vec![Task::new("some task", "some:image")]),
        defaults: Some(JobDefaults {
            queue: Some("x-invalid".to_string()),
            ..Default::default()
        }),
        ..Default::default()
    };
    assert!(validate_job(&job).is_err());
}

#[test]
fn test_validate_job_defaults_invalid_priority() {
    let job = Job {
        name: Some("test job".to_string()),
        tasks: Some(vec![Task::new("some task", "some:image")]),
        defaults: Some(JobDefaults {
            priority: 15,
            ..Default::default()
        }),
        ..Default::default()
    };
    assert!(validate_job(&job).is_err());
}

// ---------------------------------------------------------------------------
// Auto-delete validation tests
// ---------------------------------------------------------------------------

#[test]
fn test_validate_auto_delete_valid() {
    let job = Job {
        name: Some("test job".to_string()),
        tasks: Some(vec![Task::new("some task", "some:image")]),
        auto_delete: Some(AutoDelete {
            after: Some("24h".to_string()),
            ..Default::default()
        }),
        ..Default::default()
    };
    assert!(validate_job(&job).is_ok());
}

#[test]
fn test_validate_auto_delete_invalid_duration() {
    let job = Job {
        name: Some("test job".to_string()),
        tasks: Some(vec![Task::new("some task", "some:image")]),
        auto_delete: Some(AutoDelete {
            after: Some("invalid".to_string()),
            ..Default::default()
        }),
        ..Default::default()
    };
    assert!(validate_job(&job).is_err());
}

#[test]
fn test_validate_auto_delete_empty() {
    // Empty after is valid
    let job = Job {
        name: Some("test job".to_string()),
        tasks: Some(vec![Task::new("some task", "some:image")]),
        auto_delete: Some(AutoDelete {
            after: Some("".to_string()),
            ..Default::default()
        }),
        ..Default::default()
    };
    assert!(validate_job(&job).is_ok());
}

// ---------------------------------------------------------------------------
// Wait validation tests
// ---------------------------------------------------------------------------

#[test]
fn test_validate_wait_valid() {
    let job = Job {
        name: Some("test job".to_string()),
        tasks: Some(vec![Task::new("some task", "some:image")]),
        wait: Some(Wait {
            timeout: "1h".to_string(),
            ..Default::default()
        }),
        ..Default::default()
    };
    assert!(validate_job(&job).is_ok());
}

#[test]
fn test_validate_wait_empty_timeout() {
    let job = Job {
        name: Some("test job".to_string()),
        tasks: Some(vec![Task::new("some task", "some:image")]),
        wait: Some(Wait {
            timeout: "".to_string(),
            ..Default::default()
        }),
        ..Default::default()
    };
    assert!(validate_job(&job).is_err());
}

#[test]
fn test_validate_wait_invalid_duration() {
    let job = Job {
        name: Some("test job".to_string()),
        tasks: Some(vec![Task::new("some task", "some:image")]),
        wait: Some(Wait {
            timeout: "not-a-duration".to_string(),
            ..Default::default()
        }),
        ..Default::default()
    };
    assert!(validate_job(&job).is_err());
}

// ---------------------------------------------------------------------------
// Output expression validation tests
// ---------------------------------------------------------------------------

#[test]
fn test_validate_job_output_valid_expr() {
    let job = Job {
        name: Some("test job".to_string()),
        tasks: Some(vec![Task::new("some task", "some:image")]),
        output: Some("{{ result }}".to_string()),
        ..Default::default()
    };
    assert!(validate_job(&job).is_ok());
}

#[test]
fn test_validate_job_output_invalid_expr() {
    let job = Job {
        name: Some("test job".to_string()),
        tasks: Some(vec![Task::new("some task", "some:image")]),
        output: Some("{invalid".to_string()),
        ..Default::default()
    };
    assert!(validate_job(&job).is_err());
}

#[test]
fn test_validate_job_output_empty() {
    // Empty output is valid
    let job = Job {
        name: Some("test job".to_string()),
        tasks: Some(vec![Task::new("some task", "some:image")]),
        output: Some("".to_string()),
        ..Default::default()
    };
    assert!(validate_job(&job).is_ok());
}

// ---------------------------------------------------------------------------
// Task If expression validation tests
// ---------------------------------------------------------------------------

#[test]
fn test_validate_task_if_valid_expr() {
    let job = Job {
        name: Some("test job".to_string()),
        tasks: Some(vec![Task {
            name: Some("test task".to_string()),
            image: Some("some:image".to_string()),
            r#if: Some("{{ .Success }}".to_string()),
            ..Default::default()
        }]),
        ..Default::default()
    };
    assert!(validate_job(&job).is_ok());
}

#[test]
fn test_validate_task_if_invalid_expr() {
    let job = Job {
        name: Some("test job".to_string()),
        tasks: Some(vec![Task {
            name: Some("test task".to_string()),
            image: Some("some:image".to_string()),
            r#if: Some("{invalid".to_string()),
            ..Default::default()
        }]),
        ..Default::default()
    };
    assert!(validate_job(&job).is_err());
}

// ---------------------------------------------------------------------------
// Scheduled job validation tests
// ---------------------------------------------------------------------------

#[test]
fn test_validate_scheduled_job_valid() {
    let job = ScheduledJob {
        name: Some("test scheduled job".to_string()),
        tasks: Some(vec![Task::new("test task", "some:image")]),
        schedule: Some(Schedule {
            cron: "0 0 * * *".to_string(),
            ..Default::default()
        }),
        ..Default::default()
    };
    assert!(validate_scheduled_job(&job).is_ok());
}

#[test]
fn test_validate_scheduled_job_no_schedule() {
    let job = ScheduledJob {
        name: Some("test scheduled job".to_string()),
        tasks: Some(vec![Task::new("test task", "some:image")]),
        schedule: None,
        ..Default::default()
    };
    assert!(validate_scheduled_job(&job).is_err());
}

#[test]
fn test_validate_scheduled_job_invalid_cron() {
    let job = ScheduledJob {
        name: Some("test scheduled job".to_string()),
        tasks: Some(vec![Task::new("test task", "some:image")]),
        schedule: Some(Schedule {
            cron: "invalid-cron".to_string(),
            ..Default::default()
        }),
        ..Default::default()
    };
    assert!(validate_scheduled_job(&job).is_err());
}

#[test]
fn test_validate_scheduled_job_empty_cron() {
    let job = ScheduledJob {
        name: Some("test scheduled job".to_string()),
        tasks: Some(vec![Task::new("test task", "some:image")]),
        schedule: Some(Schedule {
            cron: "".to_string(),
            ..Default::default()
        }),
        ..Default::default()
    };
    assert!(validate_scheduled_job(&job).is_err());
}

// ---------------------------------------------------------------------------
// Permission validation tests
// ---------------------------------------------------------------------------

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

// ---------------------------------------------------------------------------
// Probe validation tests
// ---------------------------------------------------------------------------

#[test]
fn test_validate_probe_valid() {
    let job = Job {
        name: Some("test job".to_string()),
        tasks: Some(vec![Task {
            name: Some("test task".to_string()),
            image: Some("some:image".to_string()),
            sidecars: Some(vec![SidecarTask {
                name: Some("sidecar".to_string()),
                probe: Some(Probe {
                    port: 8080,
                    path: Some("/health".to_string()),
                    timeout: Some("5s".to_string()),
                    ..Default::default()
                }),
                ..Default::default()
            }]),
            ..Default::default()
        }]),
        ..Default::default()
    };
    assert!(validate_job(&job).is_ok());
}

#[test]
fn test_validate_probe_invalid_port_zero() {
    let job = Job {
        name: Some("test job".to_string()),
        tasks: Some(vec![Task {
            name: Some("test task".to_string()),
            image: Some("some:image".to_string()),
            sidecars: Some(vec![SidecarTask {
                name: Some("sidecar".to_string()),
                probe: Some(Probe {
                    port: 0,
                    path: Some("/health".to_string()),
                    ..Default::default()
                }),
                ..Default::default()
            }]),
            ..Default::default()
        }]),
        ..Default::default()
    };
    assert!(validate_job(&job).is_err());
}

#[test]
fn test_validate_probe_invalid_port_too_high() {
    let job = Job {
        name: Some("test job".to_string()),
        tasks: Some(vec![Task {
            name: Some("test task".to_string()),
            image: Some("some:image".to_string()),
            sidecars: Some(vec![SidecarTask {
                name: Some("sidecar".to_string()),
                probe: Some(Probe {
                    port: 65536,
                    path: Some("/health".to_string()),
                    ..Default::default()
                }),
                ..Default::default()
            }]),
            ..Default::default()
        }]),
        ..Default::default()
    };
    assert!(validate_job(&job).is_err());
}

#[test]
fn test_validate_probe_missing_path() {
    let job = Job {
        name: Some("test job".to_string()),
        tasks: Some(vec![Task {
            name: Some("test task".to_string()),
            image: Some("some:image".to_string()),
            sidecars: Some(vec![SidecarTask {
                name: Some("sidecar".to_string()),
                probe: Some(Probe {
                    port: 8080,
                    path: None,
                    ..Default::default()
                }),
                ..Default::default()
            }]),
            ..Default::default()
        }]),
        ..Default::default()
    };
    assert!(validate_job(&job).is_err());
}

#[test]
fn test_validate_probe_path_too_long() {
    let job = Job {
        name: Some("test job".to_string()),
        tasks: Some(vec![Task {
            name: Some("test task".to_string()),
            image: Some("some:image".to_string()),
            sidecars: Some(vec![SidecarTask {
                name: Some("sidecar".to_string()),
                probe: Some(Probe {
                    port: 8080,
                    path: Some("a".repeat(257)),
                    ..Default::default()
                }),
                ..Default::default()
            }]),
            ..Default::default()
        }]),
        ..Default::default()
    };
    assert!(validate_job(&job).is_err());
}

#[test]
fn test_validate_probe_invalid_timeout() {
    let job = Job {
        name: Some("test job".to_string()),
        tasks: Some(vec![Task {
            name: Some("test task".to_string()),
            image: Some("some:image".to_string()),
            sidecars: Some(vec![SidecarTask {
                name: Some("sidecar".to_string()),
                probe: Some(Probe {
                    port: 8080,
                    path: Some("/health".to_string()),
                    timeout: Some("invalid".to_string()),
                    ..Default::default()
                }),
                ..Default::default()
            }]),
            ..Default::default()
        }]),
        ..Default::default()
    };
    assert!(validate_job(&job).is_err());
}

// ---------------------------------------------------------------------------
// Aux task validation tests
// ---------------------------------------------------------------------------

#[test]
fn test_validate_aux_task_valid() {
    let job = Job {
        name: Some("test job".to_string()),
        tasks: Some(vec![Task {
            name: Some("test task".to_string()),
            image: Some("some:image".to_string()),
            pre: Some(vec![AuxTask {
                name: Some("pre task".to_string()),
                ..Default::default()
            }]),
            ..Default::default()
        }]),
        ..Default::default()
    };
    assert!(validate_job(&job).is_ok());
}

#[test]
fn test_validate_aux_task_no_name() {
    let job = Job {
        name: Some("test job".to_string()),
        tasks: Some(vec![Task {
            name: Some("test task".to_string()),
            image: Some("some:image".to_string()),
            pre: Some(vec![AuxTask {
                name: None,
                ..Default::default()
            }]),
            ..Default::default()
        }]),
        ..Default::default()
    };
    assert!(validate_job(&job).is_err());
}

// ---------------------------------------------------------------------------
// Cron validation tests (matching Go TestValidateCron)
// ---------------------------------------------------------------------------

#[test]
fn test_validate_cron_valid() {
    assert!(validate_cron("0 0 * * *").is_ok());
    assert!(validate_cron("0/10 0 * * *").is_ok());
    assert!(validate_cron("*/5 * * * *").is_ok());
    assert!(validate_cron("0 0 1 * *").is_ok());
    assert!(validate_cron("0 0 * * 0").is_ok()); // Sunday
}

#[test]
fn test_validate_cron_invalid() {
    assert!(validate_cron("invalid-cron").is_err());
    assert!(validate_cron("").is_err());
    // 6 fields (with seconds) → rejected by ParseStandard
    assert!(validate_cron("0 0 0 * * *").is_err());
    // 7 fields (with seconds + year) → rejected by ParseStandard
    assert!(validate_cron("0 0 0 0 * * *").is_err());
    // Only 4 fields
    assert!(validate_cron("0 0 * *").is_err());
}

// ---------------------------------------------------------------------------
// Expression validation tests (matching Go TestValidateExpr)
// ---------------------------------------------------------------------------

#[test]
fn test_validate_expr_valid() {
    // Each.List = "1+1" → valid
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

    // Each.List = "{{1+1}}" → valid
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

    // Each.List = "5+5" → valid
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
    // Each.List = "{1+1" → invalid (unclosed brace)
    let job = Job {
        name: Some("test job".to_string()),
        tasks: Some(vec![Task {
            name: Some("test task".to_string()),
            each: Each {
                list: Some("{1+1".to_string()),
                task: Some(Box::new(Task::new("test task", "some:image"))),
                ..Default::default()
            },
            ..Default::default()
        }]),
        ..Default::default()
    };
    assert!(validate_job(&job).is_err());
}

// ---------------------------------------------------------------------------
// Helper function tests
// ---------------------------------------------------------------------------

#[test]
fn test_sanitize_expr() {
    assert_eq!(sanitize_expr("{{ 1 + 1 }}"), "1 + 1");
    assert_eq!(sanitize_expr("{{inputs.var}}"), "inputs.var");
    assert_eq!(sanitize_expr("randomInt()"), "randomInt()");
    assert_eq!(sanitize_expr("plain text"), "plain text");
    assert_eq!(sanitize_expr("{{  {{ nested }} }}"), " {{ nested }} ");
}

#[test]
fn test_valid_expr_edge_cases() {
    assert!(valid_expr("1 == 1"));
    assert!(valid_expr("{{1+1}}"));
    assert!(!valid_expr(""));
    assert!(!valid_expr("   "));
    assert!(!valid_expr("{{}}"));
    assert!(!valid_expr("{{ }}"));
}

#[test]
fn test_valid_expr_with_template() {
    assert!(valid_expr("{{ .Value }}"));
    assert!(valid_expr("{{ inputs.message }}"));
    assert!(valid_expr("{{ len(items) }}"));
}

#[test]
fn test_valid_expr_rejects_invalid() {
    assert!(!valid_expr("{"));
    assert!(!valid_expr("}"));
    assert!(!valid_expr("{{"));
    assert!(!valid_expr("}}"));
    assert!(!valid_expr("{{}");
    assert!(!valid_expr("{}}"));
}

// ---------------------------------------------------------------------------
// Task without image validation
// ---------------------------------------------------------------------------

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

// ---------------------------------------------------------------------------
// Registry validation tests
// ---------------------------------------------------------------------------

#[test]
fn test_validate_task_with_registry() {
    let job = Job {
        name: Some("test job".to_string()),
        tasks: Some(vec![Task {
            name: Some("test task".to_string()),
            image: Some("some:image".to_string()),
            registry: Some(Registry {
                username: Some("user".to_string()),
                password: Some("pass".to_string()),
                ..Default::default()
            }),
            ..Default::default()
        }]),
        ..Default::default()
    };
    assert!(validate_job(&job).is_ok());
}
