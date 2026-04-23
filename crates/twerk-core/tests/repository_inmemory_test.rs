use time::OffsetDateTime;
use twerk_core::id::{JobId, NodeId, RoleId, ScheduledJobId, TaskId, UserId};
use twerk_core::job::{Job, JobState, ScheduledJob, ScheduledJobState};
use twerk_core::node::{Node, NodeStatus};
use twerk_core::repository::{Error, Options, Repository};
use twerk_core::repository_inmemory::InMemoryRepository;
use twerk_core::role::Role;
use twerk_core::task::{Task, TaskLogPart, TaskState};
use twerk_core::user::User;

fn create_test_task(id: &str, job_id: Option<&JobId>, state: TaskState) -> Task {
    Task {
        id: Some(TaskId::new(id).unwrap()),
        job_id: job_id.map(|j| j.clone()),
        state,
        ..Default::default()
    }
}

fn create_test_job(id: &str) -> Job {
    Job {
        id: Some(JobId::new(id).unwrap()),
        state: JobState::Pending,
        ..Default::default()
    }
}

fn create_test_node(id: &str, status: Option<NodeStatus>) -> Node {
    Node {
        id: Some(NodeId::new(id).unwrap()),
        status,
        ..Default::default()
    }
}

fn create_test_user(username: &str) -> User {
    User {
        id: Some(UserId::new("uid-1").unwrap()),
        username: Some(username.to_string()),
        ..Default::default()
    }
}

fn create_test_role(slug: &str) -> Role {
    Role {
        id: Some(RoleId::new("rid-1").unwrap()),
        slug: Some(slug.to_string()),
        ..Default::default()
    }
}

#[tokio::test]
async fn create_and_get_task() {
    let repo = InMemoryRepository::new(Options::default());
    let task = create_test_task("task-1", None, TaskState::Created);
    repo.create_task(&task).await.unwrap();
    let retrieved = repo.get_task_by_id("task-1").await.unwrap();
    assert_eq!(retrieved.id, task.id);
}

#[tokio::test]
async fn get_task_not_found() {
    let repo = InMemoryRepository::new(Options::default());
    let result = repo.get_task_by_id("nonexistent").await;
    assert!(matches!(result, Err(Error::TaskNotFound)));
}

#[tokio::test]
async fn get_active_tasks() {
    let repo = InMemoryRepository::new(Options::default());
    let job_id = JobId::new("550e8400-e29b-41d4-a716-446655440000").unwrap();

    repo.create_task(&create_test_task(
        "task-1",
        Some(&job_id),
        TaskState::Running,
    ))
    .await
    .unwrap();
    repo.create_task(&create_test_task(
        "task-2",
        Some(&job_id),
        TaskState::Completed,
    ))
    .await
    .unwrap();
    repo.create_task(&create_test_task(
        "task-3",
        Some(&job_id),
        TaskState::Pending,
    ))
    .await
    .unwrap();

    let active = repo.get_active_tasks(job_id.as_str()).await.unwrap();
    assert_eq!(active.len(), 2);
}

#[tokio::test]
async fn create_and_get_node() {
    let repo = InMemoryRepository::new(Options::default());
    let node = create_test_node("node-1", Some(NodeStatus::UP));
    repo.create_node(&node).await.unwrap();
    let retrieved = repo.get_node_by_id("node-1").await.unwrap();
    assert_eq!(retrieved.id, node.id);
}

#[tokio::test]
async fn get_active_nodes() {
    let repo = InMemoryRepository::new(Options::default());
    repo.create_node(&create_test_node("node-1", Some(NodeStatus::UP)))
        .await
        .unwrap();
    repo.create_node(&create_test_node("node-2", Some(NodeStatus::DOWN)))
        .await
        .unwrap();
    repo.create_node(&create_test_node("node-3", Some(NodeStatus::UP)))
        .await
        .unwrap();

    let active = repo.get_active_nodes().await.unwrap();
    assert_eq!(active.len(), 2);
}

#[tokio::test]
async fn create_and_get_job() {
    let repo = InMemoryRepository::new(Options::default());
    let job = create_test_job("550e8400-e29b-41d4-a716-446655440000");
    repo.create_job(&job).await.unwrap();
    let retrieved = repo
        .get_job_by_id("550e8400-e29b-41d4-a716-446655440000")
        .await
        .unwrap();
    assert_eq!(retrieved.id, job.id);
}

#[tokio::test]
async fn create_and_get_scheduled_job() {
    let repo = InMemoryRepository::new(Options::default());
    let sj = ScheduledJob {
        id: Some(ScheduledJobId::new("sj-1").unwrap()),
        state: ScheduledJobState::Active,
        ..Default::default()
    };
    repo.create_scheduled_job(&sj).await.unwrap();
    let retrieved = repo.get_scheduled_job_by_id("sj-1").await.unwrap();
    assert_eq!(retrieved.id, sj.id);
}

#[tokio::test]
async fn delete_scheduled_job() {
    let repo = InMemoryRepository::new(Options::default());
    let sj = ScheduledJob {
        id: Some(ScheduledJobId::new("sj-1").unwrap()),
        state: ScheduledJobState::Active,
        ..Default::default()
    };
    repo.create_scheduled_job(&sj).await.unwrap();
    repo.delete_scheduled_job("sj-1").await.unwrap();
    let result = repo.get_scheduled_job_by_id("sj-1").await;
    assert!(matches!(result, Err(Error::ScheduledJobNotFound)));
}

#[tokio::test]
async fn create_and_get_user() {
    let repo = InMemoryRepository::new(Options::default());
    let user = create_test_user("alice");
    repo.create_user(&user).await.unwrap();
    let retrieved = repo.get_user("alice").await.unwrap();
    assert_eq!(retrieved.username, user.username);
}

#[tokio::test]
async fn user_not_found() {
    let repo = InMemoryRepository::new(Options::default());
    let result = repo.get_user("nobody").await;
    assert!(matches!(result, Err(Error::UserNotFound)));
}

#[tokio::test]
async fn create_and_get_role() {
    let repo = InMemoryRepository::new(Options::default());
    let role = create_test_role("admin");
    repo.create_role(&role).await.unwrap();
    let retrieved = repo.get_role("rid-1").await.unwrap();
    assert_eq!(retrieved.slug, role.slug);
}

#[tokio::test]
async fn assign_and_unassign_role() {
    let repo = InMemoryRepository::new(Options::default());
    let user = create_test_user("alice");
    let role = create_test_role("admin");
    repo.create_user(&user).await.unwrap();
    repo.create_role(&role).await.unwrap();

    repo.assign_role("uid-1", "rid-1").await.unwrap();
    let roles = repo.get_user_roles("uid-1").await.unwrap();
    assert_eq!(roles.len(), 1);

    repo.unassign_role("uid-1", "rid-1").await.unwrap();
    let roles = repo.get_user_roles("uid-1").await.unwrap();
    assert_eq!(roles.len(), 0);
}

#[tokio::test]
async fn get_metrics() {
    let repo = InMemoryRepository::new(Options::default());
    let metrics = repo.get_metrics().await.unwrap();
    assert_eq!(metrics.jobs.running, 0);
    assert_eq!(metrics.tasks.running, 0);
    assert_eq!(metrics.nodes.running, 0);
}

#[tokio::test]
async fn health_check() {
    let repo = InMemoryRepository::new(Options::default());
    repo.health_check().await.unwrap();
}

#[tokio::test]
async fn task_log_parts() {
    let repo = InMemoryRepository::new(Options::default());
    let log_part = TaskLogPart {
        id: Some("log-1".to_string()),
        task_id: Some(TaskId::new("task-1").unwrap()),
        number: 1,
        contents: Some("test output".to_string()),
        created_at: Some(OffsetDateTime::now_utc()),
    };
    repo.create_task_log_part(&log_part).await.unwrap();
    let page = repo.get_task_log_parts("task-1", "", 1, 10).await.unwrap();
    assert_eq!(page.items.len(), 1);
}

#[tokio::test]
async fn paginate_jobs() {
    let repo = InMemoryRepository::new(Options::default());
    for i in 0..5 {
        let job = Job {
            id: Some(JobId::new(format!("550e8400-e29b-41d4-a716-44665544000{}", i)).unwrap()),
            ..Default::default()
        };
        repo.create_job(&job).await.unwrap();
    }
    let page = repo.get_jobs("", "", 1, 2).await.unwrap();
    assert_eq!(page.items.len(), 2);
    assert_eq!(page.total_items, 5);
    assert_eq!(page.total_pages, 3);
}

#[tokio::test]
async fn update_task() {
    let repo = InMemoryRepository::new(Options::default());
    let task = create_test_task("task-1", None, TaskState::Created);
    repo.create_task(&task).await.unwrap();

    repo.update_task(
        "task-1",
        Box::new(|mut t| {
            t.state = TaskState::Running;
            Ok(t)
        }),
    )
    .await
    .unwrap();

    let updated = repo.get_task_by_id("task-1").await.unwrap();
    assert_eq!(updated.state, TaskState::Running);
}

#[tokio::test]
async fn update_node() {
    let repo = InMemoryRepository::new(Options::default());
    let node = create_test_node("node-1", Some(NodeStatus::UP));
    repo.create_node(&node).await.unwrap();

    repo.update_node(
        "node-1",
        Box::new(|mut n| {
            n.status = Some(NodeStatus::DOWN);
            Ok(n)
        }),
    )
    .await
    .unwrap();

    let updated = repo.get_node_by_id("node-1").await.unwrap();
    assert_eq!(updated.status, Some(NodeStatus::DOWN));
}

#[tokio::test]
async fn update_scheduled_job() {
    let repo = InMemoryRepository::new(Options::default());
    let sj = ScheduledJob {
        id: Some(ScheduledJobId::new("sj-1").unwrap()),
        state: ScheduledJobState::Active,
        ..Default::default()
    };
    repo.create_scheduled_job(&sj).await.unwrap();

    repo.update_scheduled_job(
        "sj-1",
        Box::new(|mut s| {
            s.state = ScheduledJobState::Paused;
            Ok(s)
        }),
    )
    .await
    .unwrap();

    let updated = repo.get_scheduled_job_by_id("sj-1").await.unwrap();
    assert_eq!(updated.state, ScheduledJobState::Paused);
}
