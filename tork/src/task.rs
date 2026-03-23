//! Domain types for task execution.
//!
//! This module provides the core task types used throughout the system.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use time::OffsetDateTime;

/// TaskState represents the list of states that a task can be in at any given moment.
pub type TaskState = &'static str;

pub const TASK_STATE_CREATED: TaskState = "CREATED";
pub const TASK_STATE_PENDING: TaskState = "PENDING";
pub const TASK_STATE_SCHEDULED: TaskState = "SCHEDULED";
pub const TASK_STATE_RUNNING: TaskState = "RUNNING";
pub const TASK_STATE_CANCELLED: TaskState = "CANCELLED";
pub const TASK_STATE_STOPPED: TaskState = "STOPPED";
pub const TASK_STATE_COMPLETED: TaskState = "COMPLETED";
pub const TASK_STATE_FAILED: TaskState = "FAILED";
pub const TASK_STATE_SKIPPED: TaskState = "SKIPPED";

pub const TASK_STATE_ACTIVE: &[TaskState] = &[
    TASK_STATE_CREATED,
    TASK_STATE_PENDING,
    TASK_STATE_SCHEDULED,
    TASK_STATE_RUNNING,
];

/// Task is the basic unit of work that a Worker can handle.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Task {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub job_id: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub parent_id: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub position: Option<i32>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub state: Option<TaskState>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub created_at: Option<OffsetDateTime>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub scheduled_at: Option<OffsetDateTime>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub started_at: Option<OffsetDateTime>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub completed_at: Option<OffsetDateTime>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub failed_at: Option<OffsetDateTime>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub cmd: Option<Vec<String>>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub entrypoint: Option<Vec<String>>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub run: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub image: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub registry: Option<Registry>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub env: Option<HashMap<String, String>>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub files: Option<HashMap<String, String>>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub queue: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub redelivered: Option<i32>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub pre: Option<Vec<Task>>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub post: Option<Vec<Task>>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub sidecars: Option<Vec<Task>>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub mounts: Option<Vec<Mount>>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub networks: Option<Vec<String>>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub node_id: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub retry: Option<TaskRetry>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub limits: Option<TaskLimits>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub timeout: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub var: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub r#if: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub parallel: Option<ParallelTask>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub each: Option<EachTask>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub subjob: Option<SubJobTask>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub gpus: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub tags: Option<Vec<String>>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub workdir: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub priority: Option<i32>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub progress: Option<f64>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub probe: Option<Probe>,
}

impl Task {
    /// Returns true if the task is in an active state.
    #[must_use]
    pub fn is_active(&self) -> bool {
        self.state
            .map(|state| TASK_STATE_ACTIVE.contains(&state))
            .unwrap_or(false)
    }

    /// Creates a deep clone of the task.
    #[must_use]
    pub fn clone(&self) -> Self {
        Self {
            id: self.id.clone(),
            job_id: self.job_id.clone(),
            parent_id: self.parent_id.clone(),
            position: self.position,
            name: self.name.clone(),
            description: self.description.clone(),
            state: self.state,
            created_at: self.created_at,
            scheduled_at: self.scheduled_at,
            started_at: self.started_at,
            completed_at: self.completed_at,
            failed_at: self.failed_at,
            cmd: self.cmd.clone(),
            entrypoint: self.entrypoint.clone(),
            run: self.run.clone(),
            image: self.image.clone(),
            registry: self.registry.clone(),
            env: self.env.clone(),
            files: self.files.clone(),
            queue: self.queue.clone(),
            redelivered: self.redelivered,
            error: self.error.clone(),
            pre: self.pre.as_ref().map(clone_tasks),
            post: self.post.as_ref().map(clone_tasks),
            sidecars: self.sidecars.as_ref().map(clone_tasks),
            mounts: self.mounts.as_ref().map(clone_mounts),
            networks: self.networks.clone(),
            node_id: self.node_id.clone(),
            retry: self.retry.clone(),
            limits: self.limits.clone(),
            timeout: self.timeout.clone(),
            result: self.result.clone(),
            var: self.var.clone(),
            r#if: self.r#if.clone(),
            parallel: self.parallel.clone(),
            each: self.each.clone(),
            subjob: self.subjob.clone(),
            gpus: self.gpus.clone(),
            tags: self.tags.clone(),
            workdir: self.workdir.clone(),
            priority: self.priority,
            progress: self.progress,
            probe: self.probe.clone(),
        }
    }
}

/// Creates a deep clone of a slice of tasks.
fn clone_tasks(tasks: &[Task]) -> Vec<Task> {
    tasks.iter().map(Task::clone).collect()
}

/// TaskSummary provides a summary view of a task.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TaskSummary {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub job_id: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub position: Option<i32>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub progress: Option<f64>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub state: Option<TaskState>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub created_at: Option<OffsetDateTime>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub scheduled_at: Option<OffsetDateTime>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub started_at: Option<OffsetDateTime>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub completed_at: Option<OffsetDateTime>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub var: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub tags: Option<Vec<String>>,
}

impl TaskSummary {
    /// Creates a TaskSummary from a Task.
    #[must_use]
    pub fn from_task(t: &Task) -> Self {
        Self {
            id: t.id.clone(),
            job_id: t.job_id.clone(),
            position: t.position,
            progress: t.progress,
            name: t.name.clone(),
            description: t.description.clone(),
            state: t.state,
            created_at: t.created_at,
            scheduled_at: t.scheduled_at,
            started_at: t.started_at,
            completed_at: t.completed_at,
            error: t.error.clone(),
            result: t.result.clone(),
            var: t.var.clone(),
            tags: t.tags.clone(),
        }
    }
}

/// TaskLogPart represents a part of a task's log output.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TaskLogPart {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub number: Option<i32>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub task_id: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub contents: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub created_at: Option<OffsetDateTime>,
}

/// SubJobTask represents a sub-job task configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SubJobTask {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub tasks: Option<Vec<Task>>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub inputs: Option<HashMap<String, String>>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub secrets: Option<HashMap<String, String>>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub auto_delete: Option<AutoDelete>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub output: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub detached: Option<bool>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub webhooks: Option<Vec<Webhook>>,
}

impl SubJobTask {
    /// Creates a deep clone of the SubJobTask.
    #[must_use]
    pub fn clone(&self) -> Self {
        Self {
            id: self.id.clone(),
            name: self.name.clone(),
            description: self.description.clone(),
            tasks: self.tasks.as_ref().map(clone_tasks),
            inputs: self.inputs.clone(),
            secrets: self.secrets.clone(),
            auto_delete: self.auto_delete.clone(),
            output: self.output.clone(),
            detached: self.detached,
            webhooks: self.webhooks.as_ref().map(clone_webhooks),
        }
    }
}

/// ParallelTask represents a task that runs other tasks in parallel.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ParallelTask {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tasks: Option<Vec<Task>>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub completions: Option<i32>,
}

impl ParallelTask {
    /// Creates a deep clone of the ParallelTask.
    #[must_use]
    pub fn clone(&self) -> Self {
        Self {
            tasks: self.tasks.as_ref().map(clone_tasks),
            completions: self.completions,
        }
    }
}

/// EachTask represents a task that iterates over a list.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EachTask {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub var: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub list: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub task: Option<Task>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub size: Option<i32>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub completions: Option<i32>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub concurrency: Option<i32>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub index: Option<i32>,
}

impl EachTask {
    /// Creates a deep clone of the EachTask.
    #[must_use]
    pub fn clone(&self) -> Self {
        Self {
            var: self.var.clone(),
            list: self.list.clone(),
            task: self.task.as_ref().map(Task::clone),
            size: self.size,
            completions: self.completions,
            concurrency: self.concurrency,
            index: self.index,
        }
    }
}

/// TaskRetry defines retry configuration for a task.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TaskRetry {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub limit: Option<i32>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub attempts: Option<i32>,
}

impl TaskRetry {
    /// Creates a deep clone of the TaskRetry.
    #[must_use]
    pub fn clone(&self) -> Self {
        Self {
            limit: self.limit,
            attempts: self.attempts,
        }
    }
}

/// TaskLimits defines resource limits for a task.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TaskLimits {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cpus: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub memory: Option<String>,
}

impl TaskLimits {
    /// Creates a deep clone of the TaskLimits.
    #[must_use]
    pub fn clone(&self) -> Self {
        Self {
            cpus: self.cpus.clone(),
            memory: self.memory.clone(),
        }
    }
}

/// Registry defines container registry credentials.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Registry {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub username: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub password: Option<String>,
}

impl Registry {
    /// Creates a deep clone of the Registry.
    #[must_use]
    pub fn clone(&self) -> Self {
        Self {
            username: self.username.clone(),
            password: self.password.clone(),
        }
    }
}

/// Probe defines health check configuration for a task.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Probe {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub port: Option<i32>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub timeout: Option<String>,
}

impl Probe {
    /// Creates a deep clone of the Probe.
    #[must_use]
    pub fn clone(&self) -> Self {
        Self {
            path: self.path.clone(),
            port: self.port,
            timeout: self.timeout.clone(),
        }
    }
}

/// Mount represents a filesystem mount configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Mount {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde(rename = "type")]
    pub mount_type: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub source: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub target: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub opts: Option<HashMap<String, String>>,
}

impl Mount {
    /// Creates a deep clone of the Mount.
    #[must_use]
    pub fn clone(&self) -> Self {
        Self {
            id: self.id.clone(),
            mount_type: self.mount_type.clone(),
            source: self.source.clone(),
            target: self.target.clone(),
            opts: self.opts.clone(),
        }
    }
}

/// Creates a deep clone of a slice of Mounts.
fn clone_mounts(mounts: &[Mount]) -> Vec<Mount> {
    mounts.iter().map(Mount::clone).collect()
}

/// AutoDelete defines automatic cleanup configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AutoDelete {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub after: Option<String>,
}

impl AutoDelete {
    /// Creates a deep clone of the AutoDelete.
    #[must_use]
    pub fn clone(&self) -> Self {
        Self {
            after: self.after.clone(),
        }
    }
}

/// Webhook defines webhook notification configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Webhook {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub headers: Option<HashMap<String, String>>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub event: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub r#if: Option<String>,
}

impl Webhook {
    /// Creates a deep clone of the Webhook.
    #[must_use]
    pub fn clone(&self) -> Self {
        Self {
            url: self.url.clone(),
            headers: self.headers.clone(),
            event: self.event.clone(),
            r#if: self.r#if.clone(),
        }
    }
}

/// Creates a deep clone of a slice of Webhooks.
fn clone_webhooks(webhooks: &[Webhook]) -> Vec<Webhook> {
    webhooks.iter().map(Webhook::clone).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_task_is_active() {
        let task = Task {
            id: Some("test-1".to_string()),
            state: Some(TASK_STATE_RUNNING),
            ..Default::default()
        };
        assert!(task.is_active());

        let completed_task = Task {
            id: Some("test-2".to_string()),
            state: Some(TASK_STATE_COMPLETED),
            ..Default::default()
        };
        assert!(!completed_task.is_active());
    }

    #[test]
    fn test_task_is_active_with_no_state() {
        let task = Task {
            id: Some("test-1".to_string()),
            state: None,
            ..Default::default()
        };
        assert!(!task.is_active());
    }

    #[test]
    fn test_task_clone() {
        let task = Task {
            id: Some("test-1".to_string()),
            job_id: Some("job-1".to_string()),
            name: Some("Test Task".to_string()),
            state: Some(TASK_STATE_PENDING),
            cmd: Some(vec!["echo".to_string(), "hello".to_string()]),
            env: Some(
                [("KEY".to_string(), "VALUE".to_string())]
                    .into_iter()
                    .collect(),
            ),
            ..Default::default()
        };

        let cloned = task.clone();

        assert_eq!(cloned.id, task.id);
        assert_eq!(cloned.job_id, task.job_id);
        assert_eq!(cloned.name, task.name);
        assert_eq!(cloned.state, task.state);
        assert_eq!(cloned.cmd, task.cmd);
        assert_eq!(cloned.env, task.env);
    }

    #[test]
    fn test_task_clone_nested() {
        let task = Task {
            id: Some("parent".to_string()),
            pre: Some(vec![Task {
                id: Some("child-1".to_string()),
                ..Default::default()
            }]),
            retry: Some(TaskRetry {
                limit: Some(3),
                attempts: Some(1),
            }),
            ..Default::default()
        };

        let cloned = task.clone();

        assert!(cloned.pre.is_some());
        assert!(cloned.pre.as_ref().unwrap()[0].id == Some("child-1".to_string()));
        assert!(cloned.retry.as_ref().unwrap().limit == Some(3));
    }

    #[test]
    fn test_task_summary_from_task() {
        let task = Task {
            id: Some("task-1".to_string()),
            job_id: Some("job-1".to_string()),
            name: Some("Test Task".to_string()),
            state: Some(TASK_STATE_COMPLETED),
            progress: Some(100.0),
            tags: Some(vec!["tag1".to_string(), "tag2".to_string()]),
            ..Default::default()
        };

        let summary = TaskSummary::from_task(&task);

        assert_eq!(summary.id, task.id);
        assert_eq!(summary.job_id, task.job_id);
        assert_eq!(summary.name, task.name);
        assert_eq!(summary.state, task.state);
        assert_eq!(summary.progress, task.progress);
        assert_eq!(summary.tags, task.tags);
    }

    #[test]
    fn test_task_retry_clone() {
        let retry = TaskRetry {
            limit: Some(5),
            attempts: Some(2),
        };

        let cloned = retry.clone();

        assert_eq!(cloned.limit, retry.limit);
        assert_eq!(cloned.attempts, retry.attempts);
    }

    #[test]
    fn test_task_limits_clone() {
        let limits = TaskLimits {
            cpus: Some("2".to_string()),
            memory: Some("4Gi".to_string()),
        };

        let cloned = limits.clone();

        assert_eq!(cloned.cpus, limits.cpus);
        assert_eq!(cloned.memory, limits.memory);
    }

    #[test]
    fn test_registry_clone() {
        let registry = Registry {
            username: Some("user".to_string()),
            password: Some("pass".to_string()),
        };

        let cloned = registry.clone();

        assert_eq!(cloned.username, registry.username);
        assert_eq!(cloned.password, registry.password);
    }

    #[test]
    fn test_probe_clone() {
        let probe = Probe {
            path: Some("/health".to_string()),
            port: Some(8080),
            timeout: Some("5s".to_string()),
        };

        let cloned = probe.clone();

        assert_eq!(cloned.path, probe.path);
        assert_eq!(cloned.port, probe.port);
        assert_eq!(cloned.timeout, probe.timeout);
    }

    #[test]
    fn test_parallel_task_clone() {
        let task = ParallelTask {
            tasks: Some(vec![
                Task {
                    id: Some("p1".to_string()),
                    ..Default::default()
                },
                Task {
                    id: Some("p2".to_string()),
                    ..Default::default()
                },
            ]),
            completions: Some(2),
        };

        let cloned = task.clone();

        assert!(cloned.tasks.is_some());
        assert_eq!(cloned.tasks.as_ref().unwrap().len(), 2);
        assert_eq!(cloned.completions, task.completions);
    }

    #[test]
    fn test_each_task_clone() {
        let each = EachTask {
            var: Some("item".to_string()),
            list: Some("items".to_string()),
            task: Some(Task {
                id: Some("loop-task".to_string()),
                ..Default::default()
            }),
            size: Some(10),
            completions: Some(5),
            concurrency: Some(2),
            index: Some(0),
        };

        let cloned = each.clone();

        assert_eq!(cloned.var, each.var);
        assert_eq!(cloned.list, each.list);
        assert!(cloned.task.is_some());
        assert_eq!(cloned.size, each.size);
        assert_eq!(cloned.completions, each.completions);
        assert_eq!(cloned.concurrency, each.concurrency);
        assert_eq!(cloned.index, each.index);
    }

    #[test]
    fn test_subjob_task_clone() {
        let subjob = SubJobTask {
            id: Some("sj-1".to_string()),
            name: Some("SubJob".to_string()),
            tasks: Some(vec![Task {
                id: Some("subtask-1".to_string()),
                ..Default::default()
            }]),
            inputs: Some(
                [("input1".to_string(), "val1".to_string())]
                    .into_iter()
                    .collect(),
            ),
            secrets: Some(
                [("secret1".to_string(), "val1".to_string())]
                    .into_iter()
                    .collect(),
            ),
            auto_delete: Some(AutoDelete {
                after: Some("1h".to_string()),
            }),
            output: Some("output".to_string()),
            detached: Some(true),
            webhooks: Some(vec![Webhook {
                url: Some("https://example.com".to_string()),
                ..Default::default()
            }]),
        };

        let cloned = subjob.clone();

        assert_eq!(cloned.id, subjob.id);
        assert_eq!(cloned.name, subjob.name);
        assert!(cloned.tasks.is_some());
        assert!(cloned.inputs.is_some());
        assert!(cloned.secrets.is_some());
        assert!(cloned.auto_delete.is_some());
        assert_eq!(cloned.output, subjob.output);
        assert_eq!(cloned.detached, subjob.detached);
        assert!(cloned.webhooks.is_some());
    }

    #[test]
    fn test_mount_clone() {
        let mount = Mount {
            id: Some("mount-1".to_string()),
            mount_type: Some("volume".to_string()),
            source: Some("/source".to_string()),
            target: Some("/target".to_string()),
            opts: Some(
                [("readonly".to_string(), "true".to_string())]
                    .into_iter()
                    .collect(),
            ),
        };

        let cloned = mount.clone();

        assert_eq!(cloned.id, mount.id);
        assert_eq!(cloned.mount_type, mount.mount_type);
        assert_eq!(cloned.source, mount.source);
        assert_eq!(cloned.target, mount.target);
        assert_eq!(cloned.opts, mount.opts);
    }

    #[test]
    fn test_webhook_clone() {
        let webhook = Webhook {
            url: Some("https://example.com/hook".to_string()),
            headers: Some(
                [("Content-Type".to_string(), "application/json".to_string())]
                    .into_iter()
                    .collect(),
            ),
            event: Some("task.completed".to_string()),
            r#if: Some("task.state == 'COMPLETED'".to_string()),
        };

        let cloned = webhook.clone();

        assert_eq!(cloned.url, webhook.url);
        assert_eq!(cloned.headers, webhook.headers);
        assert_eq!(cloned.event, webhook.event);
        assert_eq!(cloned.r#if, webhook.r#if);
    }

    #[test]
    fn test_auto_delete_clone() {
        let auto_delete = AutoDelete {
            after: Some("24h".to_string()),
        };

        let cloned = auto_delete.clone();

        assert_eq!(cloned.after, auto_delete.after);
    }

    #[test]
    fn test_clone_tasks_empty() {
        let tasks: Vec<Task> = vec![];
        let cloned = clone_tasks(&tasks);
        assert!(cloned.is_empty());
    }

    #[test]
    fn test_clone_webhooks_empty() {
        let webhooks: Vec<Webhook> = vec![];
        let cloned = clone_webhooks(&webhooks);
        assert!(cloned.is_empty());
    }

    #[test]
    fn test_clone_mounts_empty() {
        let mounts: Vec<Mount> = vec![];
        let cloned = clone_mounts(&mounts);
        assert!(cloned.is_empty());
    }

    #[test]
    fn test_task_state_constants() {
        assert_eq!(TASK_STATE_CREATED, "CREATED");
        assert_eq!(TASK_STATE_PENDING, "PENDING");
        assert_eq!(TASK_STATE_SCHEDULED, "SCHEDULED");
        assert_eq!(TASK_STATE_RUNNING, "RUNNING");
        assert_eq!(TASK_STATE_CANCELLED, "CANCELLED");
        assert_eq!(TASK_STATE_STOPPED, "STOPPED");
        assert_eq!(TASK_STATE_COMPLETED, "COMPLETED");
        assert_eq!(TASK_STATE_FAILED, "FAILED");
        assert_eq!(TASK_STATE_SKIPPED, "SKIPPED");
    }

    #[test]
    fn test_task_state_active_contains_expected_states() {
        assert!(TASK_STATE_ACTIVE.contains(&TASK_STATE_CREATED));
        assert!(TASK_STATE_ACTIVE.contains(&TASK_STATE_PENDING));
        assert!(TASK_STATE_ACTIVE.contains(&TASK_STATE_SCHEDULED));
        assert!(TASK_STATE_ACTIVE.contains(&TASK_STATE_RUNNING));
        assert!(!TASK_STATE_ACTIVE.contains(&TASK_STATE_COMPLETED));
        assert!(!TASK_STATE_ACTIVE.contains(&TASK_STATE_FAILED));
    }
}

// Implement Default manually since we use Option fields throughout
impl Default for Task {
    fn default() -> Self {
        Self {
            id: None,
            job_id: None,
            parent_id: None,
            position: None,
            name: None,
            description: None,
            state: None,
            created_at: None,
            scheduled_at: None,
            started_at: None,
            completed_at: None,
            failed_at: None,
            cmd: None,
            entrypoint: None,
            run: None,
            image: None,
            registry: None,
            env: None,
            files: None,
            queue: None,
            redelivered: None,
            error: None,
            pre: None,
            post: None,
            sidecars: None,
            mounts: None,
            networks: None,
            node_id: None,
            retry: None,
            limits: None,
            timeout: None,
            result: None,
            var: None,
            r#if: None,
            parallel: None,
            each: None,
            subjob: None,
            gpus: None,
            tags: None,
            workdir: None,
            priority: None,
            progress: None,
            probe: None,
        }
    }
}
