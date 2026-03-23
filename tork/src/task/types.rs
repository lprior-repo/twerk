//! Task types for the tork runtime.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use time::OffsetDateTime;

use super::state::TaskState;

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

/// TaskSummary provides a summary view of a task.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TaskSummary {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub job_id: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub state: Option<TaskState>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub created_at: Option<OffsetDateTime>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub started_at: Option<OffsetDateTime>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub completed_at: Option<OffsetDateTime>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub progress: Option<f64>,
}

/// TaskLogPart represents a part of a task log.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TaskLogPart {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub timestamp: Option<DateTime<Utc>>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
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

    #[serde(default)]
    pub detached: bool,
}

/// ParallelTask defines parallel task configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ParallelTask {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tasks: Option<Vec<Task>>,

    #[serde(default)]
    pub completions: i32,
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

/// TaskRetry defines retry configuration for a task.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TaskRetry {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub limit: Option<i32>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub attempts: Option<i32>,
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

/// Registry holds container registry credentials.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Registry {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub username: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub password: Option<String>,
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

/// AutoDelete defines automatic deletion configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AutoDelete {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub after: Option<String>,
}

/// Webhook defines a webhook configuration.
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
