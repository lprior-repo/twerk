//! Integration tests for PostgreSQL record conversion types.
//!
//! Extracted from inline `#[cfg(test)]` blocks in:
//! - `records/helpers.rs`, `records/auth.rs`, `records/log.rs`
//! - `records/node.rs`, `records/scheduled_job.rs`
//! - `records/task.rs`, `records/job.rs`

mod common;

use common::fixed_now;
use std::collections::HashMap;
use twerk_core::id::UserId;
use twerk_core::job::{JobDefaults as CoreJobDefaults, JobState, ScheduledJobState};
use twerk_core::node::NodeStatus;
use twerk_core::node::NODE_STATUS_UP;
use twerk_core::task::Permission;
use twerk_core::task::{AutoDelete, ParallelTask, Registry, TaskLimits, TaskRetry};
use twerk_core::user::User;
use twerk_core::webhook::Webhook;
use twerk_infrastructure::datastore::postgres::records::auth::{
    RoleRecord, RoleRecordExt, UserRecord, UserRecordExt,
};
use twerk_infrastructure::datastore::postgres::records::helpers::str_to_task_state;
use twerk_infrastructure::datastore::postgres::records::job::{JobRecord, JobRecordExt};
use twerk_infrastructure::datastore::postgres::records::log::{
    TaskLogPartRecord, TaskLogPartRecordExt,
};
use twerk_infrastructure::datastore::postgres::records::node::{NodeRecord, NodeRecordExt};
use twerk_infrastructure::datastore::postgres::records::scheduled_job::{
    ScheduledJobRecord, ScheduledJobRecordExt,
};
use twerk_infrastructure::datastore::postgres::records::task::{TaskRecord, TaskRecordExt};

// ═══════════════════════════════════════════════════════════════════════════
// helpers.rs tests
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn str_to_task_state_converts_all_states() {
    let states = [
        "CREATED",
        "PENDING",
        "SCHEDULED",
        "RUNNING",
        "CANCELLED",
        "STOPPED",
        "COMPLETED",
        "FAILED",
        "SKIPPED",
    ];
    for state in &states {
        let converted = str_to_task_state(state);
        assert_eq!(converted.to_string(), *state);
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// auth.rs tests — UserRecord → User
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn user_record_to_user_basic_fields() {
    let now = fixed_now();
    let record = UserRecord {
        id: "user-001".to_string(),
        name: "Test User".to_string(),
        username_: "testuser".to_string(),
        password_: "$2b$12$hashed".to_string(),
        created_at: now,
        is_disabled: false,
    };
    let user = record.to_user().expect("conversion should succeed");

    assert_eq!(user.id.as_deref(), Some("user-001"));
    assert_eq!(user.name.as_deref(), Some("Test User"));
    assert_eq!(user.username.as_deref(), Some("testuser"));
    assert_eq!(user.password_hash.as_deref(), Some("$2b$12$hashed"));
    assert!(user.password.is_none()); // password should never be set from record
    assert!(!user.disabled);
}

#[test]
fn user_record_to_user_disabled() {
    let now = fixed_now();
    let record = UserRecord {
        id: "user-002".to_string(),
        name: "Banned".to_string(),
        username_: "banned".to_string(),
        password_: "".to_string(),
        created_at: now,
        is_disabled: true,
    };
    let user = record.to_user().expect("conversion should succeed");

    assert!(user.disabled);
}

// ═══════════════════════════════════════════════════════════════════════════
// auth.rs tests — RoleRecord → Role
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn role_record_to_role_basic_fields() {
    let now = fixed_now();
    let record = RoleRecord {
        id: "role-001".to_string(),
        slug: "admin".to_string(),
        name: "Administrator".to_string(),
        created_at: now,
    };
    let role = record.to_role().expect("conversion should succeed");

    assert_eq!(role.id.as_deref(), Some("role-001"));
    assert_eq!(role.slug.as_deref(), Some("admin"));
    assert_eq!(role.name.as_deref(), Some("Administrator"));
}

// ═══════════════════════════════════════════════════════════════════════════
// log.rs tests — TaskLogPartRecord → TaskLogPart
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn task_log_part_record_to_task_log_part() {
    let now = fixed_now();
    let record = TaskLogPartRecord {
        id: "log-001".to_string(),
        number_: 1,
        task_id: "task-001".to_string(),
        created_at: now,
        contents: "line 1\nline 2\n".to_string(),
    };
    let part = record
        .to_task_log_part()
        .expect("conversion should succeed");

    assert_eq!(part.id.as_deref(), Some("log-001"));
    assert_eq!(part.number, 1);
    assert_eq!(part.task_id.as_deref(), Some("task-001"));
    assert_eq!(part.contents.as_deref(), Some("line 1\nline 2\n"));
    assert!(part.created_at.is_some());
}

// ═══════════════════════════════════════════════════════════════════════════
// node.rs tests — NodeRecord → Node
// ═══════════════════════════════════════════════════════════════════════════

fn base_node_record() -> NodeRecord {
    let now = time::OffsetDateTime::now_utc();
    NodeRecord {
        id: "node-001".to_string(),
        name: "worker-1".to_string(),
        started_at: now,
        last_heartbeat_at: now, // recent heartbeat
        cpu_percent: 45.5,
        queue: "default".to_string(),
        status: NODE_STATUS_UP.to_string(),
        hostname: "worker-1.local".to_string(),
        port: 8080,
        task_count: 3,
        version_: "1.0.0".to_string(),
    }
}

#[test]
fn node_record_to_node_basic_fields() {
    let record = base_node_record();
    let node = record.to_node().expect("conversion should succeed");

    assert_eq!(node.id.as_deref(), Some("node-001"));
    assert_eq!(node.name.as_deref(), Some("worker-1"));
    assert_eq!(node.hostname.as_deref(), Some("worker-1.local"));
    assert_eq!(node.port, Some(8080));
    assert_eq!(node.task_count, Some(3));
    assert_eq!(node.version, Some("1.0.0".to_string()));
    assert_eq!(node.queue.as_deref(), Some("default"));
}

#[test]
fn node_record_to_node_recent_heartbeat_stays_up() {
    let record = base_node_record();
    let node = record.to_node().expect("conversion should succeed");

    assert_eq!(node.status, Some(NodeStatus::UP));
}

#[test]
fn node_record_to_node_stale_heartbeat_goes_offline() {
    let stale = fixed_now() - time::Duration::seconds(120);
    let record = NodeRecord {
        last_heartbeat_at: stale,
        status: NODE_STATUS_UP.to_string(),
        ..base_node_record()
    };
    let node = record.to_node().expect("conversion should succeed");

    assert_eq!(node.status, Some(NodeStatus::OFFLINE));
}

#[test]
fn node_record_to_node_non_up_status_preserved() {
    let stale = fixed_now() - time::Duration::seconds(120);
    let record = NodeRecord {
        last_heartbeat_at: stale,
        status: "DOWN".to_string(),
        ..base_node_record()
    };
    let node = record.to_node().expect("conversion should succeed");

    assert_eq!(node.status, Some(NodeStatus::DOWN));
}

#[test]
fn node_record_to_node_cpu_percent_preserved() {
    let record = NodeRecord {
        cpu_percent: 99.9,
        ..base_node_record()
    };
    let node = record.to_node().expect("conversion should succeed");
    assert!((node.cpu_percent.unwrap_or(0.0) - 99.9).abs() < f64::EPSILON);
}

// ═══════════════════════════════════════════════════════════════════════════
// scheduled_job.rs tests — ScheduledJobRecord → ScheduledJob
// ═══════════════════════════════════════════════════════════════════════════

#[allow(clippy::unwrap_used)]
fn base_scheduled_job_record() -> ScheduledJobRecord {
    let now = fixed_now();
    ScheduledJobRecord {
        id: "sched-001".to_string(),
        cron_expr: Some("0 0 * * *".to_string()),
        name: Some("Nightly Build".to_string()),
        description: Some("Build every night".to_string()),
        tags: Some(vec!["nightly".to_string()]),
        state: "ACTIVE".to_string(),
        created_at: now,
        created_by: "user-001".to_string(),
        tasks: serde_json::to_vec(&Vec::<twerk_core::task::Task>::new()).unwrap_or_default(),
        inputs: serde_json::to_vec(&std::collections::HashMap::<String, String>::new())
            .unwrap_or_default(),
        output_: None,
        defaults: None,
        webhooks: None,
        auto_delete: None,
        secrets: None,
    }
}

#[allow(clippy::unwrap_used)]
fn base_user() -> User {
    User {
        id: Some(UserId::new("user-001").unwrap()),
        name: Some("Test User".to_string()),
        username: Some("testuser".to_string()),
        password_hash: Some("hashed".to_string()),
        password: None,
        created_at: Some(fixed_now()),
        disabled: false,
    }
}

#[test]
fn scheduled_job_record_to_scheduled_job_basic() {
    let record = base_scheduled_job_record();
    let sj = record
        .to_scheduled_job(vec![], base_user(), vec![], None)
        .expect("conversion should succeed");

    assert_eq!(sj.id.as_deref(), Some("sched-001"));
    assert_eq!(sj.name.as_deref(), Some("Nightly Build"));
    assert_eq!(sj.description.as_deref(), Some("Build every night"));
    assert_eq!(sj.state, ScheduledJobState::Active);
}

#[test]
fn scheduled_job_record_to_scheduled_job_with_cron() {
    let record = ScheduledJobRecord {
        cron_expr: Some("0 0 * * *".to_string()),
        ..base_scheduled_job_record()
    };
    let sj = record
        .to_scheduled_job(vec![], base_user(), vec![], None)
        .expect("conversion should succeed");

    assert_eq!(sj.cron.as_deref(), Some("0 0 * * *"));
}

#[test]
fn scheduled_job_record_to_scheduled_job_with_inputs() {
    let mut inputs = HashMap::new();
    inputs.insert("BRANCH".to_string(), "main".to_string());
    let record = ScheduledJobRecord {
        inputs: serde_json::to_vec(&inputs).unwrap_or_default(),
        ..base_scheduled_job_record()
    };
    let sj = record
        .to_scheduled_job(vec![], base_user(), vec![], None)
        .expect("conversion should succeed");

    let sj_inputs = sj.inputs.as_ref().expect("inputs should be present");
    assert_eq!(sj_inputs.get("BRANCH").map(String::as_str), Some("main"));
}

#[test]
fn scheduled_job_record_to_scheduled_job_with_tags() {
    let record = ScheduledJobRecord {
        tags: Some(vec!["nightly".to_string(), "prod".to_string()]),
        ..base_scheduled_job_record()
    };
    let sj = record
        .to_scheduled_job(vec![], base_user(), vec![], None)
        .expect("conversion should succeed");

    let tags = sj.tags.as_ref().expect("tags should be present");
    assert_eq!(tags.len(), 2);
}

#[test]
fn scheduled_job_record_to_scheduled_job_no_secrets() {
    let record = base_scheduled_job_record();
    let sj = record
        .to_scheduled_job(vec![], base_user(), vec![], None)
        .expect("conversion should succeed");

    assert!(sj.secrets.is_none());
}

#[test]
fn scheduled_job_record_to_scheduled_job_empty_webhooks_and_perms_yield_none() {
    let record = ScheduledJobRecord {
        webhooks: Some(serde_json::to_vec(&Vec::<Webhook>::new()).unwrap_or_default()),
        ..base_scheduled_job_record()
    };
    let sj = record
        .to_scheduled_job(vec![], base_user(), vec![], None)
        .expect("conversion should succeed");

    assert!(sj.webhooks.is_none());
    assert!(sj.permissions.is_none());
}

// ═══════════════════════════════════════════════════════════════════════════
// task.rs tests — TaskRecord → Task
// ═══════════════════════════════════════════════════════════════════════════

#[allow(clippy::unwrap_used)]
fn base_task_record() -> TaskRecord {
    let now = fixed_now();
    TaskRecord {
        id: "00000000-0000-0000-0000-000000000001".to_string(),
        job_id: "00000000-0000-0000-0000-000000000002".to_string(),
        position: 0,
        name: Some("build".to_string()),
        description: Some("build the project".to_string()),
        state: "PENDING".to_string(),
        created_at: now,
        scheduled_at: None,
        started_at: None,
        completed_at: None,
        failed_at: None,
        cmd: Some(vec!["cargo".to_string(), "build".to_string()]),
        entrypoint: None,
        run_script: Some("cargo build".to_string()),
        image: Some("rust:latest".to_string()),
        registry: None,
        env: None,
        files_: None,
        queue: Some("default".to_string()),
        error_: None,
        pre_tasks: None,
        post_tasks: None,
        sidecars: None,
        mounts: None,
        networks: None,
        node_id: None,
        retry: None,
        limits: None,
        timeout: Some("30s".to_string()),
        var: Some("result".to_string()),
        result: None,
        parallel: None,
        parent_id: None,
        each_: None,
        subjob: None,
        gpus: None,
        if_: None,
        tags: None,
        priority: Some(5),
        workdir: Some("/src".to_string()),
        progress: Some(0.0),
    }
}

#[test]
fn task_record_to_task_basic_fields() {
    let record = base_task_record();
    let task = record.to_task().expect("conversion should succeed");

    assert_eq!(
        task.id.as_deref(),
        Some("00000000-0000-0000-0000-000000000001")
    );
    assert_eq!(
        task.job_id.as_deref(),
        Some("00000000-0000-0000-0000-000000000002")
    );
    assert_eq!(task.position, 0);
    assert_eq!(task.name.as_deref(), Some("build"));
    assert_eq!(task.description.as_deref(), Some("build the project"));
    assert_eq!(task.state.to_string(), "PENDING");
    assert_eq!(task.run.as_deref(), Some("cargo build"));
    assert_eq!(task.image.as_deref(), Some("rust:latest"));
    assert_eq!(task.queue.as_deref(), Some("default"));
    assert_eq!(task.timeout.as_deref(), Some("30s"));
    assert_eq!(task.var.as_deref(), Some("result"));
    assert_eq!(task.priority, 5);
    assert_eq!(task.workdir.as_deref(), Some("/src"));
    assert_eq!(task.progress, 0.0);
    assert!(task.probe.is_none());
    assert_eq!(task.redelivered, 0);
    assert!(task.parent_id.is_none());
    assert!(task.error.is_none());
    assert!(task.result.is_none());
}

#[test]
fn task_record_to_task_with_cmd_and_entrypoint() {
    let record = TaskRecord {
        cmd: Some(vec![
            "sh".to_string(),
            "-c".to_string(),
            "echo hi".to_string(),
        ]),
        entrypoint: Some(vec!["/entrypoint.sh".to_string()]),
        ..base_task_record()
    };
    let task = record.to_task().expect("conversion should succeed");

    let cmd = task.cmd.as_ref().expect("cmd should be present");
    assert_eq!(cmd.len(), 3);
    assert_eq!(cmd[0], "sh");
    assert_eq!(cmd[1], "-c");
    assert_eq!(cmd[2], "echo hi");

    let entry = task
        .entrypoint
        .as_ref()
        .expect("entrypoint should be present");
    assert_eq!(entry.len(), 1);
    assert_eq!(entry[0], "/entrypoint.sh");
}

#[test]
fn task_record_to_task_with_env_and_files() {
    let mut env_map = HashMap::new();
    env_map.insert("RUST_LOG".to_string(), "debug".to_string());
    env_map.insert("HOME".to_string(), "/root".to_string());
    let env = Some(serde_json::to_vec(&env_map).expect("env json should serialize"));

    let mut files_map = HashMap::new();
    files_map.insert("config.yml".to_string(), "key: val".to_string());
    let files = Some(serde_json::to_vec(&files_map).expect("files json should serialize"));

    let record = TaskRecord {
        env,
        files_: files,
        ..base_task_record()
    };
    let task = record.to_task().expect("conversion should succeed");

    let env_result = task.env.as_ref().expect("env should be present");
    assert_eq!(
        env_result.get("RUST_LOG").map(String::as_str),
        Some("debug")
    );
    assert_eq!(env_result.get("HOME").map(String::as_str), Some("/root"));

    let files_result = task.files.as_ref().expect("files should be present");
    assert_eq!(
        files_result.get("config.yml").map(String::as_str),
        Some("key: val")
    );
}

#[test]
fn task_record_to_task_with_registry() {
    let registry = Registry {
        username: Some("admin".to_string()),
        password: Some("s3cret".to_string()),
    };
    let registry_bytes =
        Some(serde_json::to_vec(&registry).expect("registry json should serialize"));

    let record = TaskRecord {
        registry: registry_bytes,
        ..base_task_record()
    };
    let task = record.to_task().expect("conversion should succeed");

    let reg = task.registry.as_ref().expect("registry should be present");
    assert_eq!(reg.username.as_deref(), Some("admin"));
    assert_eq!(reg.password.as_deref(), Some("s3cret"));
}

#[test]
fn task_record_to_task_with_retry_and_limits() {
    let retry = TaskRetry {
        limit: 3,
        attempts: 0,
    };
    let limits = TaskLimits {
        cpus: Some("0.5".to_string()),
        memory: Some("256MB".to_string()),
    };
    let record = TaskRecord {
        retry: serde_json::to_vec(&retry).ok(),
        limits: serde_json::to_vec(&limits).ok(),
        ..base_task_record()
    };
    let task = record.to_task().expect("conversion should succeed");

    let r = task.retry.as_ref().expect("retry should be present");
    assert_eq!(r.limit, 3);
    assert_eq!(r.attempts, 0);

    let l = task.limits.as_ref().expect("limits should be present");
    assert_eq!(l.cpus.as_deref(), Some("0.5"));
    assert_eq!(l.memory.as_deref(), Some("256MB"));
}

#[test]
fn task_record_to_task_with_parallel() {
    let parallel = ParallelTask {
        tasks: Some(vec![]),
        completions: 0,
    };
    let record = TaskRecord {
        parallel: serde_json::to_vec(&parallel).ok(),
        ..base_task_record()
    };
    let task = record.to_task().expect("conversion should succeed");

    assert!(task.parallel.is_some());
    let p = task.parallel.as_ref().unwrap();
    assert_eq!(p.completions, 0);
    assert!(p.tasks.is_some());
}

#[test]
fn task_record_to_task_with_networks_and_tags() {
    let record = TaskRecord {
        networks: Some(vec!["bridge".to_string(), "host".to_string()]),
        tags: Some(vec!["ci".to_string(), "rust".to_string()]),
        ..base_task_record()
    };
    let task = record.to_task().expect("conversion should succeed");

    let nets = task.networks.as_ref().expect("networks should be present");
    assert_eq!(nets, &["bridge".to_string(), "host".to_string()]);

    let tags = task.tags.as_ref().expect("tags should be present");
    assert_eq!(tags, &["ci".to_string(), "rust".to_string()]);
}

#[test]
fn task_record_to_task_with_parent_id() {
    let record = TaskRecord {
        parent_id: Some("00000000-0000-0000-0000-000000000003".to_string()),
        ..base_task_record()
    };
    let task = record.to_task().expect("conversion should succeed");

    assert_eq!(
        task.parent_id.as_deref(),
        Some("00000000-0000-0000-0000-000000000003")
    );
}

#[test]
fn task_record_to_task_with_error_and_result() {
    let record = TaskRecord {
        error_: Some("oom killed".to_string()),
        result: Some("success".to_string()),
        ..base_task_record()
    };
    let task = record.to_task().expect("conversion should succeed");

    assert_eq!(task.error.as_deref(), Some("oom killed"));
    assert_eq!(task.result.as_deref(), Some("success"));
}

#[test]
fn task_record_to_task_default_priority_and_progress() {
    let record = TaskRecord {
        priority: None,
        progress: None,
        ..base_task_record()
    };
    let task = record.to_task().expect("conversion should succeed");

    assert_eq!(task.priority, 0);
    assert_eq!(task.progress, 0.0);
}

#[test]
fn task_record_to_task_all_timestamps() {
    let now = fixed_now();
    let record = TaskRecord {
        scheduled_at: Some(now),
        started_at: Some(now),
        completed_at: Some(now),
        failed_at: Some(now),
        ..base_task_record()
    };
    let task = record.to_task().expect("conversion should succeed");

    assert!(task.scheduled_at.is_some());
    assert!(task.started_at.is_some());
    assert!(task.completed_at.is_some());
    assert!(task.failed_at.is_some());
}

#[test]
fn task_record_to_task_empty_optional_fields() {
    let now = fixed_now();
    let record = TaskRecord {
        id: "00000000-0000-0000-0000-000000000004".to_string(),
        job_id: "00000000-0000-0000-0000-000000000002".to_string(),
        state: "CREATED".to_string(),
        created_at: now,
        position: 0,
        name: None,
        description: None,
        scheduled_at: None,
        started_at: None,
        completed_at: None,
        failed_at: None,
        cmd: None,
        entrypoint: None,
        run_script: None,
        image: None,
        registry: None,
        env: None,
        files_: None,
        queue: None,
        error_: None,
        pre_tasks: None,
        post_tasks: None,
        sidecars: None,
        mounts: None,
        networks: None,
        node_id: None,
        retry: None,
        limits: None,
        timeout: None,
        var: None,
        result: None,
        parallel: None,
        parent_id: None,
        each_: None,
        subjob: None,
        gpus: None,
        if_: None,
        tags: None,
        priority: None,
        workdir: None,
        progress: None,
    };
    let task = record.to_task().expect("conversion should succeed");

    assert_eq!(
        task.id.as_deref(),
        Some("00000000-0000-0000-0000-000000000004")
    );
    assert_eq!(task.state.to_string(), "CREATED");
    assert!(task.name.is_none());
    assert!(task.description.is_none());
    assert!(task.cmd.is_none());
    assert!(task.image.is_none());
    assert!(task.queue.is_none());
    assert!(task.timeout.is_none());
    assert!(task.var.is_none());
    assert!(task.result.is_none());
    assert!(task.error.is_none());
    assert!(task.networks.is_none());
    assert!(task.tags.is_none());
    assert!(task.gpus.is_none());
    assert!(task.r#if.is_none());
    assert!(task.workdir.is_none());
    assert!(task.parent_id.is_none());
    assert!(task.node_id.is_none());
    assert!(task.registry.is_none());
    assert!(task.env.is_none());
    assert!(task.files.is_none());
    assert!(task.retry.is_none());
    assert!(task.limits.is_none());
    assert!(task.parallel.is_none());
    assert!(task.each.is_none());
    assert!(task.subjob.is_none());
    assert!(task.mounts.is_none());
    assert!(task.pre.is_none());
    assert!(task.post.is_none());
    assert!(task.sidecars.is_none());
}

// ═══════════════════════════════════════════════════════════════════════════
// job.rs tests — JobRecord → Job
// ═══════════════════════════════════════════════════════════════════════════

#[allow(clippy::unwrap_used)]
fn base_job_record() -> JobRecord {
    let now = fixed_now();
    JobRecord {
        id: "00000000-0000-0000-0000-000000000001".to_string(),
        name: Some("Build Job".to_string()),
        description: Some("Build the project".to_string()),
        tags: Some(vec!["ci".to_string()]),
        state: "PENDING".to_string(),
        created_at: now,
        created_by: "00000000-0000-0000-0000-000000000002".to_string(),
        started_at: None,
        completed_at: None,
        failed_at: None,
        delete_at: None,
        tasks: serde_json::to_vec(&Vec::<twerk_core::task::Task>::new()).unwrap_or_default(),
        position: 1,
        inputs: serde_json::to_vec(&std::collections::HashMap::<String, String>::new())
            .unwrap_or_default(),
        context: serde_json::to_vec(&twerk_core::job::JobContext::default()).unwrap_or_default(),
        parent_id: None,
        task_count: 5,
        output_: None,
        result: None,
        error_: None,
        defaults: None,
        webhooks: None,
        auto_delete: None,
        secrets: None,
        progress: Some(0.0),
        scheduled_job_id: None,
    }
}

#[allow(clippy::unwrap_used)]
fn base_job_user() -> User {
    User {
        id: Some(twerk_core::id::UserId::new("00000000-0000-0000-0000-000000000002").unwrap()),
        name: Some("Test User".to_string()),
        username: Some("testuser".to_string()),
        password_hash: Some("hashed".to_string()),
        password: None,
        created_at: Some(fixed_now()),
        disabled: false,
    }
}

#[test]
fn job_record_to_job_basic_fields() {
    let record = base_job_record();
    let user = base_job_user();
    let job = record
        .to_job(vec![], vec![], user, vec![], None)
        .expect("conversion should succeed");

    assert_eq!(
        job.id.as_deref(),
        Some("00000000-0000-0000-0000-000000000001")
    );
    assert_eq!(job.name.as_deref(), Some("Build Job"));
    assert_eq!(job.description.as_deref(), Some("Build the project"));
    assert_eq!(job.state, JobState::Pending);
    assert_eq!(job.position, 1);
    assert_eq!(job.task_count, 5);
    assert_eq!(job.progress, 0.0);
}

#[test]
fn job_record_to_job_with_tags() {
    let record = JobRecord {
        tags: Some(vec![
            "ci".to_string(),
            "rust".to_string(),
            "release".to_string(),
        ]),
        ..base_job_record()
    };
    let job = record
        .to_job(vec![], vec![], base_job_user(), vec![], None)
        .expect("conversion should succeed");

    let tags = job.tags.as_ref().expect("tags should be present");
    assert_eq!(tags.len(), 3);
    assert_eq!(tags[0], "ci");
    assert_eq!(tags[1], "rust");
    assert_eq!(tags[2], "release");
}

#[test]
fn job_record_to_job_with_created_by() {
    let record = base_job_record();
    let user = base_job_user();
    let job = record
        .to_job(vec![], vec![], user, vec![], None)
        .expect("conversion should succeed");

    let created_by = job
        .created_by
        .as_ref()
        .expect("created_by should be present");
    assert_eq!(
        created_by.id.as_deref(),
        Some("00000000-0000-0000-0000-000000000002")
    );
    assert_eq!(created_by.username.as_deref(), Some("testuser"));
}

#[test]
fn job_record_to_job_with_inputs() {
    let mut inputs = HashMap::new();
    inputs.insert("var1".to_string(), "val1".to_string());
    inputs.insert("var2".to_string(), "val2".to_string());
    let record = JobRecord {
        inputs: serde_json::to_vec(&inputs).unwrap_or_default(),
        ..base_job_record()
    };
    let job = record
        .to_job(vec![], vec![], base_job_user(), vec![], None)
        .expect("conversion should succeed");

    let job_inputs = job.inputs.as_ref().expect("inputs should be present");
    assert_eq!(job_inputs.len(), 2);
    assert_eq!(job_inputs.get("var1").map(String::as_str), Some("val1"));
    assert_eq!(job_inputs.get("var2").map(String::as_str), Some("val2"));
}

#[test]
fn job_record_to_job_with_defaults() {
    let defaults = CoreJobDefaults {
        timeout: Some("30s".to_string()),
        retry: Some(TaskRetry {
            limit: 3,
            attempts: 0,
        }),
        limits: Some(TaskLimits {
            cpus: Some("1.0".to_string()),
            memory: Some("512MB".to_string()),
        }),
        queue: None,
        priority: 0,
    };
    let record = JobRecord {
        defaults: serde_json::to_vec(&defaults).ok(),
        ..base_job_record()
    };
    let job = record
        .to_job(vec![], vec![], base_job_user(), vec![], None)
        .expect("conversion should succeed");

    let d = job.defaults.as_ref().expect("defaults should be present");
    assert_eq!(d.timeout.as_deref(), Some("30s"));
    let r = d.retry.as_ref().expect("retry should be present");
    assert_eq!(r.limit, 3);
    let l = d.limits.as_ref().expect("limits should be present");
    assert_eq!(l.cpus.as_deref(), Some("1.0"));
}

#[test]
fn job_record_to_job_with_auto_delete() {
    let auto_delete = AutoDelete {
        after: Some("5h".to_string()),
    };
    let record = JobRecord {
        auto_delete: serde_json::to_vec(&auto_delete).ok(),
        ..base_job_record()
    };
    let job = record
        .to_job(vec![], vec![], base_job_user(), vec![], None)
        .expect("conversion should succeed");

    let ad = job
        .auto_delete
        .as_ref()
        .expect("auto_delete should be present");
    assert_eq!(ad.after.as_deref(), Some("5h"));
}

#[test]
fn job_record_to_job_with_webhooks() {
    let webhooks = vec![
        Webhook {
            url: Some("http://example.com/1".to_string()),
            headers: None,
            event: None,
            r#if: None,
        },
        Webhook {
            url: Some("http://example.com/2".to_string()),
            headers: Some({
                let mut m = HashMap::new();
                m.insert("Auth".to_string(), "Bearer token".to_string());
                m
            }),
            event: Some("job.StatusChange".to_string()),
            r#if: None,
        },
    ];
    let record = JobRecord {
        webhooks: serde_json::to_vec(&webhooks).ok(),
        ..base_job_record()
    };
    let job = record
        .to_job(vec![], vec![], base_job_user(), vec![], None)
        .expect("conversion should succeed");

    let wh = job.webhooks.as_ref().expect("webhooks should be present");
    assert_eq!(wh.len(), 2);
    assert_eq!(wh[0].url.as_deref(), Some("http://example.com/1"));
    assert_eq!(wh[1].event.as_deref(), Some("job.StatusChange"));
}

#[test]
fn job_record_to_job_with_permissions() {
    let perms = vec![
        Permission {
            user: Some(base_job_user()),
            role: None,
        },
        Permission {
            user: None,
            role: Some(twerk_core::role::Role {
                id: Some(
                    twerk_core::id::RoleId::new("00000000-0000-0000-0000-000000000003").unwrap(),
                ),
                slug: Some("public".to_string()),
                name: Some("Public".to_string()),
                created_at: Some(fixed_now()),
            }),
        },
    ];
    let record = base_job_record();
    let job = record
        .to_job(vec![], vec![], base_job_user(), perms, None)
        .expect("conversion should succeed");

    let job_perms = job
        .permissions
        .as_ref()
        .expect("permissions should be present");
    assert_eq!(job_perms.len(), 2);
    assert!(job_perms[0].user.is_some());
    assert!(job_perms[1].role.is_some());
}

#[test]
fn job_record_to_job_with_schedule() {
    let record = JobRecord {
        scheduled_job_id: Some("00000000-0000-0000-0000-000000000004".to_string()),
        ..base_job_record()
    };
    let job = record
        .to_job(vec![], vec![], base_job_user(), vec![], None)
        .expect("conversion should succeed");

    let sched = job.schedule.as_ref().expect("schedule should be present");
    assert_eq!(
        sched.id.as_deref(),
        Some("00000000-0000-0000-0000-000000000004")
    );
    assert!(sched.cron.is_none());
}

#[test]
fn job_record_to_job_with_delete_at() {
    let delete_at = fixed_now() + time::Duration::days(7);
    let record = JobRecord {
        delete_at: Some(delete_at),
        ..base_job_record()
    };
    let job = record
        .to_job(vec![], vec![], base_job_user(), vec![], None)
        .expect("conversion should succeed");

    assert!(job.delete_at.is_some());
}

#[test]
fn job_record_to_job_without_encryption_secrets_pass_through() {
    let mut secrets = HashMap::new();
    secrets.insert("key".to_string(), "value".to_string());
    let record = JobRecord {
        secrets: serde_json::to_vec(&secrets).ok(),
        ..base_job_record()
    };
    // No encryption key → secrets should be returned as-is (not encrypted)
    let job = record
        .to_job(vec![], vec![], base_job_user(), vec![], None)
        .expect("conversion should succeed");

    let job_secrets = job.secrets.as_ref().expect("secrets should be present");
    assert_eq!(job_secrets.get("key").map(String::as_str), Some("value"));
}

#[test]
fn job_record_to_job_no_secrets() {
    let record = base_job_record();
    let job = record
        .to_job(vec![], vec![], base_job_user(), vec![], None)
        .expect("conversion should succeed");

    assert!(job.secrets.is_none());
}

#[test]
fn job_record_to_job_empty_webhooks_and_perms_yield_none() {
    // Empty webhooks JSON and empty perms → None
    let record_with_empty_webhooks = JobRecord {
        webhooks: Some(serde_json::to_vec(&Vec::<Webhook>::new()).unwrap_or_default()),
        ..base_job_record()
    };
    let job = record_with_empty_webhooks
        .to_job(vec![], vec![], base_job_user(), vec![], None)
        .expect("conversion should succeed");

    assert!(job.webhooks.is_none()); // empty vec → None
    assert!(job.permissions.is_none()); // empty perms → None
}

#[test]
fn job_record_to_job_progress_defaults() {
    let record = JobRecord {
        progress: None,
        ..base_job_record()
    };
    let job = record
        .to_job(vec![], vec![], base_job_user(), vec![], None)
        .expect("conversion should succeed");

    assert_eq!(job.progress, 0.0);
}
