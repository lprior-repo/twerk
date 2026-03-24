//! Heartbeat handler for node heartbeats.
//!
//! Port of Go `internal/coordinator/handlers/heartbeat.go` with 100% parity.
//!
//! # Go Parity
//!
//! 1. Receives Node heartbeats
//! 2. First heartbeat for a node: creates it in datastore (ErrNodeNotFound check)
//! 3. Subsequent: updates node via ds.UpdateNode with old heartbeat filtering
//! 4. Updates task count on the node

use std::sync::Arc;

use tork::node::Node;
use tork::Datastore;

use crate::handlers::HandlerError;

// ---------------------------------------------------------------------------
// Pure Calculations (Data → Calc)
// ---------------------------------------------------------------------------

/// Result of comparing incoming vs stored heartbeat timestamps.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum HeartbeatFreshness {
    /// Incoming heartbeat is newer or equal — should update.
    Fresh,
    /// Incoming heartbeat is older — should be ignored.
    Stale,
}

/// Determines whether an incoming heartbeat is newer than the stored one.
/// Go: `if u.LastHeartbeatAt.After(n.LastHeartbeatAt) { return nil }`
#[must_use]
pub(crate) fn compare_heartbeat_freshness(
    stored: time::OffsetDateTime,
    incoming: time::OffsetDateTime,
) -> HeartbeatFreshness {
    if stored > incoming {
        HeartbeatFreshness::Stale
    } else {
        HeartbeatFreshness::Fresh
    }
}

/// Applies heartbeat fields from an incoming node to the stored node,
/// returning a new node struct (no mutation).
/// Go: `u.LastHeartbeatAt = n.LastHeartbeatAt`
///     `u.CPUPercent = n.CPUPercent`
///     `u.Status = n.Status`
///     `u.TaskCount = n.TaskCount`
#[must_use]
pub(crate) fn apply_heartbeat_update(stored: &Node, incoming: &Node) -> Node {
    Node {
        last_heartbeat_at: incoming.last_heartbeat_at,
        cpu_percent: incoming.cpu_percent,
        status: incoming.status.clone(),
        task_count: incoming.task_count,
        ..stored.clone()
    }
}

// ---------------------------------------------------------------------------
// Handler (Action boundary)
// ---------------------------------------------------------------------------

/// Heartbeat handler for processing node heartbeats.
///
/// Holds a reference to the datastore for I/O operations.
/// All core logic is delegated to pure calculation functions above.
pub struct HeartbeatHandler {
    ds: Arc<dyn Datastore>,
}

impl std::fmt::Debug for HeartbeatHandler {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("HeartbeatHandler").finish()
    }
}

impl HeartbeatHandler {
    /// Create a new heartbeat handler with datastore dependency.
    /// Go: `NewHeartbeatHandler(ds datastore.Datastore) node.HandlerFunc`
    pub fn new(ds: Arc<dyn Datastore>) -> Self {
        Self { ds }
    }

    /// Handle a heartbeat from a node.
    ///
    /// Go parity (`handle`):
    /// 1. Look up node by ID
    /// 2. If not found (first heartbeat): create the node in datastore
    /// 3. If found: apply update only if incoming heartbeat is newer than stored
    pub async fn handle(&self, node: &Node) -> Result<(), HandlerError> {
        let node_id = node
            .id
            .as_deref()
            .ok_or_else(|| HandlerError::Validation("node ID is required".into()))?;

        let existing = self
            .ds
            .get_node_by_id(node_id.to_string())
            .await
            .map_err(|e| HandlerError::Datastore(e.to_string()))?;

        match existing {
            None => {
                // Go: first heartbeat — create node
                self.ds
                    .create_node(node.clone())
                    .await
                    .map_err(|e| HandlerError::Datastore(e.to_string()))?;
            }
            Some(stored) => {
                // Go: subsequent heartbeat — filter stale, then update
                let freshness =
                    compare_heartbeat_freshness(stored.last_heartbeat_at, node.last_heartbeat_at);

                if let HeartbeatFreshness::Fresh = freshness {
                    let updated = apply_heartbeat_update(&stored, node);
                    self.ds
                        .update_node(node_id.to_string(), updated)
                        .await
                        .map_err(|e| HandlerError::Datastore(e.to_string()))?;
                }
            }
        }

        Ok(())
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------
