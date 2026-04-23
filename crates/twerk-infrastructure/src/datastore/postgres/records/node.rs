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


