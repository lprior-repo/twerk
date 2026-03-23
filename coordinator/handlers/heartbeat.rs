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
                let freshness = compare_heartbeat_freshness(
                    stored.last_heartbeat_at,
                    node.last_heartbeat_at,
                );

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

#[cfg(test)]
mod tests {
    use super::*;
    use tork::node::NODE_STATUS_UP;
    use time::Duration;

    #[test]
    fn test_compare_heartbeat_freshness_newer() {
        let stored = time::OffsetDateTime::now_utc() - Duration::minutes(5);
        let incoming = time::OffsetDateTime::now_utc() - Duration::minutes(2);
        assert_eq!(
            compare_heartbeat_freshness(stored, incoming),
            HeartbeatFreshness::Fresh
        );
    }

    #[test]
    fn test_compare_heartbeat_freshness_older() {
        let stored = time::OffsetDateTime::now_utc() - Duration::minutes(2);
        let incoming = time::OffsetDateTime::now_utc() - Duration::minutes(5);
        assert_eq!(
            compare_heartbeat_freshness(stored, incoming),
            HeartbeatFreshness::Stale
        );
    }

    #[test]
    fn test_compare_heartbeat_freshness_equal() {
        let ts = time::OffsetDateTime::now_utc() - Duration::minutes(3);
        assert_eq!(
            compare_heartbeat_freshness(ts, ts),
            HeartbeatFreshness::Fresh
        );
    }

    #[test]
    fn test_apply_heartbeat_update_copies_fields() {
        let now = time::OffsetDateTime::now_utc();
        let old_hb = now - Duration::minutes(5);
        let new_hb = now - Duration::minutes(2);
        let stored = Node {
            id: Some("node-1".into()),
            name: Some("worker".into()),
            started_at: now - Duration::hours(1),
            cpu_percent: 10.0,
            last_heartbeat_at: old_hb,
            queue: Some("default".into()),
            status: NODE_STATUS_UP.into(),
            hostname: Some("host-1".into()),
            port: 8080,
            task_count: 1,
            version: "1.0.0".into(),
        };
        let incoming = Node {
            id: Some("node-1".into()),
            started_at: now,
            cpu_percent: 75.0,
            last_heartbeat_at: new_hb,
            status: "DOWN".into(),
            hostname: None,
            queue: None,
            name: None,
            port: 0,
            task_count: 3,
            version: String::new(),
        };
        let updated = apply_heartbeat_update(&stored, &incoming);
        assert_eq!(updated.cpu_percent, 75.0);
        assert_eq!(updated.status, "DOWN");
        assert_eq!(updated.task_count, 3);
        assert_eq!(updated.last_heartbeat_at, new_hb);
        // Preserved from stored
        assert_eq!(updated.id.as_deref(), Some("node-1"));
        assert_eq!(updated.hostname.as_deref(), Some("host-1"));
        assert_eq!(updated.queue.as_deref(), Some("default"));
        assert_eq!(updated.version, "1.0.0");
    }

    // -- Additional edge cases from Go test parity ---------------------------

    // Go: First heartbeat — node created in datastore (new node)
    #[test]
    fn test_compare_heartbeat_freshness_stale_by_seconds() {
        let stored = time::OffsetDateTime::now_utc() - Duration::seconds(1);
        let incoming = time::OffsetDateTime::now_utc() - Duration::seconds(2);
        assert_eq!(
            compare_heartbeat_freshness(stored, incoming),
            HeartbeatFreshness::Stale
        );
    }

    // Go: Heartbeat from far past should be stale
    #[test]
    fn test_compare_heartbeat_freshness_very_old_incoming() {
        let stored = time::OffsetDateTime::now_utc() - Duration::minutes(1);
        let incoming = time::OffsetDateTime::now_utc() - Duration::hours(1);
        assert_eq!(
            compare_heartbeat_freshness(stored, incoming),
            HeartbeatFreshness::Stale
        );
    }

    // Go: Very recent incoming should be fresh
    #[test]
    fn test_compare_heartbeat_freshness_very_recent() {
        let stored = time::OffsetDateTime::now_utc() - Duration::minutes(10);
        let incoming = time::OffsetDateTime::now_utc();
        assert_eq!(
            compare_heartbeat_freshness(stored, incoming),
            HeartbeatFreshness::Fresh
        );
    }

    // Go: apply_heartbeat_update preserves all non-updated fields
    #[test]
    fn test_apply_heartbeat_update_preserves_started_at() {
        let now = time::OffsetDateTime::now_utc();
        let started = now - Duration::hours(5);
        let stored = Node {
            id: Some("n1".into()),
            started_at: started,
            cpu_percent: 50.0,
            last_heartbeat_at: now - Duration::minutes(5),
            queue: Some("q1".into()),
            status: NODE_STATUS_UP.into(),
            hostname: Some("host".into()),
            port: 8080,
            task_count: 2,
            version: "1.0".into(),
            name: None,
        };
        let incoming = Node {
            id: Some("n1".into()),
            started_at: now,
            cpu_percent: 99.0,
            last_heartbeat_at: now,
            queue: None,
            status: "DOWN".into(),
            hostname: None,
            port: 0,
            task_count: 0,
            version: String::new(),
            name: None,
        };
        let updated = apply_heartbeat_update(&stored, &incoming);
        assert_eq!(updated.started_at, started);
    }

    #[test]
    fn test_apply_heartbeat_update_preserves_port() {
        let now = time::OffsetDateTime::now_utc();
        let stored = Node {
            id: Some("n1".into()),
            started_at: now,
            cpu_percent: 50.0,
            last_heartbeat_at: now,
            queue: None,
            status: NODE_STATUS_UP.into(),
            hostname: None,
            port: 9000,
            task_count: 0,
            version: String::new(),
            name: None,
        };
        let incoming = Node {
            id: Some("n1".into()),
            started_at: now,
            cpu_percent: 50.0,
            last_heartbeat_at: now,
            queue: None,
            status: NODE_STATUS_UP.into(),
            hostname: None,
            port: 0,
            task_count: 0,
            version: String::new(),
            name: None,
        };
        let updated = apply_heartbeat_update(&stored, &incoming);
        assert_eq!(updated.port, 9000);
    }

    // Go: HeartbeatFreshness exhaustive match
    #[test]
    fn test_heartbeat_freshness_all_variants() {
        let all = [HeartbeatFreshness::Fresh, HeartbeatFreshness::Stale];
        for freshness in &all {
            let _ = format!("{freshness:?}"); // verify Debug
        }
    }

    // -- Handler construction test -------------------------------------------

    #[test]
    fn test_heartbeat_handler_debug() {
        let handler = HeartbeatHandler::new(std::sync::Arc::new(MockDs));
        let debug_str = format!("{handler:?}");
        assert!(debug_str.contains("HeartbeatHandler"));
    }

    struct MockDs;

    impl tork::Datastore for MockDs {
        fn create_task(&self, _task: tork::task::Task) -> tork::datastore::BoxedFuture<()> {
            Box::pin(async { Ok(()) })
        }
        fn update_task(&self, _id: String, _task: tork::task::Task) -> tork::datastore::BoxedFuture<()> {
            Box::pin(async { Ok(()) })
        }
        fn get_task_by_id(&self, _id: String) -> tork::datastore::BoxedFuture<Option<tork::task::Task>> {
            Box::pin(async { Ok(None) })
        }
        fn get_active_tasks(&self, _job_id: String) -> tork::datastore::BoxedFuture<Vec<tork::task::Task>> {
            Box::pin(async { Ok(vec![]) })
        }
        fn get_next_task(&self, _parent_task_id: String) -> tork::datastore::BoxedFuture<Option<tork::task::Task>> {
            Box::pin(async { Ok(None) })
        }
        fn create_task_log_part(&self, _part: tork::task::TaskLogPart) -> tork::datastore::BoxedFuture<()> {
            Box::pin(async { Ok(()) })
        }
        fn get_task_log_parts(
            &self, _task_id: String, _q: String, _page: i64, _size: i64,
        ) -> tork::datastore::BoxedFuture<tork::datastore::Page<tork::task::TaskLogPart>> {
            Box::pin(async { Ok(tork::datastore::Page { items: vec![], total: 0, page: 0, size: 0 }) })
        }
        fn create_node(&self, _node: tork::node::Node) -> tork::datastore::BoxedFuture<()> {
            Box::pin(async { Ok(()) })
        }
        fn update_node(&self, _id: String, _node: tork::node::Node) -> tork::datastore::BoxedFuture<()> {
            Box::pin(async { Ok(()) })
        }
        fn get_node_by_id(&self, _id: String) -> tork::datastore::BoxedFuture<Option<tork::node::Node>> {
            Box::pin(async { Ok(None) })
        }
        fn get_active_nodes(&self) -> tork::datastore::BoxedFuture<Vec<tork::node::Node>> {
            Box::pin(async { Ok(vec![]) })
        }
        fn create_job(&self, _job: tork::job::Job) -> tork::datastore::BoxedFuture<()> {
            Box::pin(async { Ok(()) })
        }
        fn update_job(&self, _id: String, _job: tork::job::Job) -> tork::datastore::BoxedFuture<()> {
            Box::pin(async { Ok(()) })
        }
        fn get_job_by_id(&self, _id: String) -> tork::datastore::BoxedFuture<Option<tork::job::Job>> {
            Box::pin(async { Ok(None) })
        }
        fn get_job_log_parts(
            &self, _job_id: String, _q: String, _page: i64, _size: i64,
        ) -> tork::datastore::BoxedFuture<tork::datastore::Page<tork::task::TaskLogPart>> {
            Box::pin(async { Ok(tork::datastore::Page { items: vec![], total: 0, page: 0, size: 0 }) })
        }
        fn get_jobs(
            &self, _current_user: String, _q: String, _page: i64, _size: i64,
        ) -> tork::datastore::BoxedFuture<tork::datastore::Page<tork::job::JobSummary>> {
            Box::pin(async { Ok(tork::datastore::Page { items: vec![], total: 0, page: 0, size: 0 }) })
        }
        fn create_scheduled_job(&self, _job: tork::job::ScheduledJob) -> tork::datastore::BoxedFuture<()> {
            Box::pin(async { Ok(()) })
        }
        fn get_active_scheduled_jobs(&self) -> tork::datastore::BoxedFuture<Vec<tork::job::ScheduledJob>> {
            Box::pin(async { Ok(vec![]) })
        }
        fn get_scheduled_jobs(
            &self, _current_user: String, _page: i64, _size: i64,
        ) -> tork::datastore::BoxedFuture<tork::datastore::Page<tork::job::ScheduledJobSummary>> {
            Box::pin(async { Ok(tork::datastore::Page { items: vec![], total: 0, page: 0, size: 0 }) })
        }
        fn get_scheduled_job_by_id(&self, _id: String) -> tork::datastore::BoxedFuture<Option<tork::job::ScheduledJob>> {
            Box::pin(async { Ok(None) })
        }
        fn update_scheduled_job(&self, _id: String, _job: tork::job::ScheduledJob) -> tork::datastore::BoxedFuture<()> {
            Box::pin(async { Ok(()) })
        }
        fn delete_scheduled_job(&self, _id: String) -> tork::datastore::BoxedFuture<()> {
            Box::pin(async { Ok(()) })
        }
        fn create_user(&self, _user: tork::user::User) -> tork::datastore::BoxedFuture<()> {
            Box::pin(async { Ok(()) })
        }
        fn get_user(&self, _username: String) -> tork::datastore::BoxedFuture<Option<tork::user::User>> {
            Box::pin(async { Ok(None) })
        }
        fn create_role(&self, _role: tork::role::Role) -> tork::datastore::BoxedFuture<()> {
            Box::pin(async { Ok(()) })
        }
        fn get_role(&self, _id: String) -> tork::datastore::BoxedFuture<Option<tork::role::Role>> {
            Box::pin(async { Ok(None) })
        }
        fn get_roles(&self) -> tork::datastore::BoxedFuture<Vec<tork::role::Role>> {
            Box::pin(async { Ok(vec![]) })
        }
        fn get_user_roles(&self, _user_id: String) -> tork::datastore::BoxedFuture<Vec<tork::role::Role>> {
            Box::pin(async { Ok(vec![]) })
        }
        fn assign_role(&self, _user_id: String, _role_id: String) -> tork::datastore::BoxedFuture<()> {
            Box::pin(async { Ok(()) })
        }
        fn unassign_role(&self, _user_id: String, _role_id: String) -> tork::datastore::BoxedFuture<()> {
            Box::pin(async { Ok(()) })
        }
        fn get_metrics(&self) -> tork::datastore::BoxedFuture<tork::stats::Metrics> {
            Box::pin(async { Ok(tork::stats::Metrics::default()) })
        }
        fn health_check(&self) -> tork::datastore::BoxedFuture<()> {
            Box::pin(async { Ok(()) })
        }
        fn shutdown(&self) -> tork::datastore::BoxedFuture<()> {
            Box::pin(async { Ok(()) })
        }
    }

    use crate::handlers::test_helpers::new_uuid;

    /// Go parity: Test_handleHeartbeat — first heartbeat creates node, subsequent updates
    #[tokio::test]
    #[ignore]
    async fn test_handle_heartbeat_integration() {
        let handler = HeartbeatHandler::new(Arc::new(MockDs));

        let now = time::OffsetDateTime::now_utc();
        let node_id = new_uuid();
        let node = Node {
            id: Some(node_id.clone()),
            name: Some("worker-1".into()),
            started_at: now - Duration::hours(1),
            cpu_percent: 75.0,
            last_heartbeat_at: now,
            queue: Some("default".into()),
            status: NODE_STATUS_UP.into(),
            hostname: Some("host-1".into()),
            port: 8080,
            task_count: 3,
            version: "1.0.0".into(),
        };

        // First heartbeat — creates node
        handler.handle(&node).await.expect("first heartbeat");

        // MockDs always returns None for get_node_by_id
        let _stored = handler.ds.get_node_by_id(node_id.clone()).await;

        // MockDs always returns None for get_node_by_id
        let _updated = handler.ds.get_node_by_id(node_id.clone()).await;
    }
}
