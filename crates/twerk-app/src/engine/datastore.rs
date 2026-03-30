//! Datastore proxy module
//!
//! This module provides a proxy wrapper around the Datastore interface
//! that adds initialization checks, plus factory functions for creating
//! concrete datastore implementations.

use std::env;
use std::sync::Arc;

use anyhow::Result;
use async_trait::async_trait;
use tokio::sync::RwLock;

use twerk_core::job::{Job, JobSummary, ScheduledJob, ScheduledJobSummary};
use twerk_core::node::Node;
use twerk_core::role::Role;
use twerk_core::task::{Task, TaskLogPart};
use twerk_core::user::User;
use twerk_infrastructure::datastore::{
    inmemory::InMemoryDatastore, Datastore, Error as DatastoreError, Page,
};

// ── Datastore proxy ────────────────────────────────────────────

/// [`DatastoreProxy`] wraps a [`Datastore`] and adds initialization checks.
#[derive(Clone)]
pub struct DatastoreProxy {
    inner: Arc<RwLock<Option<Box<dyn Datastore + Send + Sync>>>>,
}

/// Error message for uninitialized datastore operations.
const DATSTORE_NOT_INIT: &str = "Datastore not initialized";

impl DatastoreProxy {
    /// Creates a new empty datastore proxy.
    pub fn new() -> Self {
        Self {
            inner: Arc::new(RwLock::new(None)),
        }
    }

    /// Initializes the datastore based on configuration.
    pub async fn init(&self) -> Result<()> {
        let datastore = create_datastore().await?;
        *self.inner.write().await = Some(datastore);
        Ok(())
    }

    /// Sets a custom datastore implementation.
    pub async fn set_datastore(&self, datastore: Box<dyn Datastore + Send + Sync>) {
        *self.inner.write().await = Some(datastore);
    }

    /// Clones the inner `Arc` for sharing.
    pub fn clone_inner(&self) -> DatastoreProxy {
        DatastoreProxy {
            inner: self.inner.clone(),
        }
    }
}

#[async_trait]
impl Datastore for DatastoreProxy {
    async fn create_task(&self, task: &Task) -> twerk_infrastructure::datastore::Result<()> {
        let inner = self.inner.read().await;
        let ds = inner
            .as_deref()
            .ok_or_else(|| DatastoreError::Database(DATSTORE_NOT_INIT.to_string()))?;
        ds.create_task(task).await
    }

    async fn update_task(
        &self,
        id: &str,
        modify: Box<dyn FnOnce(Task) -> twerk_infrastructure::datastore::Result<Task> + Send>,
    ) -> twerk_infrastructure::datastore::Result<()> {
        let inner = self.inner.read().await;
        let ds = inner
            .as_deref()
            .ok_or_else(|| DatastoreError::Database(DATSTORE_NOT_INIT.to_string()))?;
        ds.update_task(id, modify).await
    }

    async fn get_task_by_id(&self, id: &str) -> twerk_infrastructure::datastore::Result<Task> {
        let inner = self.inner.read().await;
        let ds = inner
            .as_deref()
            .ok_or_else(|| DatastoreError::Database(DATSTORE_NOT_INIT.to_string()))?;
        ds.get_task_by_id(id).await
    }

    async fn get_active_tasks(
        &self,
        job_id: &str,
    ) -> twerk_infrastructure::datastore::Result<Vec<Task>> {
        let inner = self.inner.read().await;
        let ds = inner
            .as_deref()
            .ok_or_else(|| DatastoreError::Database(DATSTORE_NOT_INIT.to_string()))?;
        ds.get_active_tasks(job_id).await
    }

    async fn get_next_task(
        &self,
        parent_task_id: &str,
    ) -> twerk_infrastructure::datastore::Result<Task> {
        let inner = self.inner.read().await;
        let ds = inner
            .as_deref()
            .ok_or_else(|| DatastoreError::Database(DATSTORE_NOT_INIT.to_string()))?;
        ds.get_next_task(parent_task_id).await
    }

    async fn create_task_log_part(
        &self,
        part: &TaskLogPart,
    ) -> twerk_infrastructure::datastore::Result<()> {
        let inner = self.inner.read().await;
        let ds = inner
            .as_deref()
            .ok_or_else(|| DatastoreError::Database(DATSTORE_NOT_INIT.to_string()))?;
        ds.create_task_log_part(part).await
    }

    async fn get_task_log_parts(
        &self,
        task_id: &str,
        q: &str,
        page: i64,
        size: i64,
    ) -> twerk_infrastructure::datastore::Result<Page<TaskLogPart>> {
        let inner = self.inner.read().await;
        let ds = inner
            .as_deref()
            .ok_or_else(|| DatastoreError::Database(DATSTORE_NOT_INIT.to_string()))?;
        ds.get_task_log_parts(task_id, q, page, size).await
    }

    async fn create_node(&self, node: &Node) -> twerk_infrastructure::datastore::Result<()> {
        let inner = self.inner.read().await;
        let ds = inner
            .as_deref()
            .ok_or_else(|| DatastoreError::Database(DATSTORE_NOT_INIT.to_string()))?;
        ds.create_node(node).await
    }

    async fn update_node(
        &self,
        id: &str,
        modify: Box<dyn FnOnce(Node) -> twerk_infrastructure::datastore::Result<Node> + Send>,
    ) -> twerk_infrastructure::datastore::Result<()> {
        let inner = self.inner.read().await;
        let ds = inner
            .as_deref()
            .ok_or_else(|| DatastoreError::Database(DATSTORE_NOT_INIT.to_string()))?;
        ds.update_node(id, modify).await
    }

    async fn get_node_by_id(&self, id: &str) -> twerk_infrastructure::datastore::Result<Node> {
        let inner = self.inner.read().await;
        let ds = inner
            .as_deref()
            .ok_or_else(|| DatastoreError::Database(DATSTORE_NOT_INIT.to_string()))?;
        ds.get_node_by_id(id).await
    }

    async fn get_active_nodes(&self) -> twerk_infrastructure::datastore::Result<Vec<Node>> {
        let inner = self.inner.read().await;
        let ds = inner
            .as_deref()
            .ok_or_else(|| DatastoreError::Database(DATSTORE_NOT_INIT.to_string()))?;
        ds.get_active_nodes().await
    }

    async fn create_job(&self, job: &Job) -> twerk_infrastructure::datastore::Result<()> {
        let inner = self.inner.read().await;
        let ds = inner
            .as_deref()
            .ok_or_else(|| DatastoreError::Database(DATSTORE_NOT_INIT.to_string()))?;
        ds.create_job(job).await
    }

    async fn update_job(
        &self,
        id: &str,
        modify: Box<dyn FnOnce(Job) -> twerk_infrastructure::datastore::Result<Job> + Send>,
    ) -> twerk_infrastructure::datastore::Result<()> {
        let inner = self.inner.read().await;
        let ds = inner
            .as_deref()
            .ok_or_else(|| DatastoreError::Database(DATSTORE_NOT_INIT.to_string()))?;
        ds.update_job(id, modify).await
    }

    async fn get_job_by_id(&self, id: &str) -> twerk_infrastructure::datastore::Result<Job> {
        let inner = self.inner.read().await;
        let ds = inner
            .as_deref()
            .ok_or_else(|| DatastoreError::Database(DATSTORE_NOT_INIT.to_string()))?;
        ds.get_job_by_id(id).await
    }

    async fn get_job_log_parts(
        &self,
        job_id: &str,
        q: &str,
        page: i64,
        size: i64,
    ) -> twerk_infrastructure::datastore::Result<Page<TaskLogPart>> {
        let inner = self.inner.read().await;
        let ds = inner
            .as_deref()
            .ok_or_else(|| DatastoreError::Database(DATSTORE_NOT_INIT.to_string()))?;
        ds.get_job_log_parts(job_id, q, page, size).await
    }

    async fn get_jobs(
        &self,
        current_user: &str,
        q: &str,
        page: i64,
        size: i64,
    ) -> twerk_infrastructure::datastore::Result<Page<JobSummary>> {
        let inner = self.inner.read().await;
        let ds = inner
            .as_deref()
            .ok_or_else(|| DatastoreError::Database(DATSTORE_NOT_INIT.to_string()))?;
        ds.get_jobs(current_user, q, page, size).await
    }

    async fn create_scheduled_job(
        &self,
        sj: &ScheduledJob,
    ) -> twerk_infrastructure::datastore::Result<()> {
        let inner = self.inner.read().await;
        let ds = inner
            .as_deref()
            .ok_or_else(|| DatastoreError::Database(DATSTORE_NOT_INIT.to_string()))?;
        ds.create_scheduled_job(sj).await
    }

    async fn get_active_scheduled_jobs(
        &self,
    ) -> twerk_infrastructure::datastore::Result<Vec<ScheduledJob>> {
        let inner = self.inner.read().await;
        let ds = inner
            .as_deref()
            .ok_or_else(|| DatastoreError::Database(DATSTORE_NOT_INIT.to_string()))?;
        ds.get_active_scheduled_jobs().await
    }

    async fn get_scheduled_jobs(
        &self,
        current_user: &str,
        page: i64,
        size: i64,
    ) -> twerk_infrastructure::datastore::Result<Page<ScheduledJobSummary>> {
        let inner = self.inner.read().await;
        let ds = inner
            .as_deref()
            .ok_or_else(|| DatastoreError::Database(DATSTORE_NOT_INIT.to_string()))?;
        ds.get_scheduled_jobs(current_user, page, size).await
    }

    async fn get_scheduled_job_by_id(
        &self,
        id: &str,
    ) -> twerk_infrastructure::datastore::Result<ScheduledJob> {
        let inner = self.inner.read().await;
        let ds = inner
            .as_deref()
            .ok_or_else(|| DatastoreError::Database(DATSTORE_NOT_INIT.to_string()))?;
        ds.get_scheduled_job_by_id(id).await
    }

    async fn update_scheduled_job(
        &self,
        id: &str,
        modify: Box<
            dyn FnOnce(ScheduledJob) -> twerk_infrastructure::datastore::Result<ScheduledJob>
                + Send,
        >,
    ) -> twerk_infrastructure::datastore::Result<()> {
        let inner = self.inner.read().await;
        let ds = inner
            .as_deref()
            .ok_or_else(|| DatastoreError::Database(DATSTORE_NOT_INIT.to_string()))?;
        ds.update_scheduled_job(id, modify).await
    }

    async fn delete_scheduled_job(&self, id: &str) -> twerk_infrastructure::datastore::Result<()> {
        let inner = self.inner.read().await;
        let ds = inner
            .as_deref()
            .ok_or_else(|| DatastoreError::Database(DATSTORE_NOT_INIT.to_string()))?;
        ds.delete_scheduled_job(id).await
    }

    async fn create_user(&self, user: &User) -> twerk_infrastructure::datastore::Result<()> {
        let inner = self.inner.read().await;
        let ds = inner
            .as_deref()
            .ok_or_else(|| DatastoreError::Database(DATSTORE_NOT_INIT.to_string()))?;
        ds.create_user(user).await
    }

    async fn get_user(&self, username: &str) -> twerk_infrastructure::datastore::Result<User> {
        let inner = self.inner.read().await;
        let ds = inner
            .as_deref()
            .ok_or_else(|| DatastoreError::Database(DATSTORE_NOT_INIT.to_string()))?;
        ds.get_user(username).await
    }

    async fn create_role(&self, role: &Role) -> twerk_infrastructure::datastore::Result<()> {
        let inner = self.inner.read().await;
        let ds = inner
            .as_deref()
            .ok_or_else(|| DatastoreError::Database(DATSTORE_NOT_INIT.to_string()))?;
        ds.create_role(role).await
    }

    async fn get_role(&self, id: &str) -> twerk_infrastructure::datastore::Result<Role> {
        let inner = self.inner.read().await;
        let ds = inner
            .as_deref()
            .ok_or_else(|| DatastoreError::Database(DATSTORE_NOT_INIT.to_string()))?;
        ds.get_role(id).await
    }

    async fn get_roles(&self) -> twerk_infrastructure::datastore::Result<Vec<Role>> {
        let inner = self.inner.read().await;
        let ds = inner
            .as_deref()
            .ok_or_else(|| DatastoreError::Database(DATSTORE_NOT_INIT.to_string()))?;
        ds.get_roles().await
    }

    async fn get_user_roles(
        &self,
        user_id: &str,
    ) -> twerk_infrastructure::datastore::Result<Vec<Role>> {
        let inner = self.inner.read().await;
        let ds = inner
            .as_deref()
            .ok_or_else(|| DatastoreError::Database(DATSTORE_NOT_INIT.to_string()))?;
        ds.get_user_roles(user_id).await
    }

    async fn assign_role(
        &self,
        user_id: &str,
        role_id: &str,
    ) -> twerk_infrastructure::datastore::Result<()> {
        let inner = self.inner.read().await;
        let ds = inner
            .as_deref()
            .ok_or_else(|| DatastoreError::Database(DATSTORE_NOT_INIT.to_string()))?;
        ds.assign_role(user_id, role_id).await
    }

    async fn unassign_role(
        &self,
        user_id: &str,
        role_id: &str,
    ) -> twerk_infrastructure::datastore::Result<()> {
        let inner = self.inner.read().await;
        let ds = inner
            .as_deref()
            .ok_or_else(|| DatastoreError::Database(DATSTORE_NOT_INIT.to_string()))?;
        ds.unassign_role(user_id, role_id).await
    }

    async fn get_metrics(
        &self,
    ) -> twerk_infrastructure::datastore::Result<twerk_core::stats::Metrics> {
        let inner = self.inner.read().await;
        let ds = inner
            .as_deref()
            .ok_or_else(|| DatastoreError::Database(DATSTORE_NOT_INIT.to_string()))?;
        ds.get_metrics().await
    }

    async fn with_tx(
        &self,
        f: Box<
            dyn for<'a> FnOnce(
                    &'a dyn Datastore,
                ) -> futures_util::future::BoxFuture<
                    'a,
                    twerk_infrastructure::datastore::Result<()>,
                > + Send,
        >,
    ) -> twerk_infrastructure::datastore::Result<()> {
        let inner = self.inner.read().await;
        let ds = inner
            .as_deref()
            .ok_or_else(|| DatastoreError::Database(DATSTORE_NOT_INIT.to_string()))?;
        ds.with_tx(f).await
    }

    async fn health_check(&self) -> twerk_infrastructure::datastore::Result<()> {
        let inner = self.inner.read().await;
        let ds = inner
            .as_deref()
            .ok_or_else(|| DatastoreError::Database(DATSTORE_NOT_INIT.to_string()))?;
        ds.health_check().await
    }
}

impl Default for DatastoreProxy {
    fn default() -> Self {
        Self::new()
    }
}

// ── In-memory datastore (Removed, moved to twerk-infrastructure)

// ── Datastore factory ──────────────────────────────────────────

const DEFAULT_POSTGRES_DSN: &str =
    "host=localhost user=twerk password=twerk dbname=twerk port=5432 sslmode=disable";

pub async fn create_datastore() -> Result<Box<dyn Datastore + Send + Sync>> {
    let dstype = env_string_default("datastore.type", "postgres");

    match dstype.as_str() {
        "postgres" => {
            let dsn = env_string_default("datastore.postgres.dsn", DEFAULT_POSTGRES_DSN);
            let opts = twerk_infrastructure::datastore::Options {
                encryption_key: Some(env_string("datastore.encryption.key"))
                    .filter(|s| !s.is_empty()),
                ..Default::default()
            };
            let pg = twerk_infrastructure::datastore::postgres::PostgresDatastore::new(&dsn, opts)
                .await
                .map_err(|e| anyhow::anyhow!("unable to connect to postgres: {}", e))?;
            Ok(Box::new(pg))
        }
        "inmemory" => Ok(Box::new(InMemoryDatastore::new())),
        other => Err(anyhow::anyhow!("unknown datastore type: {}", other)),
    }
}

/// Retrieves an environment variable, returning an empty string if not set.
///
/// This is intentional for optional configuration values where missing env vars
/// should be treated as empty strings rather than errors.
fn env_string(key: &str) -> String {
    let env_key = format!("TWERK_{}", key.to_uppercase().replace('.', "_"));
    // Explicitly handle the Result: convert to Option, then use unwrap_or_default
    env::var(&env_key).ok().unwrap_or_default()
}

fn env_string_default(key: &str, default: &str) -> String {
    let v = env_string(key);
    if v.is_empty() {
        default.to_string()
    } else {
        v
    }
}

#[must_use]
pub fn new_inmemory_datastore() -> Box<dyn Datastore + Send + Sync> {
    Box::new(InMemoryDatastore::new())
}

#[must_use]
pub fn new_inmemory_datastore_arc() -> std::sync::Arc<dyn Datastore> {
    std::sync::Arc::new(InMemoryDatastore::new())
}
