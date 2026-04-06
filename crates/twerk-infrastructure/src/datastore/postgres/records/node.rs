//! Node record types and conversions to domain types.

use sqlx::FromRow;

use crate::datastore::Error as DatastoreError;
use twerk_core::{
    id::NodeId,
    node::{Node, NodeStatus},
};

/// Node record from the database
#[derive(Debug, Clone, FromRow)]
pub struct NodeRecord {
    pub id: String,
    pub name: String,
    pub started_at: time::OffsetDateTime,
    pub last_heartbeat_at: time::OffsetDateTime,
    pub cpu_percent: f64,
    pub queue: String,
    pub status: String,
    pub hostname: String,
    pub port: i64,
    pub task_count: i64,
    pub version_: String,
}

/// Extension trait for NodeRecord conversions
pub trait NodeRecordExt {
    /// Converts the database record to a Node domain object.
    fn to_node(&self) -> Result<Node, DatastoreError>;
}

impl NodeRecordExt for NodeRecord {
    fn to_node(&self) -> Result<Node, DatastoreError> {
        let now = time::OffsetDateTime::now_utc();
        let heartbeat_timeout = now - time::Duration::seconds(2 * 30); // 2 * HEARTBEAT_RATE
        let status = if self.last_heartbeat_at < heartbeat_timeout && self.status == "UP" {
            NodeStatus::from("OFFLINE")
        } else {
            NodeStatus::from(self.status.as_str())
        };

        Ok(Node {
            id: Some(NodeId::new(self.id.clone())?),
            name: Some(self.name.clone()),
            started_at: Some(self.started_at),
            cpu_percent: Some(self.cpu_percent),
            last_heartbeat_at: Some(self.last_heartbeat_at),
            queue: Some(self.queue.clone()),
            status: Some(status),
            hostname: Some(self.hostname.clone()),
            port: Some(self.port),
            task_count: Some(self.task_count),
            version: Some(self.version_.clone()),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::super::helpers::fixed_now;
    use super::*;
    use twerk_core::node::NODE_STATUS_UP;

    // ── Helpers ──────────────────────────────────────────────────────────

    fn base_node_record() -> NodeRecord {
        let now = time::OffsetDateTime::now_utc();
        NodeRecord {
            id: "node-001".to_string(),
            name: "worker-1".to_string(),
            started_at: now,
            last_heartbeat_at: now, // recent heartbeat
            cpu_percent: 45.5,
            queue: "default".to_string(),
            status: NODE_STATUS_UP.to_string(),
            hostname: "worker-1.local".to_string(),
            port: 8080,
            task_count: 3,
            version_: "1.0.0".to_string(),
        }
    }

    // ── NodeRecord → Node conversion tests ──────────────────────────────

    #[test]
    fn node_record_to_node_basic_fields() {
        let record = base_node_record();
        let node = record.to_node().expect("conversion should succeed");

        assert_eq!(node.id.as_deref(), Some("node-001"));
        assert_eq!(node.name.as_deref(), Some("worker-1"));
        assert_eq!(node.hostname.as_deref(), Some("worker-1.local"));
        assert_eq!(node.port, Some(8080));
        assert_eq!(node.task_count, Some(3));
        assert_eq!(node.version, Some("1.0.0".to_string()));
        assert_eq!(node.queue.as_deref(), Some("default"));
    }

    #[test]
    fn node_record_to_node_recent_heartbeat_stays_up() {
        // Node with recent heartbeat should keep its UP status
        let record = base_node_record();
        let node = record.to_node().expect("conversion should succeed");

        assert_eq!(node.status, Some(NodeStatus::UP));
    }

    #[test]
    fn node_record_to_node_stale_heartbeat_goes_offline() {
        // Node with stale heartbeat (>60s old) and UP status should become OFFLINE
        let stale = fixed_now() - time::Duration::seconds(120);
        let record = NodeRecord {
            last_heartbeat_at: stale,
            status: NODE_STATUS_UP.to_string(),
            ..base_node_record()
        };
        let node = record.to_node().expect("conversion should succeed");

        assert_eq!(node.status, Some(NodeStatus::OFFLINE));
    }

    #[test]
    fn node_record_to_node_non_up_status_preserved() {
        // A DOWN node should stay DOWN regardless of heartbeat
        let stale = fixed_now() - time::Duration::seconds(120);
        let record = NodeRecord {
            last_heartbeat_at: stale,
            status: "DOWN".to_string(),
            ..base_node_record()
        };
        let node = record.to_node().expect("conversion should succeed");

        // Only UP transitions to OFFLINE on stale heartbeat
        assert_eq!(node.status, Some(NodeStatus::DOWN));
    }

    #[test]
    fn node_record_to_node_cpu_percent_preserved() {
        let record = NodeRecord {
            cpu_percent: 99.9,
            ..base_node_record()
        };
        let node = record.to_node().expect("conversion should succeed");
        assert!((node.cpu_percent.unwrap_or(0.0) - 99.9).abs() < f64::EPSILON);
    }
}
