//! Health and node-related event handlers

use anyhow::Result;
use std::sync::Arc;
use tracing::warn;
use twerk_core::node::Node;
use twerk_core::node::NodeStatus;

const DEFAULT_NODE_QUEUE: &str = "default";
const DEFAULT_NODE_VERSION: &str = env!("CARGO_PKG_VERSION");

fn heartbeat_node(node: Node) -> Node {
    let now = time::OffsetDateTime::now_utc();
    let heartbeat_at = match node.last_heartbeat_at {
        Some(timestamp) if timestamp <= now => timestamp,
        Some(_) | None => now,
    };
    let status = match node.status {
        Some(status) => status,
        None => NodeStatus::UP,
    };
    let queue = match node.queue {
        Some(queue) if !queue.trim().is_empty() => queue,
        _ => DEFAULT_NODE_QUEUE.to_string(),
    };
    let version = match node.version {
        Some(version) if !version.trim().is_empty() => version,
        _ => DEFAULT_NODE_VERSION.to_string(),
    };

    Node {
        status: Some(status),
        last_heartbeat_at: Some(heartbeat_at),
        queue: Some(queue),
        version: Some(version),
        ..node
    }
}

/// Handles node heartbeat.
///
/// # Errors
/// Returns error if node update fails.
pub async fn handle_heartbeat(
    ds: Arc<dyn twerk_infrastructure::datastore::Datastore>,
    _broker: Arc<dyn twerk_infrastructure::broker::Broker>,
    node: twerk_core::node::Node,
) -> Result<()> {
    let new_node = heartbeat_node(node);

    if let Some(node_id) = &new_node.id {
        let node_id_str = node_id.to_string();
        let update_result = ds
            .update_node(
                &node_id_str,
                Box::new({
                    let heartbeat_at = new_node.last_heartbeat_at;
                    let cpu = new_node.cpu_percent;
                    let task_count = new_node.task_count;
                    let status = new_node.status.clone();
                    let queue = new_node.queue.clone();
                    let version = new_node.version.clone();
                    move |u: twerk_core::node::Node| {
                        Ok(twerk_core::node::Node {
                            last_heartbeat_at: heartbeat_at,
                            cpu_percent: cpu,
                            task_count,
                            status,
                            queue,
                            version,
                            ..u
                        })
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
