use crate::id::NodeId;
use serde::{Deserialize, Serialize};
use time::{Duration, OffsetDateTime};
use utoipa::ToSchema;

pub const LAST_HEARTBEAT_TIMEOUT: Duration = Duration::minutes(5);
pub const HEARTBEAT_RATE: Duration = Duration::seconds(30);
pub const NODE_STATUS_UP: &str = "UP";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default, ToSchema)]
pub enum NodeStatus {
    #[default]
    UP,
    DOWN,
    OFFLINE,
}

impl From<&str> for NodeStatus {
    fn from(s: &str) -> Self {
        match s.to_uppercase().as_str() {
            "UP" => NodeStatus::UP,
            "DOWN" => NodeStatus::DOWN,
            _ => NodeStatus::OFFLINE,
        }
    }
}

impl AsRef<str> for NodeStatus {
    fn as_ref(&self) -> &str {
        match self {
            NodeStatus::UP => "UP",
            NodeStatus::DOWN => "DOWN",
            NodeStatus::OFFLINE => "OFFLINE",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct Node {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<NodeId>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub started_at: Option<OffsetDateTime>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cpu_percent: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_heartbeat_at: Option<OffsetDateTime>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub queue: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub status: Option<NodeStatus>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hostname: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub port: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub task_count: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub version: Option<String>,
}

impl Node {
    #[must_use]
    pub fn deep_clone(&self) -> Node {
        self.clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_node_clone() {
        let original = Node {
            id: Some(NodeId::new("node-1").unwrap()),
            name: Some("worker-1".to_string()),
            status: Some(NodeStatus::UP),
            ..Default::default()
        };

        let cloned = original.deep_clone();

        assert_eq!(original.id, cloned.id);
        assert_eq!(original.name, cloned.name);
        assert_eq!(original.status, cloned.status);
    }

    #[test]
    fn test_node_status_from_str() {
        assert_eq!(NodeStatus::from("UP"), NodeStatus::UP);
        assert_eq!(NodeStatus::from("down"), NodeStatus::DOWN);
        assert_eq!(NodeStatus::from("OFFLINE"), NodeStatus::OFFLINE);
        assert_eq!(NodeStatus::from("unknown"), NodeStatus::OFFLINE);
    }
}
