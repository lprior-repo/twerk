//! Node operations for `PostgresDatastore`.

use sqlx::Postgres;
use twerk_core::node::Node;

use crate::datastore::postgres::records::{NodeRecord, NodeRecordExt};
use crate::datastore::postgres::{DatastoreError, DatastoreResult, Executor, PostgresDatastore};

impl PostgresDatastore {
    pub(super) async fn create_node_impl(&self, node: &Node) -> DatastoreResult<()> {
        let id = node.id.as_ref().ok_or(DatastoreError::InvalidInput(
            "node ID is required".to_string(),
        ))?;
        let name = node.name.as_ref().ok_or(DatastoreError::InvalidInput(
            "node name is required".to_string(),
        ))?;
        let hostname = node.hostname.as_ref().ok_or(DatastoreError::InvalidInput(
            "node hostname is required".to_string(),
        ))?;
        let queue = node.queue.as_ref().ok_or(DatastoreError::InvalidInput(
            "node queue is required".to_string(),
        ))?;
        let version = node.version.as_ref().ok_or(DatastoreError::InvalidInput(
            "node version is required".to_string(),
        ))?;

        let q = r"INSERT INTO nodes (id, name, started_at, last_heartbeat_at, cpu_percent, queue, status, hostname, port, task_count, version_) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11)";
        let query = sqlx::query(q)
            .bind(&**id)
            .bind(name)
            .bind(
                node.started_at
                    .unwrap_or_else(time::OffsetDateTime::now_utc),
            )
            .bind(
                node.last_heartbeat_at
                    .unwrap_or_else(time::OffsetDateTime::now_utc),
            )
            .bind(node.cpu_percent.unwrap_or(0.0))
            .bind(queue)
            .bind(node.status.as_ref().map(|s| s.as_ref()).unwrap_or("UP"))
            .bind(hostname)
            .bind(node.port.unwrap_or(0))
            .bind(node.task_count.unwrap_or(0))
            .bind(version);

        match &self.executor {
            Executor::Pool(p) => {
                query
                    .execute(p)
                    .await
                    .map_err(|e| DatastoreError::Database(format!("create node failed: {e}")))?;
            }
            Executor::Tx(tx) => {
                let mut tx = tx.lock().await;
                query
                    .execute(&mut **tx)
                    .await
                    .map_err(|e| DatastoreError::Database(format!("create node failed: {e}")))?;
            }
        }
        Ok(())
    }

    pub(super) async fn update_node_impl(
        &self,
        id: &str,
        modify: Box<dyn FnOnce(Node) -> DatastoreResult<Node> + Send>,
    ) -> DatastoreResult<()> {
        match &self.executor {
            Executor::Pool(p) => {
                let mut tx = p
                    .begin()
                    .await
                    .map_err(|e| DatastoreError::Transaction(format!("begin tx failed: {e}")))?;
                let record: NodeRecord = sqlx::query_as::<Postgres, NodeRecord>(
                    "SELECT * FROM nodes WHERE id = $1 FOR UPDATE",
                )
                .bind(id)
                .fetch_optional(&mut *tx)
                .await
                .map_err(|e| DatastoreError::Database(format!("get node failed: {e}")))?
                .ok_or(DatastoreError::NodeNotFound)?;
                let node = record.to_node();
                let node = modify(node)?;
                sqlx::query(r"UPDATE nodes SET last_heartbeat_at = $1, cpu_percent = $2, task_count = $3, status = $4, queue = $5 WHERE id = $6")
                    .bind(node.last_heartbeat_at)
                    .bind(node.cpu_percent.unwrap_or(0.0))
                    .bind(node.task_count.unwrap_or(0))
                    .bind(node.status.as_ref().map(|s| s.as_ref()).unwrap_or("UP"))
                    .bind(node.queue.as_deref().unwrap_or("default"))
                    .bind(id)
                    .execute(&mut *tx)
                    .await
                    .map_err(|e| DatastoreError::Database(format!("update node failed: {e}")))?;
                tx.commit()
                    .await
                    .map_err(|e| DatastoreError::Transaction(format!("commit tx failed: {e}")))?;
            }
            Executor::Tx(tx) => {
                let mut tx = tx.lock().await;
                let record: NodeRecord = sqlx::query_as::<Postgres, NodeRecord>(
                    "SELECT * FROM nodes WHERE id = $1 FOR UPDATE",
                )
                .bind(id)
                .fetch_optional(&mut **tx)
                .await
                .map_err(|e| DatastoreError::Database(format!("get node failed: {e}")))?
                .ok_or(DatastoreError::NodeNotFound)?;
                let node = record.to_node();
                let node = modify(node)?;
                sqlx::query(r"UPDATE nodes SET last_heartbeat_at = $1, cpu_percent = $2, task_count = $3, status = $4, queue = $5 WHERE id = $6")
                    .bind(node.last_heartbeat_at)
                    .bind(node.cpu_percent.unwrap_or(0.0))
                    .bind(node.task_count.unwrap_or(0))
                    .bind(node.status.as_ref().map(|s| s.as_ref()).unwrap_or("UP"))
                    .bind(node.queue.as_deref().unwrap_or("default"))
                    .bind(id)
                    .execute(&mut **tx)
                    .await
                    .map_err(|e| DatastoreError::Database(format!("update node failed: {e}")))?;
            }
        }
        Ok(())
    }

    pub(super) async fn get_node_by_id_impl(&self, id: &str) -> DatastoreResult<Node> {
        let record: NodeRecord = match &self.executor {
            Executor::Pool(p) => {
                sqlx::query_as::<Postgres, NodeRecord>("SELECT * FROM nodes WHERE id = $1")
                    .bind(id)
                    .fetch_optional(p)
                    .await
            }
            Executor::Tx(tx) => {
                let mut tx = tx.lock().await;
                sqlx::query_as::<Postgres, NodeRecord>("SELECT * FROM nodes WHERE id = $1")
                    .bind(id)
                    .fetch_optional(&mut **tx)
                    .await
            }
        }
        .map_err(|e| DatastoreError::Database(format!("get node failed: {e}")))?
        .ok_or(DatastoreError::NodeNotFound)?;
        Ok(record.to_node())
    }

    pub(super) async fn get_active_nodes_impl(&self) -> DatastoreResult<Vec<Node>> {
        let records: Vec<NodeRecord> =
            match &self.executor {
                Executor::Pool(p) => sqlx::query_as::<Postgres, NodeRecord>(
                    "SELECT * FROM nodes WHERE status != 'DOWN' ORDER BY last_heartbeat_at DESC",
                )
                .fetch_all(p)
                .await,
                Executor::Tx(tx) => {
                    let mut tx = tx.lock().await;
                    sqlx::query_as::<Postgres, NodeRecord>(
                    "SELECT * FROM nodes WHERE status != 'DOWN' ORDER BY last_heartbeat_at DESC",
                )
                .fetch_all(&mut **tx)
                .await
                }
            }
            .map_err(|e| DatastoreError::Database(format!("get active nodes failed: {e}")))?;
        Ok(records.into_iter().map(|r| r.to_node()).collect())
    }
}
