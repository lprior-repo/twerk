//! Scheduler tests.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use super::mock::{create_test_job, create_test_task, MockDatastore};
use super::Scheduler;
use std::collections::HashMap;
use std::sync::Arc;
use twerk_core::job::JobDefaults;
use twerk_core::task::{
    EachTask, ParallelTask, SubJobTask, Task, TaskLimits, TaskRetry, TaskState,
};
use twerk_infrastructure::broker::inmemory::InMemoryBroker;

#[tokio::test]
async fn test_schedule_regular_task_sets_scheduled_state() {
    let ds = Arc::new(MockDatastore::new());
    let broker = InMemoryBroker::new();
    let job = create_test_job();
    ds.jobs.insert(job.id.clone().unwrap(), job);

    let mut task = create_test_task();
    task.id = Some(twerk_core::id::TaskId::new("task-regular-1").unwrap());

    ds.tasks.insert(task.id.clone().unwrap(), task.clone());

    let scheduler = Scheduler::new(ds.clone(), Arc::new(broker));
    scheduler.schedule_regular_task(task.clone()).await.unwrap();

    let stored = ds
        .tasks
        .get(&twerk_core::id::TaskId::new("task-regular-1").unwrap());
    assert!(stored.is_some());
    assert_eq!(stored.unwrap().state, TaskState::Scheduled);
}

#[tokio::test]
async fn test_schedule_regular_task_sets_default_queue() {
    let ds = Arc::new(MockDatastore::new());
    let broker = InMemoryBroker::new();
    let job = create_test_job();
    ds.jobs.insert(job.id.clone().unwrap(), job);

    let mut task = create_test_task();
    task.id = Some(twerk_core::id::TaskId::new("task-regular-2").unwrap());
    task.queue = None;

    ds.tasks.insert(task.id.clone().unwrap(), task.clone());

    let scheduler = Scheduler::new(ds.clone(), Arc::new(broker));
    scheduler.schedule_regular_task(task.clone()).await.unwrap();

    let stored = ds
        .tasks
        .get(&twerk_core::id::TaskId::new("task-regular-2").unwrap());
    assert!(stored.is_some());
    assert_eq!(stored.unwrap().queue, Some("default".to_string()));
}

#[tokio::test]
async fn test_schedule_regular_task_applies_job_defaults() {
    let ds = Arc::new(MockDatastore::new());
    let broker = InMemoryBroker::new();

    let mut job = create_test_job();
    job.defaults = Some(JobDefaults {
        queue: Some("gpu".to_string()),
        limits: Some(TaskLimits {
            cpus: Some("2".to_string()),
            memory: Some("1g".to_string()),
        }),
        timeout: Some("30m".to_string()),
        retry: Some(TaskRetry {
            attempts: 0,
            limit: 3,
        }),
        priority: 7,
    });
    ds.jobs.insert(job.id.clone().unwrap(), job);

    let mut task = create_test_task();
    task.id = Some(twerk_core::id::TaskId::new("task-regular-defaults").unwrap());
    task.queue = None;
    task.limits = None;
    task.timeout = None;
    task.retry = None;
    task.priority = 0;

    ds.tasks.insert(task.id.clone().unwrap(), task.clone());

    let scheduler = Scheduler::new(ds.clone(), Arc::new(broker));
    scheduler.schedule_regular_task(task).await.unwrap();

    let stored = ds
        .tasks
        .get(&twerk_core::id::TaskId::new("task-regular-defaults").unwrap())
        .unwrap();
    assert_eq!(stored.queue, Some("gpu".to_string()));
    assert_eq!(stored.timeout, Some("30m".to_string()));
    assert_eq!(stored.priority, 7);
    assert_eq!(stored.retry.as_ref().map(|r| r.limit), Some(3));
    assert_eq!(
        stored
            .limits
            .as_ref()
            .and_then(|limits| limits.cpus.clone()),
        Some("2".to_string())
    );
    assert_eq!(
        stored
            .limits
            .as_ref()
            .and_then(|limits| limits.memory.clone()),
        Some("1g".to_string())
    );
}

#[tokio::test]
async fn test_schedule_parallel_task_creates_child_tasks() {
    let ds = Arc::new(MockDatastore::new());
    let broker = InMemoryBroker::new();

    let job = create_test_job();
    ds.jobs.insert(job.id.clone().unwrap(), job.clone());

    let child_task = Task {
        id: None,
        name: Some("Child Task".to_string()),
        run: Some("echo hello".to_string()),
        ..Default::default()
    };

    let mut task = create_test_task();
    task.id = Some(twerk_core::id::TaskId::new("task-parallel-1").unwrap());
    task.parallel = Some(ParallelTask {
        tasks: Some(vec![child_task]),
        completions: 1,
    });

    ds.tasks.insert(task.id.clone().unwrap(), task.clone());

    let scheduler = Scheduler::new(ds.clone(), Arc::new(broker));
    scheduler
        .schedule_parallel_task(task.clone())
        .await
        .unwrap();

    let parent = ds
        .tasks
        .get(&twerk_core::id::TaskId::new("task-parallel-1").unwrap());
    assert!(parent.is_some());
    assert_eq!(parent.unwrap().state, TaskState::Running);

    let child_count = ds
        .tasks
        .iter()
        .filter(|r| r.value().parent_id.is_some())
        .count();
    assert_eq!(child_count, 1);
}

#[tokio::test]
async fn test_schedule_parallel_task_sets_parent_id_on_children() {
    let ds = Arc::new(MockDatastore::new());
    let broker = InMemoryBroker::new();

    let job = create_test_job();
    ds.jobs.insert(job.id.clone().unwrap(), job.clone());

    let child_task = Task {
        id: None,
        name: Some("Child Task".to_string()),
        run: Some("echo hello".to_string()),
        ..Default::default()
    };

    let mut task = create_test_task();
    task.id = Some(twerk_core::id::TaskId::new("task-parallel-2").unwrap());
    task.parallel = Some(ParallelTask {
        tasks: Some(vec![child_task]),
        completions: 1,
    });

    ds.tasks.insert(task.id.clone().unwrap(), task.clone());

    let scheduler = Scheduler::new(ds.clone(), Arc::new(broker));
    scheduler
        .schedule_parallel_task(task.clone())
        .await
        .unwrap();

    let child = ds.tasks.iter().find(|r| r.value().parent_id.is_some());
    assert!(child.is_some());
    assert_eq!(
        child.unwrap().value().parent_id,
        Some(twerk_core::id::TaskId::new("task-parallel-2").unwrap())
    );
}

#[tokio::test]
async fn test_schedule_each_task_creates_task_per_list_item() {
    let ds = Arc::new(MockDatastore::new());
    let broker = InMemoryBroker::new();

    let job = create_test_job();
    ds.jobs.insert(job.id.clone().unwrap(), job.clone());

    let template = Task {
        id: None,
        name: Some("Each Item".to_string()),
        run: Some("echo {{item}}".to_string()),
        ..Default::default()
    };

    let mut task = create_test_task();
    task.id = Some(twerk_core::id::TaskId::new("task-each-1").unwrap());
    task.each = Some(Box::new(EachTask {
        var: Some("item".to_string()),
        list: Some(r#"["a", "b", "c"]"#.to_string()),
        task: Some(Box::new(template)),
        size: 0,
        completions: 0,
        concurrency: 0,
        index: 0,
    }));

    ds.tasks.insert(task.id.clone().unwrap(), task.clone());

    let scheduler = Scheduler::new(ds.clone(), Arc::new(broker));
    scheduler.schedule_each_task(task.clone()).await.unwrap();

    let parent = ds
        .tasks
        .get(&twerk_core::id::TaskId::new("task-each-1").unwrap());
    assert!(parent.is_some());
    assert_eq!(parent.unwrap().state, TaskState::Running);

    let child_count = ds
        .tasks
        .iter()
        .filter(|r| r.value().parent_id.is_some())
        .count();
    assert_eq!(child_count, 3);
}

#[tokio::test]
async fn test_schedule_each_task_sets_size() {
    let ds = Arc::new(MockDatastore::new());
    let broker = InMemoryBroker::new();

    let job = create_test_job();
    ds.jobs.insert(job.id.clone().unwrap(), job.clone());

    let template = Task {
        id: None,
        name: Some("Each Item".to_string()),
        run: Some("echo {{item}}".to_string()),
        ..Default::default()
    };

    let mut task = create_test_task();
    task.id = Some(twerk_core::id::TaskId::new("task-each-2").unwrap());
    task.each = Some(Box::new(EachTask {
        var: Some("item".to_string()),
        list: Some(r#"["x", "y"]"#.to_string()),
        task: Some(Box::new(template)),
        size: 0,
        completions: 0,
        concurrency: 0,
        index: 0,
    }));

    ds.tasks.insert(task.id.clone().unwrap(), task.clone());

    let scheduler = Scheduler::new(ds.clone(), Arc::new(broker));
    scheduler.schedule_each_task(task.clone()).await.unwrap();

    let parent_guard = ds
        .tasks
        .get(&twerk_core::id::TaskId::new("task-each-2").unwrap());
    assert!(parent_guard.is_some());
    let parent = parent_guard.unwrap();
    let each = parent.each.as_ref().unwrap();
    assert_eq!(each.size, 2);
}

#[tokio::test]
async fn test_schedule_subjob_task_creates_subjob() {
    let ds = Arc::new(MockDatastore::new());
    let broker = InMemoryBroker::new();

    let job = create_test_job();
    ds.jobs.insert(job.id.clone().unwrap(), job.clone());

    let subjob_task = Task {
        id: Some(twerk_core::id::TaskId::new("task-subjob-1").unwrap()),
        job_id: Some(twerk_core::id::JobId::new("job-1").unwrap()),
        state: twerk_core::task::TaskState::Created,
        name: Some("SubJob Task".to_string()),
        subjob: Some(SubJobTask {
            id: None,
            name: Some("My SubJob".to_string()),
            description: Some("A subjob".to_string()),
            tasks: Some(vec![Task {
                name: Some("SubTask 1".to_string()),
                run: Some("echo sub".to_string()),
                ..Default::default()
            }]),
            ..Default::default()
        }),
        ..Default::default()
    };

    ds.tasks
        .insert(subjob_task.id.clone().unwrap(), subjob_task.clone());

    let scheduler = Scheduler::new(ds.clone(), Arc::new(broker));
    scheduler
        .schedule_subjob_task(subjob_task.clone())
        .await
        .unwrap();

    let parent = ds
        .tasks
        .get(&twerk_core::id::TaskId::new("task-subjob-1").unwrap());
    assert!(parent.is_some());
    assert_eq!(parent.unwrap().state, TaskState::Running);

    let subjob_count = ds.jobs.iter().count();
    assert!(subjob_count >= 1);
}

#[tokio::test]
async fn test_schedule_task_dispatches_to_parallel() {
    let ds = Arc::new(MockDatastore::new());
    let broker = InMemoryBroker::new();

    let job = create_test_job();
    ds.jobs.insert(job.id.clone().unwrap(), job.clone());

    let mut task = create_test_task();
    task.id = Some(twerk_core::id::TaskId::new("task-dispatch-parallel").unwrap());
    task.parallel = Some(ParallelTask {
        tasks: Some(vec![Task {
            id: None,
            name: Some("Parallel Child".to_string()),
            run: Some("echo parallel".to_string()),
            ..Default::default()
        }]),
        completions: 1,
    });

    ds.tasks.insert(task.id.clone().unwrap(), task.clone());

    let scheduler = Scheduler::new(ds.clone(), Arc::new(broker));
    scheduler.schedule_task(task.clone()).await.unwrap();

    let stored = ds
        .tasks
        .get(&twerk_core::id::TaskId::new("task-dispatch-parallel").unwrap());
    assert!(stored.is_some());
    assert_eq!(stored.unwrap().state, TaskState::Running);
}

#[tokio::test]
async fn test_schedule_task_dispatches_to_each() {
    let ds = Arc::new(MockDatastore::new());
    let broker = InMemoryBroker::new();

    let job = create_test_job();
    ds.jobs.insert(job.id.clone().unwrap(), job.clone());

    let mut task = create_test_task();
    task.id = Some(twerk_core::id::TaskId::new("task-dispatch-each").unwrap());
    task.each = Some(Box::new(EachTask {
        var: Some("i".to_string()),
        list: Some(r"[1, 2]".to_string()),
        task: Some(Box::new(Task {
            id: None,
            name: Some("Each Child".to_string()),
            run: Some("echo {{i}}".to_string()),
            ..Default::default()
        })),
        size: 0,
        completions: 0,
        concurrency: 0,
        index: 0,
    }));

    ds.tasks.insert(task.id.clone().unwrap(), task.clone());

    let scheduler = Scheduler::new(ds.clone(), Arc::new(broker));
    scheduler.schedule_task(task.clone()).await.unwrap();

    let stored = ds
        .tasks
        .get(&twerk_core::id::TaskId::new("task-dispatch-each").unwrap());
    assert!(stored.is_some());
    assert_eq!(stored.unwrap().state, TaskState::Running);
}

#[tokio::test]
async fn test_schedule_task_dispatches_to_subjob() {
    let ds = Arc::new(MockDatastore::new());
    let broker = InMemoryBroker::new();

    let job = create_test_job();
    ds.jobs.insert(job.id.clone().unwrap(), job.clone());

    let mut task = create_test_task();
    task.id = Some(twerk_core::id::TaskId::new("task-dispatch-subjob").unwrap());
    task.subjob = Some(SubJobTask {
        name: Some("SubJob Dispatch Test".to_string()),
        tasks: Some(vec![]),
        ..Default::default()
    });

    ds.tasks.insert(task.id.clone().unwrap(), task.clone());

    let scheduler = Scheduler::new(ds.clone(), Arc::new(broker));
    scheduler.schedule_task(task.clone()).await.unwrap();

    let stored = ds
        .tasks
        .get(&twerk_core::id::TaskId::new("task-dispatch-subjob").unwrap());
    assert!(stored.is_some());
    assert_eq!(stored.unwrap().state, TaskState::Running);
}

#[tokio::test]
async fn test_schedule_task_dispatches_to_regular() {
    let ds = Arc::new(MockDatastore::new());
    let broker = InMemoryBroker::new();
    let job = create_test_job();
    ds.jobs.insert(job.id.clone().unwrap(), job);

    let mut task = create_test_task();
    task.id = Some(twerk_core::id::TaskId::new("task-dispatch-regular").unwrap());

    ds.tasks.insert(task.id.clone().unwrap(), task.clone());

    let scheduler = Scheduler::new(ds.clone(), Arc::new(broker));
    scheduler.schedule_task(task.clone()).await.unwrap();

    let stored = ds
        .tasks
        .get(&twerk_core::id::TaskId::new("task-dispatch-regular").unwrap());
    assert!(stored.is_some());
    assert_eq!(stored.unwrap().state, TaskState::Scheduled);
}

#[tokio::test]
async fn test_schedule_each_task_with_sequence_expression() {
    let ds = Arc::new(MockDatastore::new());
    let broker = InMemoryBroker::new();

    let job = create_test_job();
    ds.jobs.insert(job.id.clone().unwrap(), job.clone());

    let template = Task {
        id: None,
        name: Some("Each Item".to_string()),
        run: Some("echo {{item_value}}".to_string()),
        ..Default::default()
    };

    let mut task = create_test_task();
    task.id = Some(twerk_core::id::TaskId::new("task-each-seq").unwrap());
    task.each = Some(Box::new(EachTask {
        var: Some("item".to_string()),
        list: Some("{{ sequence(1,5) }}".to_string()),
        task: Some(Box::new(template)),
        size: 0,
        completions: 0,
        concurrency: 0,
        index: 0,
    }));

    ds.tasks.insert(task.id.clone().unwrap(), task.clone());

    let scheduler = Scheduler::new(ds.clone(), Arc::new(broker));
    let result = scheduler.schedule_each_task(task.clone()).await;

    assert!(
        result.is_ok(),
        "schedule_each_task failed: {:?}",
        result.err()
    );

    let child_count = ds
        .tasks
        .iter()
        .filter(|r| r.value().parent_id.is_some())
        .count();
    assert_eq!(
        child_count, 4,
        "expected 4 children from sequence(1,5), got {child_count}"
    );
}

/// Reproduces the exact YAML template from examples/each.yaml
#[tokio::test]
async fn test_schedule_each_task_with_real_yaml_template() {
    let ds = Arc::new(MockDatastore::new());
    let broker = InMemoryBroker::new();

    let job = create_test_job();
    ds.jobs.insert(job.id.clone().unwrap(), job.clone());

    // Exact template from examples/each.yaml "sample each task"
    let template = Task {
        id: None,
        name: Some("output task item".to_string()),
        var: Some("eachTask{{item_index}}".to_string()),
        image: Some("ubuntu:mantic".to_string()),
        env: Some({
            let mut m = HashMap::new();
            m.insert("ITEM".to_string(), "{{item_value}}".to_string());
            m
        }),
        run: Some("echo -n $ITEM > $TWERK_OUTPUT".to_string()),
        ..Default::default()
    };

    let mut task = create_test_task();
    task.id = Some(twerk_core::id::TaskId::new("task-each-yaml").unwrap());
    task.each = Some(Box::new(EachTask {
        var: Some("item".to_string()),
        list: Some("{{ sequence(1,5) }}".to_string()),
        task: Some(Box::new(template)),
        size: 0,
        completions: 0,
        concurrency: 0,
        index: 0,
    }));

    ds.tasks.insert(task.id.clone().unwrap(), task.clone());

    let scheduler = Scheduler::new(ds.clone(), Arc::new(broker));
    let result = scheduler.schedule_each_task(task.clone()).await;

    assert!(
        result.is_ok(),
        "schedule_each_task with real YAML template failed: {:?}",
        result.err()
    );

    let child_count = ds
        .tasks
        .iter()
        .filter(|r| r.value().parent_id.is_some())
        .count();
    assert_eq!(child_count, 4, "expected 4 children, got {child_count}");
}
