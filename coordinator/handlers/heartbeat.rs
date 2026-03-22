//! Heartbeat handler for node heartbeats.

use crate::handlers::{
    noop_node_handler, HandlerContext, HandlerError, NodeHandlerFunc,
};
use tork::node::Node;

/// Heartbeat handler for processing node heartbeats.
pub struct HeartbeatHandler {
    handler: NodeHandlerFunc,
}

impl HeartbeatHandler {
    /// Create a new heartbeat handler.
    pub fn new() -> Self {
        Self {
            handler: noop_node_handler(),
        }
    }

    /// Create a heartbeat handler with a custom handler function.
    pub fn with_handler(handler: NodeHandlerFunc) -> Self {
        Self { handler }
    }

    /// Handle a heartbeat from a node.
    pub fn handle(&self, ctx: HandlerContext, node: &mut Node) -> Result<(), HandlerError> {
        (self.handler)(ctx, node)
    }
}

impl Default for HeartbeatHandler {
    fn default() -> Self {
        Self::new()
    }
}

impl Clone for HeartbeatHandler {
    fn clone(&self) -> Self {
        Self {
            handler: self.handler.clone(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;
    use time::OffsetDateTime;
    use tork::node::NODE_STATUS_UP;

    fn make_test_node() -> Node {
        Node {
            id: Some("test-node".to_string()),
            name: Some("test-node".to_string()),
            started_at: OffsetDateTime::now_utc(),
            cpu_percent: 0.0,
            last_heartbeat_at: OffsetDateTime::now_utc(),
            queue: None,
            status: NODE_STATUS_UP.to_string(),
            hostname: Some("localhost".to_string()),
            port: 8080,
            task_count: 0,
            version: "1.0.0".to_string(),
        }
    }

    #[test]
    fn test_heartbeat_handler_default() {
        let handler = HeartbeatHandler::new();
        let ctx = Arc::new(());
        let mut node = make_test_node();
        assert!(handler.handle(ctx, &mut node).is_ok());
    }

    #[test]
    fn test_heartbeat_handler_with_custom() {
        let handler = HeartbeatHandler::with_handler(Arc::new(|_ctx, _node| Ok(())));
        let ctx = Arc::new(());
        let mut node = make_test_node();
        assert!(handler.handle(ctx, &mut node).is_ok());
    }
}