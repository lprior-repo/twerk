//! Heartbeat module.
//!
//! Worker heartbeat sending to broker for coordination.

use std::sync::Arc;
use std::time::Duration;

use time::OffsetDateTime;
use tokio::sync::broadcast;
use tokio::time::sleep;
use tracing::{error, warn};

use twerk_core::host;
use twerk_core::id::NodeId;
use twerk_core::node::{Node, NodeStatus};

use crate::broker::Broker;
use crate::runtime::Runtime as RuntimeTrait;

/// Send heartbeats to the broker
pub async fn send_heartbeats(
    broker: Arc<dyn Broker>,
    runtime: Arc<dyn RuntimeTrait>,
    id: String,
    name: String,
    port: u16,
    mut stop_rx: broadcast::Receiver<()>,
) {
    let heartbeat_interval = Duration::from_secs(30);

    loop {
        tokio::select! {
            biased;

            _ = stop_rx.recv() => break,
            () = sleep(heartbeat_interval) => {}
        }

        // Check runtime health
        let status = match runtime.health_check().await {
            Ok(()) => NodeStatus::UP,
            Err(e) => {
                warn!("Runtime health check failed: {}", e);
                NodeStatus::DOWN
            }
        };

        // Get hostname - handle error explicitly
        let hostname = hostname::get().map_or_else(
            |_| {
                // Log the error and use fallback
                error!("Failed to get hostname, using 'unknown'");
                "unknown".to_string()
            },
            |h| h.to_string_lossy().into_owned(),
        );

        // Get CPU usage
        let cpu_percent = host::get_cpu_percent().into();

        // Create node for heartbeat
        let node = Node {
            id: Some(NodeId::from(id.clone())),
            name: Some(name.clone()),
            hostname: Some(hostname),
            cpu_percent,
            status: Some(status),
            port: Some(i64::from(port)),
            last_heartbeat_at: Some(OffsetDateTime::now_utc()),
            ..Default::default()
        };

        if let Err(e) = broker.publish_heartbeat(node).await {
            error!("Failed to publish heartbeat: {}", e);
        }
    }
}
