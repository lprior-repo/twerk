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
    let mut new_node = node;
    new_node.status = Some(NodeStatus::UP);
    if new_node.last_heartbeat_at.is_none() {
        new_node.last_heartbeat_at = Some(time::OffsetDateTime::now_utc());
    }

    if let Some(node_id) = &new_node.id {
        let node_id_str = node_id.to_string();
        let update_result = ds
            .update_node(
                &node_id_str,
                Box::new({
                    let heartbeat_at = new_node.last_heartbeat_at;
                    let cpu = new_node.cpu_percent;
                    let task_count = new_node.task_count;
                    move |mut u: twerk_core::node::Node| {
                        u.last_heartbeat_at = heartbeat_at;
                        u.cpu_percent = cpu;
                        u.task_count = task_count;
                        u.status = Some(NodeStatus::UP);
                        Ok(u)
                    }
                }),
            )
            .await;

        match update_result {
            Ok(()) => Ok(()),
            Err(e) if e.to_string().contains("not found") => {
                warn!(node_id = %node_id_str, "Node not found on update, creating new node");
                ds.create_node(&new_node).await.map_err(anyhow::Error::from)
            }
            Err(e) => Err(anyhow::Error::from(e)),
        }
    } else {
        warn!("Received heartbeat from node without ID, creating new node");
        ds.create_node(&new_node).await.map_err(anyhow::Error::from)
    }
}
