use std::collections::HashMap;

use time::OffsetDateTime;
use twerk_core::id::{JobId, TaskId};
use twerk_core::job::{Job, JobState};
use twerk_core::node::{Node, NodeStatus};
use twerk_core::task::{Task, TaskState};
use twerk_web::api::trigger_api::{Trigger, TriggerId};

pub fn queued_task(name: &str) -> Task {
    Task {
        name: Some(name.to_string()),
        ..Default::default()
    }
}

pub fn job(id: &str, name: &str) -> Job {
    Job {
        id: Some(JobId::new(id).unwrap()),
        name: Some(name.to_string()),
        ..Default::default()
    }
}

pub fn job_with_state(id: &str, name: &str, state: JobState) -> Job {
    Job {
        state,
        ..job(id, name)
    }
}

pub fn direct_task(job_id: &str, task_id: &str, name: &str, state: TaskState) -> Task {
    Task {
        id: Some(TaskId::new(task_id).unwrap()),
        job_id: Some(JobId::new(job_id).unwrap()),
        name: Some(name.to_string()),
        state,
        ..Default::default()
    }
}

pub fn trigger(id: &str) -> Trigger {
    let now = OffsetDateTime::UNIX_EPOCH;

    Trigger {
        id: TriggerId::parse(id).expect("valid trigger id fixture"),
        name: "test-trigger".to_string(),
        enabled: true,
        event: "test.event".to_string(),
        condition: None,
        action: "test_action".to_string(),
        metadata: HashMap::new(),
        version: 1,
        created_at: now,
        updated_at: now,
    }
}

pub fn node(id: &str, name: &str) -> Node {
    Node {
        id: Some(twerk_core::id::NodeId::new(id).expect("valid node id fixture")),
        name: Some(name.to_string()),
        status: Some(NodeStatus::UP),
        cpu_percent: Some(0.0),
        hostname: Some("localhost".to_string()),
        queue: Some("default".to_string()),
        version: Some(env!("CARGO_PKG_VERSION").to_string()),
        last_heartbeat_at: Some(OffsetDateTime::now_utc()),
        ..Default::default()
    }
}
