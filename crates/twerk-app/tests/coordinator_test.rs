#![allow(clippy::field_reassign_with_default)]
#![allow(clippy::unwrap_used)]
#![allow(clippy::expect_used)]
use anyhow::Result;
use twerk_app::engine::coordinator::create_coordinator;
use twerk_app::engine::{BrokerProxy, DatastoreProxy};
use twerk_core::id::JobId;
use twerk_core::job::{Job, JobState};
use twerk_core::task::{Task, TaskState};
use twerk_infrastructure::broker::Broker;
use twerk_infrastructure::datastore::Datastore;

async fn settle_async_work() {
    tokio::task::yield_now().await;
    tokio::time::advance(std::time::Duration::from_secs(1)).await;
    tokio::task::yield_now().await;
}

fn to_job_id(value: impl Into<String>) -> JobId {
    JobId::new(value).expect("test job id should be valid")
}

#[tokio::test(start_paused = true)]
async fn job_completes_when_tasks_are_finished() -> Result<()> {
    // Set up in-memory datastore
    std::env::set_var("TWERK_DATASTORE_TYPE", "inmemory");

    let broker = BrokerProxy::new();
    let datastore = DatastoreProxy::new();

    broker.init("inmemory", Some("")).await?;
    datastore.init().await?;

    let coordinator = create_coordinator(broker.clone(), datastore.clone()).await?;
    coordinator.start().await?;

    let job = Job {
        id: Some(to_job_id("550e8400-e29b-41d4-a716-446655440001")),
        name: Some("test job 2".to_string()),
        state: JobState::Pending,
        tasks: Some(vec![Task {
            name: Some("task 1".to_string()),
            image: Some("alpine".to_string()),
            run: Some("echo hello".to_string()),
            ..Default::default()
        }]),
        task_count: 1,
        ..Default::default()
    };

    coordinator.submit_job(job).await?;

    settle_async_work().await;
    let tasks = datastore
        .get_active_tasks("550e8400-e29b-41d4-a716-446655440001")
        .await?;

    assert_eq!(tasks.len(), 1);
    let task = tasks[0].clone();

    // Simulate task completion
    let mut completed_task = task.clone();
    completed_task.state = twerk_core::task::TaskState::Completed;
    completed_task.completed_at = Some(time::OffsetDateTime::now_utc());

    broker.publish_task_progress(&completed_task).await?;

    settle_async_work().await;
    let persisted = datastore
        .get_job_by_id("550e8400-e29b-41d4-a716-446655440001")
        .await?;

    assert_eq!(persisted.state, JobState::Completed);
    assert!(persisted.completed_at.is_some());

    Ok(())
}

#[tokio::test(start_paused = true)]
async fn first_top_level_task_is_scheduled_immediately_when_job_submitted() -> Result<()> {
    std::env::set_var("TWERK_DATASTORE_TYPE", "inmemory");

    let broker = BrokerProxy::new();
    let datastore = DatastoreProxy::new();

    broker.init("inmemory", Some("")).await?;
    datastore.init().await?;

    let coordinator = create_coordinator(broker.clone(), datastore.clone()).await?;
    coordinator.start().await?;

    let job = Job {
        id: Some(to_job_id("550e8400-e29b-41d4-a716-446655440002")),
        name: Some("first task scheduling".to_string()),
        state: JobState::Pending,
        tasks: Some(vec![Task {
            name: Some("task 1".to_string()),
            image: Some("alpine".to_string()),
            run: Some("echo hello".to_string()),
            ..Default::default()
        }]),
        task_count: 1,
        ..Default::default()
    };

    coordinator.submit_job(job).await?;

    settle_async_work().await;
    let tasks = datastore
        .get_all_tasks_for_job("550e8400-e29b-41d4-a716-446655440002")
        .await?;

    assert_eq!(tasks.len(), 1);
    assert_eq!(tasks[0].state, TaskState::Scheduled);
    assert_eq!(tasks[0].queue, Some("default".to_string()));
    assert!(tasks[0].scheduled_at.is_some());

    let persisted = datastore
        .get_job_by_id("550e8400-e29b-41d4-a716-446655440002")
        .await?;
    assert_eq!(persisted.state, JobState::Scheduled);
    assert_eq!(persisted.position, 1);

    Ok(())
}

#[tokio::test(start_paused = true)]
async fn parallel_tasks_scheduled_when_job_submitted() -> Result<()> {
    // Set up in-memory datastore
    std::env::set_var("TWERK_DATASTORE_TYPE", "inmemory");

    let broker = BrokerProxy::new();
    let datastore = DatastoreProxy::new();

    broker.init("inmemory", Some("")).await?;
    datastore.init().await?;

    let coordinator = create_coordinator(broker.clone(), datastore.clone()).await?;
    coordinator.start().await?;

    let job = Job {
        id: Some(to_job_id("550e8400-e29b-41d4-a716-446655440003")),
        state: JobState::Pending,
        tasks: Some(vec![Task {
            name: Some("parallel task".to_string()),
            parallel: Some(twerk_core::task::ParallelTask {
                tasks: Some(vec![
                    Task {
                        name: Some("p1".to_string()),
                        ..Default::default()
                    },
                    Task {
                        name: Some("p2".to_string()),
                        ..Default::default()
                    },
                ]),
                ..Default::default()
            }),
            ..Default::default()
        }]),
        task_count: 1,
        ..Default::default()
    };

    coordinator.submit_job(job).await?;

    // Wait for the parallel task to be "running" and subtasks to be "pending"
    settle_async_work().await;
    let tasks = datastore
        .get_active_tasks("550e8400-e29b-41d4-a716-446655440003")
        .await?;

    assert_eq!(tasks.len(), 3);

    let parallel_task = tasks.iter().find(|t| t.parallel.is_some()).unwrap();
    assert_eq!(parallel_task.state, TaskState::Running);

    let mut subtasks: Vec<_> = tasks
        .iter()
        .filter(|t| t.parent_id.as_deref() == parallel_task.id.as_deref())
        .collect();
    subtasks.sort_by(|left, right| left.name.cmp(&right.name));
    assert_eq!(subtasks.len(), 2);
    assert_eq!(subtasks[0].name.as_deref(), Some("p1"));
    assert_eq!(subtasks[0].state, TaskState::Scheduled);
    assert_eq!(subtasks[1].name.as_deref(), Some("p2"));
    assert_eq!(subtasks[1].state, TaskState::Scheduled);

    Ok(())
}

#[tokio::test(start_paused = true)]
async fn each_tasks_scheduled_when_job_submitted() -> Result<()> {
    // Set up in-memory datastore
    std::env::set_var("TWERK_DATASTORE_TYPE", "inmemory");

    let broker = BrokerProxy::new();
    let datastore = DatastoreProxy::new();

    broker.init("inmemory", Some("")).await?;
    datastore.init().await?;

    let coordinator = create_coordinator(broker.clone(), datastore.clone()).await?;
    coordinator.start().await?;

    let mut inputs = std::collections::HashMap::new();
    inputs.insert("list".to_string(), "[\"a\", \"b\"]".to_string());

    let job = Job {
        id: Some(to_job_id("550e8400-e29b-41d4-a716-446655440004")),
        state: JobState::Pending,
        context: Some(twerk_core::job::JobContext {
            inputs: Some(inputs),
            ..Default::default()
        }),
        tasks: Some(vec![Task {
            name: Some("each task".to_string()),
            each: Some(Box::new(twerk_core::task::EachTask {
                list: Some("{{ list }}".to_string()),
                task: Some(Box::new(Task {
                    name: Some("each-item".to_string()),
                    ..Default::default()
                })),
                ..Default::default()
            })),
            ..Default::default()
        }]),
        task_count: 1,
        ..Default::default()
    };

    coordinator.submit_job(job).await?;

    // Wait for the each task to be "running" and subtasks to be "scheduled"
    settle_async_work().await;
    let tasks = datastore
        .get_active_tasks("550e8400-e29b-41d4-a716-446655440004")
        .await?;

    assert_eq!(tasks.len(), 3);

    let each_task = tasks.iter().find(|t| t.each.is_some()).unwrap();
    assert_eq!(each_task.state, TaskState::Running);

    let mut subtasks: Vec<_> = tasks
        .iter()
        .filter(|t| t.parent_id.as_deref() == each_task.id.as_deref())
        .collect();
    subtasks.sort_by(|left, right| left.name.cmp(&right.name));
    assert_eq!(subtasks.len(), 2);
    assert_eq!(subtasks[0].name.as_deref(), Some("each-item"));
    assert_eq!(subtasks[0].state, TaskState::Scheduled);
    assert_eq!(subtasks[1].name.as_deref(), Some("each-item"));
    assert_eq!(subtasks[1].state, TaskState::Scheduled);

    Ok(())
}

#[tokio::test(start_paused = true)]
async fn subjob_scheduled_when_parent_job_running() -> Result<()> {
    // Set up in-memory datastore
    std::env::set_var("TWERK_DATASTORE_TYPE", "inmemory");

    let broker = BrokerProxy::new();
    let datastore = DatastoreProxy::new();

    broker.init("inmemory", Some("")).await?;
    datastore.init().await?;

    let coordinator = create_coordinator(broker.clone(), datastore.clone()).await?;
    coordinator.start().await?;

    let job = Job {
        id: Some(to_job_id("550e8400-e29b-41d4-a716-446655440005")),
        state: JobState::Pending,
        tasks: Some(vec![Task {
            name: Some("subjob task".to_string()),
            subjob: Some(twerk_core::task::SubJobTask {
                name: Some("my subjob".to_string()),
                tasks: Some(vec![Task {
                    name: Some("st1".to_string()),
                    ..Default::default()
                }]),
                ..Default::default()
            }),
            ..Default::default()
        }]),
        task_count: 1,
        ..Default::default()
    };

    coordinator.submit_job(job).await?;

    // Wait for the subjob task to be "running"
    settle_async_work().await;
    let tasks = datastore
        .get_active_tasks("550e8400-e29b-41d4-a716-446655440005")
        .await?;

    assert_eq!(tasks.len(), 1);
    let subjob_task = &tasks[0];
    assert_eq!(subjob_task.state, TaskState::Running);

    let subjob_id = subjob_task
        .subjob
        .as_ref()
        .and_then(|s| s.id.clone())
        .expect("subjob id missing");

    // Verify subjob is created
    let subjob = datastore.get_job_by_id(&subjob_id).await?;
    assert_eq!(subjob.name.as_deref(), Some("my subjob"));

    // Subjob should be scheduled because coordinator handles PENDING jobs
    settle_async_work().await;
    let persisted_sj = datastore.get_job_by_id(&subjob_id).await?;

    assert_eq!(persisted_sj.state, JobState::Scheduled);

    Ok(())
}
