//! Datastore proxy module
//!
//! This module provides a proxy wrapper around the Datastore interface
//! that adds initialization checks.

use std::env;
use anyhow::Result;
use std::sync::Arc;
use tokio::sync::RwLock;
use tork::datastore::{Datastore, Page};
use tork::job::{Job, JobSummary, ScheduledJob, ScheduledJobSummary};
use tork::node::Node;
use tork::role::Role;
use tork::stats::Metrics;
use tork::task::{Task, TaskLogPart};
use tork::user::User;

/// DatastoreProxy wraps a Datastore and adds initialization checks
#[derive(Clone)]
pub struct DatastoreProxy {
    inner: Arc<RwLock<Option<Box<dyn Datastore + Send + Sync>>>>,
}

impl DatastoreProxy {
    /// Creates a new empty datastore proxy
    pub fn new() -> Self {
        Self {
            inner: Arc::new(RwLock::new(None)),
        }
    }

    /// Initializes the datastore based on configuration
    pub async fn init(&self) -> Result<()> {
        let datastore = create_datastore().await?;
        *self.inner.write().await = Some(datastore);
        Ok(())
    }

    /// Sets a custom datastore implementation
    pub async fn set_datastore(&self, datastore: Box<dyn Datastore + Send + Sync>) {
        *self.inner.write().await = Some(datastore);
    }

    /// Clones the inner Arc for sharing
    pub fn clone_inner(&self) -> DatastoreProxy {
        DatastoreProxy {
            inner: self.inner.clone(),
        }
    }

    /// Checks if the datastore is initialized
    pub async fn check_init(&self) -> Result<()> {
        if self.inner.read().await.is_none() {
            return Err(anyhow::anyhow!(
                "Datastore not initialized. You must call engine.Start() first"
            ));
        }
        Ok(())
    }
}

impl Default for DatastoreProxy {
    fn default() -> Self {
        Self::new()
    }
}

/// Get a string from environment variables (TORK_ prefix converted to config key)
fn env_string(key: &str) -> String {
    let env_key = format!("TORK_{}", key.to_uppercase().replace('.', "_"));
    env::var(&env_key).unwrap_or_default()
}

/// Get a string with default from environment variables
fn env_string_default(key: &str, default: &str) -> String {
    let value = env_string(key);
    if value.is_empty() {
        default.to_string()
    } else {
        value
    }
}

/// Creates a datastore based on configuration
/// 
/// Note: Currently returns an error as the datastore implementations
/// need to be aligned with the tork::Datastore trait.
pub async fn create_datastore() -> Result<Box<dyn Datastore + Send + Sync>> {
    let dstype = env_string_default("datastore.type", "inmemory");

    match dstype.as_str() {
        "inmemory" => Err(anyhow::anyhow!(
            "In-memory datastore not yet implemented. \
            Use TORK_DATASTORE_TYPE=postgres with a running database, \
            or implement the tork::Datastore trait for an in-memory backend."
        )),
        _ => Err(anyhow::anyhow!("unknown datastore type: {}. Use 'inmemory' or 'postgres'", dstype)),
    }
}

impl Datastore for DatastoreProxy {
    fn create_task(&self, task: Task) -> tork::datastore::BoxedFuture<()> {
        let inner = self.inner.clone();
        Box::pin(async move {
            let guard = inner.read().await;
            if let Some(ds) = guard.as_ref() {
                ds.create_task(task)
            } else {
                Box::pin(async { Err(anyhow::anyhow!("Datastore not initialized").into()) })
            }
            .await
        })
    }

    fn update_task(&self, id: String, task: Task) -> tork::datastore::BoxedFuture<()> {
        let inner = self.inner.clone();
        Box::pin(async move {
            let guard = inner.read().await;
            if let Some(ds) = guard.as_ref() {
                ds.update_task(id, task)
            } else {
                Box::pin(async { Err(anyhow::anyhow!("Datastore not initialized").into()) })
            }
            .await
        })
    }

    fn get_task_by_id(&self, id: String) -> tork::datastore::BoxedFuture<Option<Task>> {
        let inner = self.inner.clone();
        Box::pin(async move {
            let guard = inner.read().await;
            if let Some(ds) = guard.as_ref() {
                ds.get_task_by_id(id)
            } else {
                Box::pin(async { Err(anyhow::anyhow!("Datastore not initialized").into()) })
            }
            .await
        })
    }

    fn get_active_tasks(&self, job_id: String) -> tork::datastore::BoxedFuture<Vec<Task>> {
        let inner = self.inner.clone();
        Box::pin(async move {
            let guard = inner.read().await;
            if let Some(ds) = guard.as_ref() {
                ds.get_active_tasks(job_id)
            } else {
                Box::pin(async { Err(anyhow::anyhow!("Datastore not initialized").into()) })
            }
            .await
        })
    }

    fn get_next_task(&self, parent_task_id: String) -> tork::datastore::BoxedFuture<Option<Task>> {
        let inner = self.inner.clone();
        Box::pin(async move {
            let guard = inner.read().await;
            if let Some(ds) = guard.as_ref() {
                ds.get_next_task(parent_task_id)
            } else {
                Box::pin(async { Err(anyhow::anyhow!("Datastore not initialized").into()) })
            }
            .await
        })
    }

    fn create_task_log_part(&self, part: TaskLogPart) -> tork::datastore::BoxedFuture<()> {
        let inner = self.inner.clone();
        Box::pin(async move {
            let guard = inner.read().await;
            if let Some(ds) = guard.as_ref() {
                ds.create_task_log_part(part)
            } else {
                Box::pin(async { Err(anyhow::anyhow!("Datastore not initialized").into()) })
            }
            .await
        })
    }

    fn get_task_log_parts(
        &self,
        task_id: String,
        q: String,
        page: i64,
        size: i64,
    ) -> tork::datastore::BoxedFuture<Page<TaskLogPart>> {
        let inner = self.inner.clone();
        Box::pin(async move {
            let guard = inner.read().await;
            if let Some(ds) = guard.as_ref() {
                ds.get_task_log_parts(task_id, q, page, size)
            } else {
                Box::pin(async { Err(anyhow::anyhow!("Datastore not initialized").into()) })
            }
            .await
        })
    }

    fn create_node(&self, node: Node) -> tork::datastore::BoxedFuture<()> {
        let inner = self.inner.clone();
        Box::pin(async move {
            let guard = inner.read().await;
            if let Some(ds) = guard.as_ref() {
                ds.create_node(node)
            } else {
                Box::pin(async { Err(anyhow::anyhow!("Datastore not initialized").into()) })
            }
            .await
        })
    }

    fn update_node(&self, id: String, node: Node) -> tork::datastore::BoxedFuture<()> {
        let inner = self.inner.clone();
        Box::pin(async move {
            let guard = inner.read().await;
            if let Some(ds) = guard.as_ref() {
                ds.update_node(id, node)
            } else {
                Box::pin(async { Err(anyhow::anyhow!("Datastore not initialized").into()) })
            }
            .await
        })
    }

    fn get_node_by_id(&self, id: String) -> tork::datastore::BoxedFuture<Option<Node>> {
        let inner = self.inner.clone();
        Box::pin(async move {
            let guard = inner.read().await;
            if let Some(ds) = guard.as_ref() {
                ds.get_node_by_id(id)
            } else {
                Box::pin(async { Err(anyhow::anyhow!("Datastore not initialized").into()) })
            }
            .await
        })
    }

    fn get_active_nodes(&self) -> tork::datastore::BoxedFuture<Vec<Node>> {
        let inner = self.inner.clone();
        Box::pin(async move {
            let guard = inner.read().await;
            if let Some(ds) = guard.as_ref() {
                ds.get_active_nodes()
            } else {
                Box::pin(async { Err(anyhow::anyhow!("Datastore not initialized").into()) })
            }
            .await
        })
    }

    fn create_job(&self, job: Job) -> tork::datastore::BoxedFuture<()> {
        let inner = self.inner.clone();
        Box::pin(async move {
            let guard = inner.read().await;
            if let Some(ds) = guard.as_ref() {
                ds.create_job(job)
            } else {
                Box::pin(async { Err(anyhow::anyhow!("Datastore not initialized").into()) })
            }
            .await
        })
    }

    fn update_job(&self, id: String, job: Job) -> tork::datastore::BoxedFuture<()> {
        let inner = self.inner.clone();
        Box::pin(async move {
            let guard = inner.read().await;
            if let Some(ds) = guard.as_ref() {
                ds.update_job(id, job)
            } else {
                Box::pin(async { Err(anyhow::anyhow!("Datastore not initialized").into()) })
            }
            .await
        })
    }

    fn get_job_by_id(&self, id: String) -> tork::datastore::BoxedFuture<Option<Job>> {
        let inner = self.inner.clone();
        Box::pin(async move {
            let guard = inner.read().await;
            if let Some(ds) = guard.as_ref() {
                ds.get_job_by_id(id)
            } else {
                Box::pin(async { Err(anyhow::anyhow!("Datastore not initialized").into()) })
            }
            .await
        })
    }

    fn get_job_log_parts(
        &self,
        job_id: String,
        q: String,
        page: i64,
        size: i64,
    ) -> tork::datastore::BoxedFuture<Page<TaskLogPart>> {
        let inner = self.inner.clone();
        Box::pin(async move {
            let guard = inner.read().await;
            if let Some(ds) = guard.as_ref() {
                ds.get_job_log_parts(job_id, q, page, size)
            } else {
                Box::pin(async { Err(anyhow::anyhow!("Datastore not initialized").into()) })
            }
            .await
        })
    }

    fn get_jobs(
        &self,
        current_user: String,
        q: String,
        page: i64,
        size: i64,
    ) -> tork::datastore::BoxedFuture<Page<JobSummary>> {
        let inner = self.inner.clone();
        Box::pin(async move {
            let guard = inner.read().await;
            if let Some(ds) = guard.as_ref() {
                ds.get_jobs(current_user, q, page, size)
            } else {
                Box::pin(async { Err(anyhow::anyhow!("Datastore not initialized").into()) })
            }
            .await
        })
    }

    fn create_scheduled_job(&self, job: ScheduledJob) -> tork::datastore::BoxedFuture<()> {
        let inner = self.inner.clone();
        Box::pin(async move {
            let guard = inner.read().await;
            if let Some(ds) = guard.as_ref() {
                ds.create_scheduled_job(job)
            } else {
                Box::pin(async { Err(anyhow::anyhow!("Datastore not initialized").into()) })
            }
            .await
        })
    }

    fn get_active_scheduled_jobs(&self) -> tork::datastore::BoxedFuture<Vec<ScheduledJob>> {
        let inner = self.inner.clone();
        Box::pin(async move {
            let guard = inner.read().await;
            if let Some(ds) = guard.as_ref() {
                ds.get_active_scheduled_jobs()
            } else {
                Box::pin(async { Err(anyhow::anyhow!("Datastore not initialized").into()) })
            }
            .await
        })
    }

    fn get_scheduled_jobs(
        &self,
        current_user: String,
        page: i64,
        size: i64,
    ) -> tork::datastore::BoxedFuture<Page<ScheduledJobSummary>> {
        let inner = self.inner.clone();
        Box::pin(async move {
            let guard = inner.read().await;
            if let Some(ds) = guard.as_ref() {
                ds.get_scheduled_jobs(current_user, page, size)
            } else {
                Box::pin(async { Err(anyhow::anyhow!("Datastore not initialized").into()) })
            }
            .await
        })
    }

    fn get_scheduled_job_by_id(
        &self,
        id: String,
    ) -> tork::datastore::BoxedFuture<Option<ScheduledJob>> {
        let inner = self.inner.clone();
        Box::pin(async move {
            let guard = inner.read().await;
            if let Some(ds) = guard.as_ref() {
                ds.get_scheduled_job_by_id(id)
            } else {
                Box::pin(async { Err(anyhow::anyhow!("Datastore not initialized").into()) })
            }
            .await
        })
    }

    fn update_scheduled_job(
        &self,
        id: String,
        job: ScheduledJob,
    ) -> tork::datastore::BoxedFuture<()> {
        let inner = self.inner.clone();
        Box::pin(async move {
            let guard = inner.read().await;
            if let Some(ds) = guard.as_ref() {
                ds.update_scheduled_job(id, job)
            } else {
                Box::pin(async { Err(anyhow::anyhow!("Datastore not initialized").into()) })
            }
            .await
        })
    }

    fn delete_scheduled_job(&self, id: String) -> tork::datastore::BoxedFuture<()> {
        let inner = self.inner.clone();
        Box::pin(async move {
            let guard = inner.read().await;
            if let Some(ds) = guard.as_ref() {
                ds.delete_scheduled_job(id)
            } else {
                Box::pin(async { Err(anyhow::anyhow!("Datastore not initialized").into()) })
            }
            .await
        })
    }

    fn create_user(&self, user: User) -> tork::datastore::BoxedFuture<()> {
        let inner = self.inner.clone();
        Box::pin(async move {
            let guard = inner.read().await;
            if let Some(ds) = guard.as_ref() {
                ds.create_user(user)
            } else {
                Box::pin(async { Err(anyhow::anyhow!("Datastore not initialized").into()) })
            }
            .await
        })
    }

    fn get_user(&self, username: String) -> tork::datastore::BoxedFuture<Option<User>> {
        let inner = self.inner.clone();
        Box::pin(async move {
            let guard = inner.read().await;
            if let Some(ds) = guard.as_ref() {
                ds.get_user(username)
            } else {
                Box::pin(async { Err(anyhow::anyhow!("Datastore not initialized").into()) })
            }
            .await
        })
    }

    fn create_role(&self, role: Role) -> tork::datastore::BoxedFuture<()> {
        let inner = self.inner.clone();
        Box::pin(async move {
            let guard = inner.read().await;
            if let Some(ds) = guard.as_ref() {
                ds.create_role(role)
            } else {
                Box::pin(async { Err(anyhow::anyhow!("Datastore not initialized").into()) })
            }
            .await
        })
    }

    fn get_role(&self, id: String) -> tork::datastore::BoxedFuture<Option<Role>> {
        let inner = self.inner.clone();
        Box::pin(async move {
            let guard = inner.read().await;
            if let Some(ds) = guard.as_ref() {
                ds.get_role(id)
            } else {
                Box::pin(async { Err(anyhow::anyhow!("Datastore not initialized").into()) })
            }
            .await
        })
    }

    fn get_roles(&self) -> tork::datastore::BoxedFuture<Vec<Role>> {
        let inner = self.inner.clone();
        Box::pin(async move {
            let guard = inner.read().await;
            if let Some(ds) = guard.as_ref() {
                ds.get_roles()
            } else {
                Box::pin(async { Err(anyhow::anyhow!("Datastore not initialized").into()) })
            }
            .await
        })
    }

    fn get_metrics(&self) -> tork::datastore::BoxedFuture<Metrics> {
        let inner = self.inner.clone();
        Box::pin(async move {
            let guard = inner.read().await;
            if let Some(ds) = guard.as_ref() {
                ds.get_metrics()
            } else {
                Box::pin(async { Err(anyhow::anyhow!("Datastore not initialized").into()) })
            }
            .await
        })
    }

    fn health_check(&self) -> tork::datastore::BoxedFuture<()> {
        let inner = self.inner.clone();
        Box::pin(async move {
            let guard = inner.read().await;
            if let Some(ds) = guard.as_ref() {
                ds.health_check()
            } else {
                Box::pin(async { Err(anyhow::anyhow!("Datastore not initialized").into()) })
            }
            .await
        })
    }

    fn shutdown(&self) -> tork::datastore::BoxedFuture<()> {
        let inner = self.inner.clone();
        Box::pin(async move {
            let guard = inner.read().await;
            if let Some(ds) = guard.as_ref() {
                ds.shutdown()
            } else {
                Box::pin(async { Err(anyhow::anyhow!("Datastore not initialized").into()) })
            }
            .await
        })
    }
}
