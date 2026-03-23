//! Supporting types for job domain.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use time::OffsetDateTime;

pub const MOUNT_TYPE_VOLUME: &str = "volume";
pub const MOUNT_TYPE_BIND: &str = "bind";
pub const MOUNT_TYPE_TMPFS: &str = "tmpfs";

/// User represents a user in the system.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct User {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub username: Option<String>,
    #[serde(skip)]
    pub password_hash: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub password: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub created_at: Option<OffsetDateTime>,
    #[serde(default)]
    pub disabled: bool,
}

impl User {
    #[must_use]
    pub fn clone(&self) -> Self {
        Self {
            id: self.id.clone(),
            name: self.name.clone(),
            username: self.username.clone(),
            password_hash: self.password_hash.clone(),
            password: self.password.clone(),
            created_at: self.created_at,
            disabled: self.disabled,
        }
    }
}

/// Role represents a role in the system.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Role {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub slug: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub created_at: Option<OffsetDateTime>,
}

impl Role {
    #[must_use]
    pub fn clone(&self) -> Self {
        Self {
            id: self.id.clone(),
            slug: self.slug.clone(),
            name: self.name.clone(),
            created_at: self.created_at,
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
    #[must_use]
    pub fn clone(&self) -> Self {
        Self {
            cpus: self.cpus.clone(),
            memory: self.memory.clone(),
        }
    }
}

/// Task represents a unit of work.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Task {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub job_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parent_id: Option<String>,
    #[serde(default)]
    pub position: i32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub state: Option<String>,
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
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub cmd: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub entrypoint: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub run: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub image: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub registry: Option<Box<Registry>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub env: Option<HashMap<String, String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub files: Option<HashMap<String, String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub queue: Option<String>,
    #[serde(default)]
    pub redelivered: i32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub pre: Vec<Task>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub post: Vec<Task>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub sidecars: Vec<Task>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub mounts: Vec<Mount>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub networks: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub node_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub retry: Option<Box<TaskRetry>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub limits: Option<Box<TaskLimits>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub timeout: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub var: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub r#if: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parallel: Option<Box<ParallelTask>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub each: Option<Box<EachTask>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub subjob: Option<Box<SubJobTask>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub gpus: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tags: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub workdir: Option<String>,
    #[serde(default)]
    pub priority: i32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub progress: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub probe: Option<Box<Probe>>,
}

impl Task {
    #[must_use]
    pub fn clone(&self) -> Self {
        Self {
            id: self.id.clone(),
            job_id: self.job_id.clone(),
            parent_id: self.parent_id.clone(),
            position: self.position,
            name: self.name.clone(),
            description: self.description.clone(),
            state: self.state.clone(),
            created_at: self.created_at,
            scheduled_at: self.scheduled_at,
            started_at: self.started_at,
            completed_at: self.completed_at,
            failed_at: self.failed_at,
            cmd: self.cmd.clone(),
            entrypoint: self.entrypoint.clone(),
            run: self.run.clone(),
            image: self.image.clone(),
            registry: self.registry.as_ref().map(|r| Box::new((**r).clone())),
            env: self.env.as_ref().map(HashMap::clone),
            files: self.files.as_ref().map(HashMap::clone),
            queue: self.queue.clone(),
            redelivered: self.redelivered,
            error: self.error.clone(),
            pre: self.pre.iter().map(Task::clone).collect(),
            post: self.post.iter().map(Task::clone).collect(),
            sidecars: self.sidecars.iter().map(Task::clone).collect(),
            mounts: self.mounts.iter().map(Mount::clone).collect(),
            networks: self.networks.clone(),
            node_id: self.node_id.clone(),
            retry: self.retry.as_ref().map(|r| Box::new((**r).clone())),
            limits: self.limits.as_ref().map(|l| Box::new((**l).clone())),
            timeout: self.timeout.clone(),
            result: self.result.clone(),
            var: self.var.clone(),
            r#if: self.r#if.clone(),
            parallel: self.parallel.as_ref().map(|p| Box::new((**p).clone())),
            each: self.each.as_ref().map(|e| Box::new((**e).clone())),
            subjob: self.subjob.as_ref().map(|s| Box::new((**s).clone())),
            gpus: self.gpus.clone(),
            tags: self.tags.clone(),
            workdir: self.workdir.clone(),
            priority: self.priority,
            progress: self.progress,
            probe: self.probe.as_ref().map(|p| Box::new((**p).clone())),
        }
    }
}

/// Registry holds container registry credentials.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Registry {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub username: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub password: Option<String>,
}

impl Registry {
    #[must_use]
    pub fn clone(&self) -> Self {
        Self {
            username: self.username.clone(),
            password: self.password.clone(),
        }
    }
}

/// Mount represents a filesystem mount.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Mount {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mount_type: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none", rename = "source")]
    pub source: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none", rename = "target")]
    pub target: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub opts: Option<HashMap<String, String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub read_only: Option<bool>,
}

impl Mount {
    #[must_use]
    pub fn clone(&self) -> Self {
        Self {
            mount_type: self.mount_type.clone(),
            source: self.source.clone(),
            target: self.target.clone(),
            opts: self.opts.clone(),
            read_only: self.read_only,
        }
    }
}

/// Probe defines health check configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Probe {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
    #[serde(default)]
    pub port: i32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub timeout: Option<String>,
}

impl Probe {
    #[must_use]
    pub fn clone(&self) -> Self {
        Self {
            path: self.path.clone(),
            port: self.port,
            timeout: self.timeout.clone(),
        }
    }
}

/// ParallelTask defines parallel task configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ParallelTask {
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tasks: Vec<Task>,
    #[serde(default)]
    pub completions: i32,
}

impl ParallelTask {
    #[must_use]
    pub fn clone(&self) -> Self {
        Self {
            tasks: self.tasks.iter().map(Task::clone).collect(),
            completions: self.completions,
        }
    }
}

/// EachTask defines foreach task configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EachTask {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub var: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub list: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub task: Option<Box<Task>>,
    #[serde(default)]
    pub size: i32,
    #[serde(default)]
    pub completions: i32,
    #[serde(default)]
    pub concurrency: i32,
    #[serde(default)]
    pub index: i32,
}

impl EachTask {
    #[must_use]
    pub fn clone(&self) -> Self {
        Self {
            var: self.var.clone(),
            list: self.list.clone(),
            task: self.task.as_ref().map(|t| Box::new((**t).clone())),
            size: self.size,
            completions: self.completions,
            concurrency: self.concurrency,
            index: self.index,
        }
    }
}

/// SubJobTask defines a subjob task configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SubJobTask {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tasks: Vec<Task>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub inputs: Option<HashMap<String, String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub secrets: Option<HashMap<String, String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub auto_delete: Option<Box<AutoDelete>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub output: Option<String>,
    #[serde(default)]
    pub detached: bool,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub webhooks: Vec<Webhook>,
}

impl SubJobTask {
    #[must_use]
    pub fn clone(&self) -> Self {
        Self {
            id: self.id.clone(),
            name: self.name.clone(),
            description: self.description.clone(),
            inputs: self.inputs.as_ref().map(HashMap::clone),
            secrets: self.secrets.as_ref().map(HashMap::clone),
            auto_delete: self.auto_delete.as_ref().map(|a| Box::new((**a).clone())),
            tasks: self.tasks.iter().map(Task::clone).collect(),
            output: self.output.clone(),
            detached: self.detached,
            webhooks: self.webhooks.iter().map(Webhook::clone).collect(),
        }
    }
}
