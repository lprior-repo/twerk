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
        job_id: job_id.cloned(),
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

// ── Pagination edge cases ────────────────────────────────────────────────────

#[tokio::test]
async fn paginate_jobs_total_pages_rounds_up() {
    // 5 items, page size 2 => ceil(5/2) = 3 pages
    let repo = InMemoryRepository::new(Options::default());
    for i in 0..5 {
        let job = Job {
            id: Some(JobId::new(format!("550e8400-e29b-41d4-a716-44665544000{i}")).unwrap()),
            ..Default::default()
        };
        repo.create_job(&job).await.unwrap();
    }
    let page = repo.get_jobs("", "", 1, 2).await.unwrap();
    assert_eq!(page.total_items, 5);
    assert_eq!(
        page.total_pages, 3,
        "5 items / size 2 must be 3 pages (ceil)"
    );
}

#[tokio::test]
async fn paginate_jobs_second_page_exact_items() {
    // 5 items, page size 2, page 2 should have exactly items at index 2 and 3
    let repo = InMemoryRepository::new(Options::default());
    for i in 0..5 {
        let job = Job {
            id: Some(JobId::new(format!("550e8400-e29b-41d4-a716-44665544000{i}")).unwrap()),
            ..Default::default()
        };
        repo.create_job(&job).await.unwrap();
    }
    let page = repo.get_jobs("", "", 2, 2).await.unwrap();
    assert_eq!(
        page.items.len(),
        2,
        "page 2 of 5 items with size 2 must have 2 items"
    );
}

#[tokio::test]
async fn paginate_jobs_last_page_partial() {
    // 5 items, page size 2, page 3 should have exactly 1 item
    let repo = InMemoryRepository::new(Options::default());
    for i in 0..5 {
        let job = Job {
            id: Some(JobId::new(format!("550e8400-e29b-41d4-a716-44665544000{i}")).unwrap()),
            ..Default::default()
        };
        repo.create_job(&job).await.unwrap();
    }
    let page = repo.get_jobs("", "", 3, 2).await.unwrap();
    assert_eq!(
        page.items.len(),
        1,
        "last page of 5 items with size 2 must have 1 item"
    );
}

#[tokio::test]
async fn paginate_beyond_last_page_returns_empty() {
    // 3 items, page size 10, page 2 should return empty
    let repo = InMemoryRepository::new(Options::default());
    for i in 0..3 {
        let job = Job {
            id: Some(JobId::new(format!("550e8400-e29b-41d4-a716-44665544000{i}")).unwrap()),
            ..Default::default()
        };
        repo.create_job(&job).await.unwrap();
    }
    let page = repo.get_jobs("", "", 2, 10).await.unwrap();
    assert_eq!(
        page.items.len(),
        0,
        "page 2 with only 3 items and size 10 must be empty"
    );
    assert_eq!(page.total_items, 3);
    assert_eq!(page.total_pages, 1);
}

#[tokio::test]
async fn paginate_empty_repo() {
    let repo = InMemoryRepository::new(Options::default());
    let page = repo.get_jobs("", "", 1, 10).await.unwrap();
    assert_eq!(page.items.len(), 0);
    assert_eq!(page.total_items, 0);
    assert_eq!(page.total_pages, 0);
}

#[tokio::test]
async fn paginate_task_log_parts_page_calculation() {
    // Create 7 log parts for a task, paginate with size 3 => 3 pages (2,2,2,1 would be wrong; 3,3,1 is correct)
    let repo = InMemoryRepository::new(Options::default());
    let task_id = TaskId::new("task-logs").unwrap();
    for i in 0..7 {
        let part = TaskLogPart {
            id: Some(format!("log-{i}")),
            task_id: Some(task_id.clone()),
            number: i,
            contents: Some(format!("output-{i}")),
            created_at: Some(OffsetDateTime::now_utc()),
        };
        repo.create_task_log_part(&part).await.unwrap();
    }

    // Page 1: items 0-2
    let p1 = repo
        .get_task_log_parts("task-logs", "", 1, 3)
        .await
        .unwrap();
    assert_eq!(p1.items.len(), 3);
    assert_eq!(p1.total_items, 7);
    assert_eq!(p1.total_pages, 3, "7 items / size 3 => ceil(7/3) = 3 pages");

    // Page 2: items 3-5
    let p2 = repo
        .get_task_log_parts("task-logs", "", 2, 3)
        .await
        .unwrap();
    assert_eq!(p2.items.len(), 3);

    // Page 3: item 6
    let p3 = repo
        .get_task_log_parts("task-logs", "", 3, 3)
        .await
        .unwrap();
    assert_eq!(p3.items.len(), 1);
}

#[tokio::test]
async fn paginate_exact_multiple_page_count() {
    // 4 items, page size 2 => exactly 2 pages, not 3
    let repo = InMemoryRepository::new(Options::default());
    for i in 0..4 {
        let job = Job {
            id: Some(JobId::new(format!("550e8400-e29b-41d4-a716-44665544000{i}")).unwrap()),
            ..Default::default()
        };
        repo.create_job(&job).await.unwrap();
    }
    let page = repo.get_jobs("", "", 1, 2).await.unwrap();
    assert_eq!(
        page.total_pages, 2,
        "4 items / size 2 must be exactly 2 pages"
    );
    assert_eq!(page.total_items, 4);
}

#[tokio::test]
async fn paginate_single_item_full_page() {
    // 1 item, page size 1 => 1 page with 1 item
    let repo = InMemoryRepository::new(Options::default());
    let job = Job {
        id: Some(JobId::new("550e8400-e29b-41d4-a716-446655440000").unwrap()),
        ..Default::default()
    };
    repo.create_job(&job).await.unwrap();
    let page = repo.get_jobs("", "", 1, 1).await.unwrap();
    assert_eq!(page.total_pages, 1);
    assert_eq!(page.items.len(), 1);
    assert_eq!(page.total_items, 1);
}

// ── get_all_tasks_for_job tests ──────────────────────────────────────────────

#[tokio::test]
async fn get_all_tasks_for_job_returns_only_matching_job() {
    let repo = InMemoryRepository::new(Options::default());
    let job_a = JobId::new("550e8400-e29b-41d4-a716-446655440000").unwrap();
    let job_b = JobId::new("550e8400-e29b-41d4-a716-446655440001").unwrap();

    // Tasks for job_a
    let task_a1 = Task {
        id: Some(TaskId::new("task-a1").unwrap()),
        job_id: Some(job_a.clone()),
        state: TaskState::Running,
        ..Default::default()
    };
    let task_a2 = Task {
        id: Some(TaskId::new("task-a2").unwrap()),
        job_id: Some(job_a.clone()),
        state: TaskState::Completed,
        ..Default::default()
    };
    // Task for job_b - should NOT appear
    let task_b1 = Task {
        id: Some(TaskId::new("task-b1").unwrap()),
        job_id: Some(job_b.clone()),
        state: TaskState::Running,
        ..Default::default()
    };

    repo.create_task(&task_a1).await.unwrap();
    repo.create_task(&task_a2).await.unwrap();
    repo.create_task(&task_b1).await.unwrap();

    let result = repo.get_all_tasks_for_job(job_a.as_str()).await.unwrap();

    // Must have exactly 2 tasks for job_a, not 3 (which would happen if != replaced ==)
    assert_eq!(
        result.len(),
        2,
        "only tasks belonging to job_a should be returned"
    );

    // Verify the exact task IDs to catch any default/empty return mutation
    let ids: Vec<&str> = result
        .iter()
        .map(|t| t.id.as_ref().unwrap().as_str())
        .collect();
    assert!(
        ids.contains(&"task-a1"),
        "task-a1 must be in results, got: {ids:?}"
    );
    assert!(
        ids.contains(&"task-a2"),
        "task-a2 must be in results, got: {ids:?}"
    );
    assert!(
        !ids.contains(&"task-b1"),
        "task-b1 must NOT be in results, got: {ids:?}"
    );
}

#[tokio::test]
async fn get_all_tasks_for_job_empty_when_no_tasks() {
    let repo = InMemoryRepository::new(Options::default());
    let job_id = JobId::new("550e8400-e29b-41d4-a716-446655440000").unwrap();
    let result = repo.get_all_tasks_for_job(job_id.as_str()).await.unwrap();
    assert_eq!(result.len(), 0, "should return empty vec, not default task");
    // Verify it's actually empty (not vec![Default::default()])
    for t in &result {
        assert!(
            t.id.is_some(),
            "should not contain default tasks with no id"
        );
    }
}

#[tokio::test]
async fn get_all_tasks_for_job_includes_all_states() {
    let repo = InMemoryRepository::new(Options::default());
    let job_id = JobId::new("550e8400-e29b-41d4-a716-446655440000").unwrap();

    for (i, state) in [
        TaskState::Created,
        TaskState::Pending,
        TaskState::Running,
        TaskState::Completed,
        TaskState::Failed,
        TaskState::Cancelled,
    ]
    .into_iter()
    .enumerate()
    {
        let task = Task {
            id: Some(TaskId::new(format!("t-{i}")).unwrap()),
            job_id: Some(job_id.clone()),
            state,
            ..Default::default()
        };
        repo.create_task(&task).await.unwrap();
    }

    let result = repo.get_all_tasks_for_job(job_id.as_str()).await.unwrap();
    // Unlike get_active_tasks, get_all_tasks_for_job returns ALL states
    assert_eq!(
        result.len(),
        6,
        "all 6 tasks across all states must be returned"
    );

    let states: Vec<TaskState> = result.iter().map(|t| t.state).collect();
    assert!(states.contains(&TaskState::Created));
    assert!(states.contains(&TaskState::Pending));
    assert!(states.contains(&TaskState::Running));
    assert!(states.contains(&TaskState::Completed));
    assert!(states.contains(&TaskState::Failed));
    assert!(states.contains(&TaskState::Cancelled));
}

// ── get_next_task tests ──────────────────────────────────────────────────────

#[tokio::test]
async fn get_next_task_returns_child_with_next_position() {
    let repo = InMemoryRepository::new(Options::default());
    let job_id = JobId::new("550e8400-e29b-41d4-a716-446655440000").unwrap();

    // Parent task at position 0
    let parent = Task {
        id: Some(TaskId::new("parent").unwrap()),
        job_id: Some(job_id.clone()),
        position: 0,
        ..Default::default()
    };
    repo.create_task(&parent).await.unwrap();

    // Child at position 1 (same job, same parent)
    let child1 = Task {
        id: Some(TaskId::new("child-1").unwrap()),
        job_id: Some(job_id.clone()),
        parent_id: Some(TaskId::new("parent").unwrap()),
        position: 1,
        ..Default::default()
    };
    // Child at position 2
    let child2 = Task {
        id: Some(TaskId::new("child-2").unwrap()),
        job_id: Some(job_id.clone()),
        parent_id: Some(TaskId::new("parent").unwrap()),
        position: 2,
        ..Default::default()
    };
    repo.create_task(&child1).await.unwrap();
    repo.create_task(&child2).await.unwrap();

    let next = repo.get_next_task("parent").await.unwrap();

    // Must return the child with the smallest position > parent's position
    assert_eq!(
        next.id.as_ref().unwrap().as_str(),
        "child-1",
        "must return child at position 1, not position 2"
    );
    assert_eq!(next.position, 1);
}

#[tokio::test]
async fn get_next_task_skips_wrong_parent() {
    let repo = InMemoryRepository::new(Options::default());
    let job_id = JobId::new("550e8400-e29b-41d4-a716-446655440000").unwrap();

    let parent = Task {
        id: Some(TaskId::new("parent").unwrap()),
        job_id: Some(job_id.clone()),
        position: 0,
        ..Default::default()
    };
    repo.create_task(&parent).await.unwrap();

    // Task with different parent_id - should be skipped
    let other_child = Task {
        id: Some(TaskId::new("other-child").unwrap()),
        job_id: Some(job_id.clone()),
        parent_id: Some(TaskId::new("some-other-parent").unwrap()),
        position: 1,
        ..Default::default()
    };
    // Correct child with higher position
    let real_child = Task {
        id: Some(TaskId::new("real-child").unwrap()),
        job_id: Some(job_id.clone()),
        parent_id: Some(TaskId::new("parent").unwrap()),
        position: 5,
        ..Default::default()
    };
    repo.create_task(&other_child).await.unwrap();
    repo.create_task(&real_child).await.unwrap();

    let next = repo.get_next_task("parent").await.unwrap();
    assert_eq!(
        next.id.as_ref().unwrap().as_str(),
        "real-child",
        "must skip tasks with wrong parent_id"
    );
}

#[tokio::test]
async fn get_next_task_skips_wrong_job() {
    let repo = InMemoryRepository::new(Options::default());
    let job_a = JobId::new("550e8400-e29b-41d4-a716-446655440000").unwrap();
    let job_b = JobId::new("550e8400-e29b-41d4-a716-446655440001").unwrap();

    let parent = Task {
        id: Some(TaskId::new("parent").unwrap()),
        job_id: Some(job_a.clone()),
        position: 0,
        ..Default::default()
    };
    repo.create_task(&parent).await.unwrap();

    // Task with same parent_id but different job_id - should be skipped
    let wrong_job_child = Task {
        id: Some(TaskId::new("wrong-job-child").unwrap()),
        job_id: Some(job_b.clone()),
        parent_id: Some(TaskId::new("parent").unwrap()),
        position: 1,
        ..Default::default()
    };
    repo.create_task(&wrong_job_child).await.unwrap();

    let result = repo.get_next_task("parent").await;
    assert!(
        matches!(result, Err(Error::TaskNotFound)),
        "should not find tasks from different job, got: {result:?}"
    );
}

#[tokio::test]
async fn get_next_task_skips_equal_position() {
    let repo = InMemoryRepository::new(Options::default());
    let job_id = JobId::new("550e8400-e29b-41d4-a716-446655440000").unwrap();

    // Parent at position 3
    let parent = Task {
        id: Some(TaskId::new("parent").unwrap()),
        job_id: Some(job_id.clone()),
        position: 3,
        ..Default::default()
    };
    repo.create_task(&parent).await.unwrap();

    // Child at same position 3 - should NOT be returned (> not >=)
    let same_pos = Task {
        id: Some(TaskId::new("same-pos").unwrap()),
        job_id: Some(job_id.clone()),
        parent_id: Some(TaskId::new("parent").unwrap()),
        position: 3,
        ..Default::default()
    };
    // Child at position 2 - should NOT be returned (< not >)
    let lower_pos = Task {
        id: Some(TaskId::new("lower-pos").unwrap()),
        job_id: Some(job_id.clone()),
        parent_id: Some(TaskId::new("parent").unwrap()),
        position: 2,
        ..Default::default()
    };
    // Child at position 4 - SHOULD be returned
    let higher_pos = Task {
        id: Some(TaskId::new("higher-pos").unwrap()),
        job_id: Some(job_id.clone()),
        parent_id: Some(TaskId::new("parent").unwrap()),
        position: 4,
        ..Default::default()
    };
    repo.create_task(&same_pos).await.unwrap();
    repo.create_task(&lower_pos).await.unwrap();
    repo.create_task(&higher_pos).await.unwrap();

    let next = repo.get_next_task("parent").await.unwrap();
    assert_eq!(
        next.id.as_ref().unwrap().as_str(),
        "higher-pos",
        "must return only child with position > parent position (not >=, not <, not ==)"
    );
    assert_eq!(next.position, 4);
}

#[tokio::test]
async fn get_next_task_no_children_returns_error() {
    let repo = InMemoryRepository::new(Options::default());
    let job_id = JobId::new("550e8400-e29b-41d4-a716-446655440000").unwrap();

    let parent = Task {
        id: Some(TaskId::new("parent").unwrap()),
        job_id: Some(job_id.clone()),
        position: 0,
        ..Default::default()
    };
    repo.create_task(&parent).await.unwrap();

    let result = repo.get_next_task("parent").await;
    assert!(
        matches!(result, Err(Error::TaskNotFound)),
        "should return TaskNotFound when no children exist, got: {result:?}"
    );
}

#[tokio::test]
async fn get_next_task_parent_not_found() {
    let repo = InMemoryRepository::new(Options::default());
    let result = repo.get_next_task("nonexistent").await;
    assert!(
        matches!(result, Err(Error::TaskNotFound)),
        "should return TaskNotFound when parent doesn't exist"
    );
}

#[tokio::test]
async fn get_next_task_returns_min_position_among_children() {
    let repo = InMemoryRepository::new(Options::default());
    let job_id = JobId::new("550e8400-e29b-41d4-a716-446655440000").unwrap();

    let parent = Task {
        id: Some(TaskId::new("parent").unwrap()),
        job_id: Some(job_id.clone()),
        position: 0,
        ..Default::default()
    };
    repo.create_task(&parent).await.unwrap();

    // Multiple children at various positions
    for (name, pos) in [("c-high", 10), ("c-mid", 5), ("c-low", 1)] {
        let child = Task {
            id: Some(TaskId::new(name).unwrap()),
            job_id: Some(job_id.clone()),
            parent_id: Some(TaskId::new("parent").unwrap()),
            position: pos,
            ..Default::default()
        };
        repo.create_task(&child).await.unwrap();
    }

    let next = repo.get_next_task("parent").await.unwrap();
    assert_eq!(
        next.id.as_ref().unwrap().as_str(),
        "c-low",
        "must return child with minimum position > parent position"
    );
    assert_eq!(next.position, 1);
}
