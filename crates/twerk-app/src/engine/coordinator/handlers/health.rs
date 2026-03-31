//! Health and node-related event handlers

use anyhow::Result;
use std::sync::Arc;
use tracing::warn;
use twerk_core::node::NodeStatus;

/// Handles node heartbeat.
///
/// # Errors
/// Returns error if node update fails.
pub async fn handle_heartbeat(
    ds: Arc<dyn twerk_infrastructure::datastore::Datastore>,
    _broker: Arc<dyn twerk_infrastructure::broker::Broker>,
    node: twerk_core::node::Node,
) -> Result<()> {
    if let Some(node_id) = &node.id {
        let node_id_str = node_id.to_string();
        ds.update_node(
            &node_id_str,
            Box::new(move |mut u: twerk_core::node::Node| {
                u.last_heartbeat_at = node.last_heartbeat_at;
                u.cpu_percent = node.cpu_percent;
                u.task_count = node.task_count;
                u.status = Some(NodeStatus::UP);
                Ok(u)
            }),
        )
        .await
        .map_err(anyhow::Error::from)
    } else {
        warn!("Received heartbeat from node without ID, creating new node");
        let mut new_node = node;
        new_node.status = Some(NodeStatus::UP);
        new_node.last_heartbeat_at = Some(time::OffsetDateTime::now_utc());
        ds.create_node(&new_node).await.map_err(anyhow::Error::from)
    }
}
