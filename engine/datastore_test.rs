//! Datastore proxy tests
//!
//! Tests for [`DatastoreProxy`], [`InMemoryDatastore`], [`PostgresAdapter`], and datastore factory functions.

use crate::datastore::{
    DatastoreProxy, InMemoryDatastore,
    create_datastore, new_inmemory_datastore, new_inmemory_datastore_arc,
    env_string, env_string_default, env_int_default,
};
use tork::datastore::Datastore;
use tork::job::Job;
use tork::task::Task;
use tork::node::Node;
use tork::user::User;
use tork::role::Role;

#[tokio::test]
async fn test_datastore_proxy_new() {
    let proxy = DatastoreProxy::new();
    // Should not be initialized
    let result = proxy.health_check().await;
    assert!(result.is_err());
}

#[tokio::test]
async fn test_datastore_proxy_init() {
    std::env::set_var("TORK_DATASTORE_TYPE", "inmemory");
    let proxy = DatastoreProxy::new();
    proxy.init().await.expect("should init");
    
    // Should now be healthy
    let result = proxy.health_check().await;
    assert!(result.is_ok());
    
    std::env::remove_var("TORK_DATASTORE_TYPE");
}

#[tokio::test]
async fn test_datastore_proxy_check_init_when_not_initialized() {
    let proxy = DatastoreProxy::new();
    let result = proxy.check_init().await;
    assert!(result.is_err());
}

#[tokio::test]
async fn test_datastore_proxy_check_init_after_init() {
    std::env::set_var("TORK_DATASTORE_TYPE", "inmemory");
    let proxy = DatastoreProxy::new();
    proxy.init().await.expect("should init");
    let result = proxy.check_init().await;
    assert!(result.is_ok());
    std::env::remove_var("TORK_DATASTORE_TYPE");
}

#[tokio::test]
async fn test_datastore_proxy_set_datastore() {
    let proxy = DatastoreProxy::new();
    proxy.set_datastore(new_inmemory_datastore()).await;
    
    let result = proxy.health_check().await;
    assert!(result.is_ok());
}

#[tokio::test]
async fn test_datastore_proxy_clone_inner() {
    let proxy = DatastoreProxy::new();
    let cloned = proxy.clone_inner();
    
    // Both should be independent but uninitialized
    let result1 = proxy.check_init().await;
    let result2 = cloned.check_init().await;
    assert!(result1.is_err());
    assert!(result2.is_err());
}

#[tokio::test]
async fn test_datastore_proxy_not_initialized_error() {
    let proxy = DatastoreProxy::new();
    let task = Task::default();
    let result = proxy.create_task(task).await;
    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("not initialized"));
}

// ── InMemoryDatastore task tests ────────────────────────────────

#[tokio::test]
async fn test_inmemory_datastore_create_and_get_task() {
    let ds = new_inmemory_datastore();
    
    let task = Task {
        id: Some("task-1".to_string()),
        ..Default::default()
    };
    
    ds.create_task(task.clone()).await.expect("should create");
    
    let found = ds.get_task_by_id("task-1".to_string()).await.expect("should get");
    assert!(found.is_some());
    assert_eq!(found.unwrap().id.as_deref(), Some("task-1"));
}

#[tokio::test]
async fn test_inmemory_datastore_get_task_not_found() {
    let ds = new_inmemory_datastore();
    let found = ds.get_task_by_id("nonexistent".to_string()).await.expect("should get");
    assert!(found.is_none());
}

#[tokio::test]
async fn test_inmemory_datastore_update_task() {
    let ds = new_inmemory_datastore();
    
    let task = Task {
        id: Some("task-1".to_string()),
        state: "CREATED".to_string(),
        ..Default::default()
    };
    
    ds.create_task(task.clone()).await.expect("should create");
    
    let updated_task = Task {
        id: Some("task-1".to_string()),
        state: "RUNNING".to_string(),
        ..Default::default()
    };
    
    ds.update_task("task-1".to_string(), updated_task).await.expect("should update");
    
    let found = ds.get_task_by_id("task-1".to_string()).await.expect("should get");
    assert!(found.is_some());
    assert_eq!(found.unwrap().state, "RUNNING");
}

#[tokio::test]
async fn test_inmemory_datastore_update_task_not_found() {
    let ds = new_inmemory_datastore();
    let task = Task {
        id: Some("task-1".to_string()),
        ..Default::default()
    };
    
    let result = ds.update_task("nonexistent".to_string(), task).await;
    assert!(result.is_err());
}

#[tokio::test]
async fn test_inmemory_datastore_get_active_tasks() {
    let ds = new_inmemory_datastore();
    
    // Create tasks with different states
    let task1 = Task {
        id: Some("task-1".to_string()),
        job_id: Some("job-1".to_string()),
        state: "RUNNING".to_string(),
        ..Default::default()
    };
    let task2 = Task {
        id: Some("task-2".to_string()),
        job_id: Some("job-1".to_string()),
        state: "COMPLETED".to_string(),
        ..Default::default()
    };
    let task3 = Task {
        id: Some("task-3".to_string()),
        job_id: Some("job-1".to_string()),
        state: "PENDING".to_string(),
        ..Default::default()
    };
    
    ds.create_task(task1).await.expect("should create");
    ds.create_task(task2).await.expect("should create");
    ds.create_task(task3).await.expect("should create");
    
    let active = ds.get_active_tasks("job-1".to_string()).await.expect("should get");
    assert_eq!(active.len(), 2); // task-1 (RUNNING) and task-3 (PENDING)
}

#[tokio::test]
async fn test_inmemory_datastore_get_next_task() {
    let ds = new_inmemory_datastore();
    
    let parent = Task {
        id: Some("parent".to_string()),
        job_id: Some("job-1".to_string()),
        state: "RUNNING".to_string(),
        ..Default::default()
    };
    let child1 = Task {
        id: Some("child-1".to_string()),
        parent_id: Some("parent".to_string()),
        job_id: Some("job-1".to_string()),
        state: "CREATED".to_string(),
        ..Default::default()
    };
    let child2 = Task {
        id: Some("child-2".to_string()),
        parent_id: Some("parent".to_string()),
        job_id: Some("job-1".to_string()),
        state: "CREATED".to_string(),
        ..Default::default()
    };
    
    ds.create_task(parent).await.expect("should create");
    ds.create_task(child1).await.expect("should create");
    ds.create_task(child2).await.expect("should create");
    
    let next = ds.get_next_task("parent".to_string()).await.expect("should get");
    assert!(next.is_some());
    assert_eq!(next.unwrap().id.as_deref(), Some("child-1"));
}

// ── InMemoryDatastore job tests ─────────────────────────────────

#[tokio::test]
async fn test_inmemory_datastore_create_and_get_job() {
    let ds = new_inmemory_datastore();
    
    let job = Job {
        id: Some("job-1".to_string()),
        name: Some("Test Job".to_string()),
        ..Default::default()
    };
    
    ds.create_job(job.clone()).await.expect("should create");
    
    let found = ds.get_job_by_id("job-1".to_string()).await.expect("should get");
    assert!(found.is_some());
    assert_eq!(found.unwrap().name.as_deref(), Some("Test Job"));
}

#[tokio::test]
async fn test_inmemory_datastore_get_job_not_found() {
    let ds = new_inmemory_datastore();
    let found = ds.get_job_by_id("nonexistent".to_string()).await.expect("should get");
    assert!(found.is_none());
}

#[tokio::test]
async fn test_inmemory_datastore_update_job() {
    let ds = new_inmemory_datastore();
    
    let job = Job {
        id: Some("job-1".to_string()),
        state: "PENDING".to_string(),
        ..Default::default()
    };
    
    ds.create_job(job).await.expect("should create");
    
    let updated = Job {
        id: Some("job-1".to_string()),
        state: "RUNNING".to_string(),
        ..Default::default()
    };
    
    ds.update_job("job-1".to_string(), updated).await.expect("should update");
    
    let found = ds.get_job_by_id("job-1".to_string()).await.expect("should get");
    assert!(found.is_some());
    assert_eq!(found.unwrap().state, "RUNNING");
}

#[tokio::test]
async fn test_inmemory_datastore_get_jobs_pagination() {
    let ds = new_inmemory_datastore();
    
    // Create multiple jobs
    for i in 0..5 {
        let job = Job {
            id: Some(format!("job-{}", i)),
            name: Some(format!("Job {}", i)),
            ..Default::default()
        };
        ds.create_job(job).await.expect("should create");
    }
    
    let page = ds.get_jobs("".to_string(), "".to_string(), 1, 2).await.expect("should get");
    assert_eq!(page.items.len(), 2);
    assert_eq!(page.total, 5);
    assert_eq!(page.page, 1);
    assert_eq!(page.size, 2);
}

// ── InMemoryDatastore node tests ────────────────────────────────

#[tokio::test]
async fn test_inmemory_datastore_create_and_get_node() {
    let ds = new_inmemory_datastore();
    
    let node = Node {
        id: Some("node-1".to_string()),
        hostname: Some("host1.example.com".to_string()),
        ..Default::default()
    };
    
    ds.create_node(node.clone()).await.expect("should create");
    
    let found = ds.get_node_by_id("node-1".to_string()).await.expect("should get");
    assert!(found.is_some());
    assert_eq!(found.unwrap().hostname.as_deref(), Some("host1.example.com"));
}

#[tokio::test]
async fn test_inmemory_datastore_update_node() {
    let ds = new_inmemory_datastore();
    
    let node = Node {
        id: Some("node-1".to_string()),
        state: "IDLE".to_string(),
        ..Default::default()
    };
    
    ds.create_node(node).await.expect("should create");
    
    let updated = Node {
        id: Some("node-1".to_string()),
        state: "RUNNING".to_string(),
        ..Default::default()
    };
    
    ds.update_node("node-1".to_string(), updated).await.expect("should update");
    
    let found = ds.get_node_by_id("node-1".to_string()).await.expect("should get");
    assert!(found.is_some());
    assert_eq!(found.unwrap().state, "RUNNING");
}

#[tokio::test]
async fn test_inmemory_datastore_get_active_nodes() {
    let ds = new_inmemory_datastore();
    
    let node1 = Node {
        id: Some("node-1".to_string()),
        ..Default::default()
    };
    let node2 = Node {
        id: Some("node-2".to_string()),
        ..Default::default()
    };
    
    ds.create_node(node1).await.expect("should create");
    ds.create_node(node2).await.expect("should create");
    
    let nodes = ds.get_active_nodes().await.expect("should get");
    assert_eq!(nodes.len(), 2);
}

// ── InMemoryDatastore user tests ───────────────────────────────

#[tokio::test]
async fn test_inmemory_datastore_create_and_get_user() {
    let ds = new_inmemory_datastore();
    
    let user = User {
        id: Some("user-1".to_string()),
        username: Some("testuser".to_string()),
        ..Default::default()
    };
    
    ds.create_user(user.clone()).await.expect("should create");
    
    let found = ds.get_user("testuser".to_string()).await.expect("should get");
    assert!(found.is_some());
    assert_eq!(found.unwrap().username.as_deref(), Some("testuser"));
}

#[tokio::test]
async fn test_inmemory_datastore_get_user_by_id() {
    let ds = new_inmemory_datastore();
    
    let user = User {
        id: Some("user-1".to_string()),
        username: Some("testuser".to_string()),
        ..Default::default()
    };
    
    ds.create_user(user).await.expect("should create");
    
    let found = ds.get_user("testuser".to_string()).await.expect("should get");
    assert!(found.is_some());
}

// ── InMemoryDatastore role tests ────────────────────────────────

#[tokio::test]
async fn test_inmemory_datastore_create_and_get_role() {
    let ds = new_inmemory_datastore();
    
    let role = Role {
        id: Some("role-1".to_string()),
        name: Some("admin".to_string()),
        ..Default::default()
    };
    
    ds.create_role(role.clone()).await.expect("should create");
    
    let found = ds.get_role("role-1".to_string()).await.expect("should get");
    assert!(found.is_some());
    assert_eq!(found.unwrap().name.as_deref(), Some("admin"));
}

#[tokio::test]
async fn test_inmemory_datastore_get_roles() {
    let ds = new_inmemory_datastore();
    
    let role1 = Role {
        id: Some("role-1".to_string()),
        name: Some("admin".to_string()),
        ..Default::default()
    };
    let role2 = Role {
        id: Some("role-2".to_string()),
        name: Some("user".to_string()),
        ..Default::default()
    };
    
    ds.create_role(role1).await.expect("should create");
    ds.create_role(role2).await.expect("should create");
    
    let roles = ds.get_roles().await.expect("should get");
    assert_eq!(roles.len(), 2);
}

#[tokio::test]
async fn test_inmemory_datastore_assign_and_get_user_roles() {
    let ds = new_inmemory_datastore();
    
    let user = User {
        id: Some("user-1".to_string()),
        username: Some("testuser".to_string()),
        ..Default::default()
    };
    let role = Role {
        id: Some("role-1".to_string()),
        name: Some("admin".to_string()),
        ..Default::default()
    };
    
    ds.create_user(user).await.expect("should create");
    ds.create_role(role).await.expect("should create");
    ds.assign_role("user-1".to_string(), "role-1".to_string()).await.expect("should assign");
    
    let roles = ds.get_user_roles("user-1".to_string()).await.expect("should get");
    assert_eq!(roles.len(), 1);
    assert_eq!(roles[0].name.as_deref(), Some("admin"));
}

#[tokio::test]
async fn test_inmemory_datastore_unassign_role() {
    let ds = new_inmemory_datastore();
    
    let user = User {
        id: Some("user-1".to_string()),
        username: Some("testuser".to_string()),
        ..Default::default()
    };
    let role = Role {
        id: Some("role-1".to_string()),
        name: Some("admin".to_string()),
        ..Default::default()
    };
    
    ds.create_user(user).await.expect("should create");
    ds.create_role(role).await.expect("should create");
    ds.assign_role("user-1".to_string(), "role-1".to_string()).await.expect("should assign");
    
    let roles_before = ds.get_user_roles("user-1".to_string()).await.expect("should get");
    assert_eq!(roles_before.len(), 1);
    
    ds.unassign_role("user-1".to_string(), "role-1".to_string()).await.expect("should unassign");
    
    let roles_after = ds.get_user_roles("user-1".to_string()).await.expect("should get");
    assert!(roles_after.is_empty());
}

// ── InMemoryDatastore metrics tests ────────────────────────────

#[tokio::test]
async fn test_inmemory_datastore_get_metrics() {
    let ds = new_inmemory_datastore();
    
    let metrics = ds.get_metrics().await.expect("should get");
    assert_eq!(metrics.jobs.running, 0);
    assert_eq!(metrics.tasks.running, 0);
    assert_eq!(metrics.nodes.running, 0);
}

#[tokio::test]
async fn test_inmemory_datastore_metrics_with_data() {
    let ds = new_inmemory_datastore();
    
    let job = Job {
        id: Some("job-1".to_string()),
        state: "RUNNING".to_string(),
        ..Default::default()
    };
    let task = Task {
        id: Some("task-1".to_string()),
        state: "RUNNING".to_string(),
        job_id: Some("job-1".to_string()),
        ..Default::default()
    };
    let node = Node {
        id: Some("node-1".to_string()),
        ..Default::default()
    };
    
    ds.create_job(job).await.expect("should create");
    ds.create_task(task).await.expect("should create");
    ds.create_node(node).await.expect("should create");
    
    let metrics = ds.get_metrics().await.expect("should get");
    assert_eq!(metrics.jobs.running, 1);
    assert_eq!(metrics.tasks.running, 1);
    assert_eq!(metrics.nodes.running, 1);
}

// ── InMemoryDatastore health and shutdown ──────────────────────

#[tokio::test]
async fn test_inmemory_datastore_health_check() {
    let ds = new_inmemory_datastore();
    let result = ds.health_check().await;
    assert!(result.is_ok());
}

#[tokio::test]
async fn test_inmemory_datastore_shutdown() {
    let ds = new_inmemory_datastore();
    let result = ds.shutdown().await;
    assert!(result.is_ok());
}

// ── new_inmemory_datastore_arc tests ───────────────────────────

#[tokio::test]
async fn test_new_inmemory_datastore_arc() {
    let ds = new_inmemory_datastore_arc();
    let result = ds.health_check().await;
    assert!(result.is_ok());
}

// ── Config helper tests ──────────────────────────────────────────

#[test]
fn test_env_string_unset() {
    std::env::remove_var("TORK_TEST_DS_UNSET");
    assert_eq!(env_string("test.ds.unset"), "");
}

#[test]
fn test_env_string_set() {
    std::env::set_var("TORK_TEST_DS_SET", "ds_value");
    assert_eq!(env_string("test.ds.set"), "ds_value");
    std::env::remove_var("TORK_TEST_DS_SET");
}

#[test]
fn test_env_string_default_empty() {
    assert_eq!(env_string_default("test.ds.default", "fallback"), "fallback");
}

#[test]
fn test_env_string_default_set() {
    std::env::set_var("TORK_TEST_DS_DEFAULT", "custom_ds");
    assert_eq!(env_string_default("test.ds.default", "fallback"), "custom_ds");
    std::env::remove_var("TORK_TEST_DS_DEFAULT");
}

#[test]
fn test_env_int_default_empty() {
    assert_eq!(env_int_default("test.ds.int.empty", 42), 42);
}

#[test]
fn test_env_int_default_set() {
    std::env::set_var("TORK_TEST_DS_INT", "100");
    assert_eq!(env_int_default("test.ds.int", 42), 100);
    std::env::remove_var("TORK_TEST_DS_INT");
}

#[test]
fn test_env_int_default_invalid() {
    std::env::set_var("TORK_TEST_DS_INT_INVALID", "not_a_number");
    assert_eq!(env_int_default("test.ds.int.invalid", 42), 42);
    std::env::remove_var("TORK_TEST_DS_INT_INVALID");
}

// ── create_datastore tests ───────────────────────────────────────

#[tokio::test]
async fn test_create_datastore_inmemory() {
    std::env::set_var("TORK_DATASTORE_TYPE", "inmemory");
    let ds = create_datastore().await;
    assert!(ds.is_ok());
    let ds = ds.unwrap();
    
    let result = ds.health_check().await;
    assert!(result.is_ok());
    
    std::env::remove_var("TORK_DATASTORE_TYPE");
}

#[tokio::test]
async fn test_create_datastore_unknown_type() {
    std::env::set_var("TORK_DATASTORE_TYPE", "unknown");
    let ds = create_datastore().await;
    assert!(ds.is_err());
    assert!(ds.unwrap_err().to_string().contains("unknown datastore type"));
    std::env::remove_var("TORK_DATASTORE_TYPE");
}

// ── InMemoryDatastore task log tests ────────────────────────────

#[tokio::test]
async fn test_inmemory_datastore_task_log_parts() {
    use tork::task::TaskLogPart;
    
    let ds = new_inmemory_datastore();
    
    let part = TaskLogPart {
        task_id: Some("task-1".to_string()),
        content: "Log line 1".to_string().into_bytes(),
        ..Default::default()
    };
    
    ds.create_task_log_part(part).await.expect("should create");
    
    let parts = ds.get_task_log_parts("task-1".to_string(), "".to_string(), 1, 10).await.expect("should get");
    assert_eq!(parts.items.len(), 1);
}

#[tokio::test]
async fn test_inmemory_datastore_pagination() {
    let ds = new_inmemory_datastore();
    
    // Create 10 jobs
    for i in 0..10 {
        let job = Job {
            id: Some(format!("job-{}", i)),
            name: Some(format!("Job {}", i)),
            ..Default::default()
        };
        ds.create_job(job).await.expect("should create");
    }
    
    // Page 1 with size 3
    let page1 = ds.get_jobs("".to_string(), "".to_string(), 1, 3).await.expect("should get");
    assert_eq!(page1.items.len(), 3);
    assert_eq!(page1.total, 10);
    assert_eq!(page1.page, 1);
    assert_eq!(page1.size, 3);
    
    // Page 2 with size 3
    let page2 = ds.get_jobs("".to_string(), "".to_string(), 2, 3).await.expect("should get");
    assert_eq!(page2.items.len(), 3);
    assert_eq!(page2.page, 2);
    
    // Page 4 with size 3 (should have 1 item)
    let page4 = ds.get_jobs("".to_string(), "".to_string(), 4, 3).await.expect("should get");
    assert_eq!(page4.items.len(), 1);
}

// ── Scheduled job tests ──────────────────────────────────────────

#[tokio::test]
async fn test_inmemory_datastore_scheduled_jobs() {
    use tork::job::ScheduledJob;
    
    let ds = new_inmemory_datastore();
    
    let scheduled = ScheduledJob {
        id: Some("sched-1".to_string()),
        name: Some("Scheduled Job".to_string()),
        state: "ACTIVE".to_string(),
        cron: Some("*/5 * * * *".to_string()),
        ..Default::default()
    };
    
    ds.create_scheduled_job(scheduled).await.expect("should create");
    
    let found = ds.get_scheduled_job_by_id("sched-1".to_string()).await.expect("should get");
    assert!(found.is_some());
    
    let active = ds.get_active_scheduled_jobs().await.expect("should get");
    assert_eq!(active.len(), 1);
    
    let all = ds.get_scheduled_jobs("".to_string(), 1, 10).await.expect("should get");
    assert_eq!(all.items.len(), 1);
}

#[tokio::test]
async fn test_inmemory_datastore_update_scheduled_job() {
    use tork::job::ScheduledJob;
    
    let ds = new_inmemory_datastore();
    
    let scheduled = ScheduledJob {
        id: Some("sched-1".to_string()),
        name: Some("Scheduled Job".to_string()),
        state: "ACTIVE".to_string(),
        ..Default::default()
    };
    
    ds.create_scheduled_job(scheduled).await.expect("should create");
    
    let updated = ScheduledJob {
        id: Some("sched-1".to_string()),
        name: Some("Updated Job".to_string()),
        state: "PAUSED".to_string(),
        ..Default::default()
    };
    
    ds.update_scheduled_job("sched-1".to_string(), updated).await.expect("should update");
    
    let found = ds.get_scheduled_job_by_id("sched-1".to_string()).await.expect("should get");
    assert!(found.is_some());
    assert_eq!(found.unwrap().state, "PAUSED");
}

#[tokio::test]
async fn test_inmemory_datastore_delete_scheduled_job() {
    use tork::job::ScheduledJob;
    
    let ds = new_inmemory_datastore();
    
    let scheduled = ScheduledJob {
        id: Some("sched-1".to_string()),
        name: Some("Scheduled Job".to_string()),
        state: "ACTIVE".to_string(),
        ..Default::default()
    };
    
    ds.create_scheduled_job(scheduled).await.expect("should create");
    
    let before = ds.get_active_scheduled_jobs().await.expect("should get");
    assert_eq!(before.len(), 1);
    
    ds.delete_scheduled_job("sched-1".to_string()).await.expect("should delete");
    
    let after = ds.get_active_scheduled_jobs().await.expect("should get");
    assert!(after.is_empty());
}