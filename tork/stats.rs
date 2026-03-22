//! Statistics and metrics types

use serde::{Deserialize, Serialize};

/// Metrics holds system-wide metrics
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct Metrics {
    /// Job-related metrics
    pub jobs: JobMetrics,
    /// Task-related metrics
    pub tasks: TaskMetrics,
    /// Node-related metrics
    pub nodes: NodeMetrics,
}

/// Job-related metrics
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct JobMetrics {
    /// Number of running jobs
    #[serde(default)]
    pub running: i64,
}

/// Task-related metrics
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct TaskMetrics {
    /// Number of running tasks
    #[serde(default)]
    pub running: i64,
}

/// Node-related metrics
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct NodeMetrics {
    /// Number of online nodes
    #[serde(default, rename = "online")]
    pub running: i64,
    /// CPU usage percentage
    #[serde(default)]
    pub cpu_percent: f64,
}
