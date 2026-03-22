//! Node-related domain types

use serde::{Deserialize, Serialize};
use time::OffsetDateTime;

/// Timeout for last heartbeat (5 minutes)
pub const LAST_HEARTBEAT_TIMEOUT_SECS: i64 = 5 * 60;
/// Heartbeat rate (30 seconds)
pub const HEARTBEAT_RATE_SECS: i64 = 30;

/// NodeStatus represents the status of a node
pub type NodeStatus = String;

/// Node is online
pub const NODE_STATUS_UP: &str = "UP";
/// Node is down
pub const NODE_STATUS_DOWN: &str = "DOWN";
/// Node is offline
pub const NODE_STATUS_OFFLINE: &str = "OFFLINE";

impl Node {
    /// Creates a new Node with the given id and name.
    #[must_use]
    pub fn new() -> Self {
        let now = OffsetDateTime::now_utc();
        Self {
            id: None,
            name: None,
            started_at: now,
            cpu_percent: 0.0,
            last_heartbeat_at: now,
            queue: None,
            status: String::new(),
            hostname: None,
            port: 0,
            task_count: 0,
            version: String::new(),
        }
    }

    /// Creates a deep clone of this node
    #[must_use]
    pub fn deep_clone(&self) -> Self {
        Self {
            id: self.id.clone(),
            name: self.name.clone(),
            started_at: self.started_at,
            cpu_percent: self.cpu_percent,
            last_heartbeat_at: self.last_heartbeat_at,
            queue: self.queue.clone(),
            status: self.status.clone(),
            hostname: self.hostname.clone(),
            port: self.port,
            task_count: self.task_count,
            version: self.version.clone(),
        }
    }
}

/// Node represents a worker node in the cluster
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Node {
    /// Unique identifier
    pub id: Option<String>,
    /// Node name
    pub name: Option<String>,
    /// When the node started
    pub started_at: OffsetDateTime,
    /// CPU usage percentage
    pub cpu_percent: f64,
    /// Last heartbeat timestamp
    pub last_heartbeat_at: OffsetDateTime,
    /// Queue this node belongs to
    #[serde(skip_serializing_if = "Option::is_none")]
    pub queue: Option<String>,
    /// Node status
    #[serde(default)]
    pub status: NodeStatus,
    /// Hostname of the node
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hostname: Option<String>,
    /// Port the node is listening on
    #[serde(default)]
    pub port: i64,
    /// Number of tasks currently running
    #[serde(default)]
    pub task_count: i64,
    /// Node version
    #[serde(default)]
    pub version: String,
}
