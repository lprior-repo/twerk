use serde::{Deserialize, Serialize};
use time::OffsetDateTime;

pub const LAST_HEARTBEAT_TIMEOUT: time::Duration = time::Duration::MINUTE * 5;
pub const HEARTBEAT_RATE: time::Duration = time::Duration::SECOND * 30;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct NodeStatus;

impl NodeStatus {
    pub const UP: &'static str = "UP";
    pub const DOWN: &'static str = "DOWN";
    pub const OFFLINE: &'static str = "OFFLINE";
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Node {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub started_at: Option<OffsetDateTime>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cpu_percent: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_heartbeat_at: Option<OffsetDateTime>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub queue: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub status: Option<NodeStatus>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hostname: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub port: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub task_count: Option<i32>,
    pub version: Option<String>,
}

impl Node {
    #[must_use]
    pub fn clone(&self) -> Node {
        Node {
            id: self.id.clone(),
            name: self.name.clone(),
            started_at: self.started_at,
            cpu_percent: self.cpu_percent,
            last_heartbeat_at: self.last_heartbeat_at,
            queue: self.queue.clone(),
            status: self.status,
            hostname: self.hostname.clone(),
            port: self.port,
            task_count: self.task_count,
            version: self.version.clone(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_node_clone() {
        let original = Node {
            id: Some("node-1".to_string()),
            name: Some("worker-1".to_string()),
            started_at: Some(OffsetDateTime::now()),
            cpu_percent: Some(45.5),
            last_heartbeat_at: Some(OffsetDateTime::now()),
            queue: Some("default".to_string()),
            status: Some(NodeStatus),
            hostname: Some("localhost".to_string()),
            port: Some(8080),
            task_count: Some(10),
            version: Some("1.0.0".to_string()),
        };

        let cloned = original.clone();

        assert_eq!(original.id, cloned.id);
        assert_eq!(original.name, cloned.name);
        assert_eq!(original.started_at, cloned.started_at);
        assert_eq!(original.cpu_percent, cloned.cpu_percent);
        assert_eq!(original.last_heartbeat_at, cloned.last_heartbeat_at);
        assert_eq!(original.queue, cloned.queue);
        assert_eq!(original.status, cloned.status);
        assert_eq!(original.hostname, cloned.hostname);
        assert_eq!(original.port, cloned.port);
        assert_eq!(original.task_count, cloned.task_count);
        assert_eq!(original.version, cloned.version);

        // Verify it's a true copy (different heap allocations)
        drop(cloned);
        assert!(original.id.is_some());
    }

    #[test]
    fn test_node_clone_preserves_none_fields() {
        let original = Node {
            id: None,
            name: None,
            started_at: None,
            cpu_percent: None,
            last_heartbeat_at: None,
            queue: None,
            status: None,
            hostname: None,
            port: None,
            task_count: None,
            version: None,
        };

        let cloned = original.clone();

        assert!(cloned.id.is_none());
        assert!(cloned.name.is_none());
        assert!(cloned.started_at.is_none());
        assert!(cloned.cpu_percent.is_none());
        assert!(cloned.last_heartbeat_at.is_none());
        assert!(cloned.queue.is_none());
        assert!(cloned.status.is_none());
        assert!(cloned.hostname.is_none());
        assert!(cloned.port.is_none());
        assert!(cloned.task_count.is_none());
        assert!(cloned.version.is_none());
    }

    #[test]
    fn test_node_status_constants() {
        assert_eq!(NodeStatus::UP, "UP");
        assert_eq!(NodeStatus::DOWN, "DOWN");
        assert_eq!(NodeStatus::OFFLINE, "OFFLINE");
    }
}
