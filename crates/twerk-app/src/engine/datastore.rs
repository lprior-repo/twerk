//! Datastore proxy module
//!
//! This module provides a proxy wrapper around the Datastore interface
//! that adds initialization checks, plus factory functions for creating
//! concrete datastore implementations.

use std::env;
use std::sync::Arc;

use anyhow::Result;
use dashmap::DashMap;
use tokio::sync::RwLock;
use async_trait::async_trait;

use twerk_infrastructure::datastore::{Datastore, Page, Error as DatastoreError};
use twerk_core::id::{JobId, NodeId, ScheduledJobId, TaskId};
use twerk_core::job::{Job, JobSummary, ScheduledJob, ScheduledJobSummary};
use twerk_core::node::Node;
use twerk_core::role::Role;
use twerk_core::task::{Task, TaskLogPart};
use twerk_core::user::User;

// ── Datastore proxy ────────────────────────────────────────────

/// [`DatastoreProxy`] wraps a [`Datastore`] and adds initialization checks.
#[derive(Clone)]
pub struct DatastoreProxy {
    inner: Arc<RwLock<Option<Box<dyn Datastore + Send + Sync>>>>,
}

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
        match inner.as_ref() {
            Some(ds) => ds.create_task(task).await,
            None => Err(DatastoreError::Database("Datastore not initialized".to_string())),
        }
    }

    async fn update_task(&self, id: &str, modify: Box<dyn FnOnce(Task) -> twerk_infrastructure::datastore::Result<Task> + Send>) -> twerk_infrastructure::datastore::Result<()> {
        let inner = self.inner.read().await;
        match inner.as_ref() {
            Some(ds) => ds.update_task(id, modify).await,
            None => Err(DatastoreError::Database("Datastore not initialized".to_string())),
        }
    }

    async fn get_task_by_id(&self, id: &str) -> twerk_infrastructure::datastore::Result<Task> {
        let inner = self.inner.read().await;
        match inner.as_ref() {
            Some(ds) => ds.get_task_by_id(id).await,
            None => Err(DatastoreError::Database("Datastore not initialized".to_string())),
        }
    }

    async fn get_active_tasks(&self, job_id: &str) -> twerk_infrastructure::datastore::Result<Vec<Task>> {
        let inner = self.inner.read().await;
        match inner.as_ref() {
            Some(ds) => ds.get_active_tasks(job_id).await,
            None => Err(DatastoreError::Database("Datastore not initialized".to_string())),
        }
    }

    async fn get_next_task(&self, parent_task_id: &str) -> twerk_infrastructure::datastore::Result<Task> {
        let inner = self.inner.read().await;
        match inner.as_ref() {
            Some(ds) => ds.get_next_task(parent_task_id).await,
            None => Err(DatastoreError::Database("Datastore not initialized".to_string())),
        }
    }

    async fn create_task_log_part(&self, part: &TaskLogPart) -> twerk_infrastructure::datastore::Result<()> {
        let inner = self.inner.read().await;
        match inner.as_ref() {
            Some(ds) => ds.create_task_log_part(part).await,
            None => Err(DatastoreError::Database("Datastore not initialized".to_string())),
        }
    }

    async fn get_task_log_parts(&self, task_id: &str, q: &str, page: i64, size: i64) -> twerk_infrastructure::datastore::Result<Page<TaskLogPart>> {
        let inner = self.inner.read().await;
        match inner.as_ref() {
            Some(ds) => ds.get_task_log_parts(task_id, q, page, size).await,
            None => Err(DatastoreError::Database("Datastore not initialized".to_string())),
        }
    }

    async fn create_node(&self, node: &Node) -> twerk_infrastructure::datastore::Result<()> {
        let inner = self.inner.read().await;
        match inner.as_ref() {
            Some(ds) => ds.create_node(node).await,
            None => Err(DatastoreError::Database("Datastore not initialized".to_string())),
        }
    }

    async fn update_node(&self, id: &str, modify: Box<dyn FnOnce(Node) -> twerk_infrastructure::datastore::Result<Node> + Send>) -> twerk_infrastructure::datastore::Result<()> {
        let inner = self.inner.read().await;
        match inner.as_ref() {
            Some(ds) => ds.update_node(id, modify).await,
            None => Err(DatastoreError::Database("Datastore not initialized".to_string())),
        }
    }

    async fn get_node_by_id(&self, id: &str) -> twerk_infrastructure::datastore::Result<Node> {
        let inner = self.inner.read().await;
        match inner.as_ref() {
            Some(ds) => ds.get_node_by_id(id).await,
            None => Err(DatastoreError::Database("Datastore not initialized".to_string())),
        }
    }

    async fn get_active_nodes(&self) -> twerk_infrastructure::datastore::Result<Vec<Node>> {
        let inner = self.inner.read().await;
        match inner.as_ref() {
            Some(ds) => ds.get_active_nodes().await,
            None => Err(DatastoreError::Database("Datastore not initialized".to_string())),
        }
    }

    async fn create_job(&self, job: &Job) -> twerk_infrastructure::datastore::Result<()> {
        let inner = self.inner.read().await;
        match inner.as_ref() {
            Some(ds) => ds.create_job(job).await,
            None => Err(DatastoreError::Database("Datastore not initialized".to_string())),
        }
    }

    async fn update_job(&self, id: &str, modify: Box<dyn FnOnce(Job) -> twerk_infrastructure::datastore::Result<Job> + Send>) -> twerk_infrastructure::datastore::Result<()> {
        let inner = self.inner.read().await;
        match inner.as_ref() {
            Some(ds) => ds.update_job(id, modify).await,
            None => Err(DatastoreError::Database("Datastore not initialized".to_string())),
        }
    }

    async fn get_job_by_id(&self, id: &str) -> twerk_infrastructure::datastore::Result<Job> {
        let inner = self.inner.read().await;
        match inner.as_ref() {
            Some(ds) => ds.get_job_by_id(id).await,
            None => Err(DatastoreError::Database("Datastore not initialized".to_string())),
        }
    }

    async fn get_job_log_parts(&self, job_id: &str, q: &str, page: i64, size: i64) -> twerk_infrastructure::datastore::Result<Page<TaskLogPart>> {
        let inner = self.inner.read().await;
        match inner.as_ref() {
            Some(ds) => ds.get_job_log_parts(job_id, q, page, size).await,
            None => Err(DatastoreError::Database("Datastore not initialized".to_string())),
        }
    }

    async fn get_jobs(&self, current_user: &str, q: &str, page: i64, size: i64) -> twerk_infrastructure::datastore::Result<Page<JobSummary>> {
        let inner = self.inner.read().await;
        match inner.as_ref() {
            Some(ds) => ds.get_jobs(current_user, q, page, size).await,
            None => Err(DatastoreError::Database("Datastore not initialized".to_string())),
        }
    }

    async fn create_scheduled_job(&self, sj: &ScheduledJob) -> twerk_infrastructure::datastore::Result<()> {
        let inner = self.inner.read().await;
        match inner.as_ref() {
            Some(ds) => ds.create_scheduled_job(sj).await,
            None => Err(DatastoreError::Database("Datastore not initialized".to_string())),
        }
    }

    async fn get_active_scheduled_jobs(&self) -> twerk_infrastructure::datastore::Result<Vec<ScheduledJob>> {
        let inner = self.inner.read().await;
        match inner.as_ref() {
            Some(ds) => ds.get_active_scheduled_jobs().await,
            None => Err(DatastoreError::Database("Datastore not initialized".to_string())),
        }
    }

    async fn get_scheduled_jobs(&self, current_user: &str, page: i64, size: i64) -> twerk_infrastructure::datastore::Result<Page<ScheduledJobSummary>> {
        let inner = self.inner.read().await;
        match inner.as_ref() {
            Some(ds) => ds.get_scheduled_jobs(current_user, page, size).await,
            None => Err(DatastoreError::Database("Datastore not initialized".to_string())),
        }
    }

    async fn get_scheduled_job_by_id(&self, id: &str) -> twerk_infrastructure::datastore::Result<ScheduledJob> {
        let inner = self.inner.read().await;
        match inner.as_ref() {
            Some(ds) => ds.get_scheduled_job_by_id(id).await,
            None => Err(DatastoreError::Database("Datastore not initialized".to_string())),
        }
    }

    async fn update_scheduled_job(&self, id: &str, modify: Box<dyn FnOnce(ScheduledJob) -> twerk_infrastructure::datastore::Result<ScheduledJob> + Send>) -> twerk_infrastructure::datastore::Result<()> {
        let inner = self.inner.read().await;
        match inner.as_ref() {
            Some(ds) => ds.update_scheduled_job(id, modify).await,
            None => Err(DatastoreError::Database("Datastore not initialized".to_string())),
        }
    }

    async fn delete_scheduled_job(&self, id: &str) -> twerk_infrastructure::datastore::Result<()> {
        let inner = self.inner.read().await;
        match inner.as_ref() {
            Some(ds) => ds.delete_scheduled_job(id).await,
            None => Err(DatastoreError::Database("Datastore not initialized".to_string())),
        }
    }

    async fn create_user(&self, user: &User) -> twerk_infrastructure::datastore::Result<()> {
        let inner = self.inner.read().await;
        match inner.as_ref() {
            Some(ds) => ds.create_user(user).await,
            None => Err(DatastoreError::Database("Datastore not initialized".to_string())),
        }
    }

    async fn get_user(&self, username: &str) -> twerk_infrastructure::datastore::Result<User> {
        let inner = self.inner.read().await;
        match inner.as_ref() {
            Some(ds) => ds.get_user(username).await,
            None => Err(DatastoreError::Database("Datastore not initialized".to_string())),
        }
    }

    async fn create_role(&self, role: &Role) -> twerk_infrastructure::datastore::Result<()> {
        let inner = self.inner.read().await;
        match inner.as_ref() {
            Some(ds) => ds.create_role(role).await,
            None => Err(DatastoreError::Database("Datastore not initialized".to_string())),
        }
    }

    async fn get_role(&self, id: &str) -> twerk_infrastructure::datastore::Result<Role> {
        let inner = self.inner.read().await;
        match inner.as_ref() {
            Some(ds) => ds.get_role(id).await,
            None => Err(DatastoreError::Database("Datastore not initialized".to_string())),
        }
    }

    async fn get_roles(&self) -> twerk_infrastructure::datastore::Result<Vec<Role>> {
        let inner = self.inner.read().await;
        match inner.as_ref() {
            Some(ds) => ds.get_roles().await,
            None => Err(DatastoreError::Database("Datastore not initialized".to_string())),
        }
    }

    async fn get_user_roles(&self, user_id: &str) -> twerk_infrastructure::datastore::Result<Vec<Role>> {
        let inner = self.inner.read().await;
        match inner.as_ref() {
            Some(ds) => ds.get_user_roles(user_id).await,
            None => Err(DatastoreError::Database("Datastore not initialized".to_string())),
        }
    }

    async fn assign_role(&self, user_id: &str, role_id: &str) -> twerk_infrastructure::datastore::Result<()> {
        let inner = self.inner.read().await;
        match inner.as_ref() {
            Some(ds) => ds.assign_role(user_id, role_id).await,
            None => Err(DatastoreError::Database("Datastore not initialized".to_string())),
        }
    }

    async fn unassign_role(&self, user_id: &str, role_id: &str) -> twerk_infrastructure::datastore::Result<()> {
        let inner = self.inner.read().await;
        match inner.as_ref() {
            Some(ds) => ds.unassign_role(user_id, role_id).await,
            None => Err(DatastoreError::Database("Datastore not initialized".to_string())),
        }
    }

    async fn get_metrics(&self) -> twerk_infrastructure::datastore::Result<twerk_core::stats::Metrics> {
        let inner = self.inner.read().await;
        match inner.as_ref() {
            Some(ds) => ds.get_metrics().await,
            None => Err(DatastoreError::Database("Datastore not initialized".to_string())),
        }
    }

    async fn with_tx(
        &self,
        f: Box<dyn for<'a> FnOnce(&'a dyn Datastore) -> futures_util::future::BoxFuture<'a, twerk_infrastructure::datastore::Result<()>> + Send>,
    ) -> twerk_infrastructure::datastore::Result<()> {
        let inner = self.inner.read().await;
        match inner.as_ref() {
            Some(ds) => ds.with_tx(f).await,
            None => Err(DatastoreError::Database("Datastore not initialized".to_string())),
        }
    }

    async fn health_check(&self) -> twerk_infrastructure::datastore::Result<()> {
        let inner = self.inner.read().await;
        match inner.as_ref() {
            Some(ds) => ds.health_check().await,
            None => Err(DatastoreError::Database("Datastore not initialized".to_string())),
        }
    }
}

impl Default for DatastoreProxy {
    fn default() -> Self {
        Self::new()
    }
}

// ── In-memory datastore ────────────────────────────────────────

pub struct InMemoryDatastore {
    tasks: Arc<DashMap<TaskId, Task>>,
    nodes: Arc<DashMap<NodeId, Node>>,
    jobs: Arc<DashMap<JobId, Job>>,
    users: Arc<DashMap<String, User>>,
    roles: Arc<DashMap<String, Role>>,
    scheduled_jobs: Arc<DashMap<ScheduledJobId, ScheduledJob>>,
    task_log_parts: Arc<DashMap<TaskId, Vec<TaskLogPart>>>,
    user_roles: Arc<DashMap<String, Vec<String>>>,
}

impl InMemoryDatastore {
    pub fn new() -> Self {
        Self {
            tasks: Arc::new(DashMap::new()),
            nodes: Arc::new(DashMap::new()),
            jobs: Arc::new(DashMap::new()),
            users: Arc::new(DashMap::new()),
            roles: Arc::new(DashMap::new()),
            scheduled_jobs: Arc::new(DashMap::new()),
            task_log_parts: Arc::new(DashMap::new()),
            user_roles: Arc::new(DashMap::new()),
        }
    }

    fn paginate<T: Clone>(items: Vec<T>, page: i64, size: i64) -> Page<T> {
        let total = items.len() as i64;
        let skip = ((page - 1).max(0) * size) as usize;
        let paged: Vec<T> = items.into_iter().skip(skip).take(size as usize).collect();
        Page {
            items: paged,
            number: page,
            size,
            total_pages: (total as f64 / size as f64).ceil() as i64,
            total_items: total,
        }
    }
}

#[async_trait]
impl Datastore for InMemoryDatastore {
    async fn create_task(&self, task: &Task) -> twerk_infrastructure::datastore::Result<()> {
        let id = task.id.clone().ok_or_else(|| DatastoreError::InvalidInput("id required".to_string()))?;
        self.tasks.insert(id, task.clone());
        Ok(())
    }

    async fn update_task(&self, id: &str, modify: Box<dyn FnOnce(Task) -> twerk_infrastructure::datastore::Result<Task> + Send>) -> twerk_infrastructure::datastore::Result<()> {
        let mut task = self.tasks.get(id).map(|r| r.value().clone()).ok_or(DatastoreError::TaskNotFound)?;
        task = modify(task)?;
        self.tasks.insert(TaskId::new(id), task);
        Ok(())
    }

    async fn get_task_by_id(&self, id: &str) -> twerk_infrastructure::datastore::Result<Task> {
        self.tasks.get(id).map(|r| r.value().clone()).ok_or(DatastoreError::TaskNotFound)
    }

    async fn get_active_tasks(&self, job_id: &str) -> twerk_infrastructure::datastore::Result<Vec<Task>> {
        Ok(self.tasks.iter()
            .filter(|e| e.value().job_id.as_deref() == Some(job_id) && e.value().is_active())
            .map(|e| e.value().clone())
            .collect())
    }

    async fn get_next_task(&self, parent_task_id: &str) -> twerk_infrastructure::datastore::Result<Task> {
        self.tasks.iter()
            .find(|e| e.value().parent_id.as_deref() == Some(parent_task_id) && e.value().state == "CREATED")
            .map(|e| e.value().clone())
            .ok_or(DatastoreError::TaskNotFound)
    }

    async fn create_task_log_part(&self, part: &TaskLogPart) -> twerk_infrastructure::datastore::Result<()> {
        let task_id = part.task_id.clone().ok_or_else(|| DatastoreError::InvalidInput("task_id required".to_string()))?;
        self.task_log_parts.entry(task_id).or_default().push(part.clone());
        Ok(())
    }

    async fn get_task_log_parts(&self, task_id: &str, _q: &str, page: i64, size: i64) -> twerk_infrastructure::datastore::Result<Page<TaskLogPart>> {
        let parts = self.task_log_parts.get(task_id).map(|r| r.value().clone()).unwrap_or_default();
        Ok(Self::paginate(parts, page, size))
    }

    async fn create_node(&self, node: &Node) -> twerk_infrastructure::datastore::Result<()> {
        let id = node.id.clone().ok_or_else(|| DatastoreError::InvalidInput("id required".to_string()))?;
        self.nodes.insert(id, node.clone());
        Ok(())
    }

    async fn update_node(&self, id: &str, modify: Box<dyn FnOnce(Node) -> twerk_infrastructure::datastore::Result<Node> + Send>) -> twerk_infrastructure::datastore::Result<()> {
        let mut node = self.nodes.get(id).map(|r| r.value().clone()).ok_or(DatastoreError::NodeNotFound)?;
        node = modify(node)?;
        self.nodes.insert(NodeId::new(id), node);
        Ok(())
    }

    async fn get_node_by_id(&self, id: &str) -> twerk_infrastructure::datastore::Result<Node> {
        self.nodes.get(id).map(|r| r.value().clone()).ok_or(DatastoreError::NodeNotFound)
    }

    async fn get_active_nodes(&self) -> twerk_infrastructure::datastore::Result<Vec<Node>> {
        Ok(self.nodes.iter().map(|e| e.value().clone()).collect())
    }

    async fn create_job(&self, job: &Job) -> twerk_infrastructure::datastore::Result<()> {
        let id = job.id.clone().ok_or_else(|| DatastoreError::InvalidInput("id required".to_string()))?;
        self.jobs.insert(id, job.clone());
        Ok(())
    }

    async fn update_job(&self, id: &str, modify: Box<dyn FnOnce(Job) -> twerk_infrastructure::datastore::Result<Job> + Send>) -> twerk_infrastructure::datastore::Result<()> {
        let mut job = self.jobs.get(id).map(|r| r.value().clone()).ok_or(DatastoreError::JobNotFound)?;
        job = modify(job)?;
        self.jobs.insert(JobId::new(id), job);
        Ok(())
    }

    async fn get_job_by_id(&self, id: &str) -> twerk_infrastructure::datastore::Result<Job> {
        self.jobs.get(id).map(|r| r.value().clone()).ok_or(DatastoreError::JobNotFound)
    }

    async fn get_job_log_parts(&self, job_id: &str, _q: &str, page: i64, size: i64) -> twerk_infrastructure::datastore::Result<Page<TaskLogPart>> {
        let task_ids: Vec<TaskId> = self.tasks.iter()
            .filter(|e| e.value().job_id.as_deref() == Some(job_id))
            .filter_map(|e| e.value().id.clone())
            .collect();
        let mut all_parts = Vec::new();
        for tid in task_ids {
            if let Some(parts) = self.task_log_parts.get(&tid) {
                all_parts.extend(parts.value().clone());
            }
        }
        Ok(Self::paginate(all_parts, page, size))
    }

    async fn get_jobs(&self, _current_user: &str, _q: &str, page: i64, size: i64) -> twerk_infrastructure::datastore::Result<Page<JobSummary>> {
        let summaries: Vec<JobSummary> = self.jobs.iter()
            .map(|e| twerk_core::job::new_job_summary(e.value()))
            .collect();
        Ok(Self::paginate(summaries, page, size))
    }

    async fn create_scheduled_job(&self, sj: &ScheduledJob) -> twerk_infrastructure::datastore::Result<()> {
        let id = sj.id.clone().ok_or_else(|| DatastoreError::InvalidInput("id required".to_string()))?;
        self.scheduled_jobs.insert(id, sj.clone());
        Ok(())
    }

    async fn get_active_scheduled_jobs(&self) -> twerk_infrastructure::datastore::Result<Vec<ScheduledJob>> {
        Ok(self.scheduled_jobs.iter()
            .filter(|e| e.value().state == "ACTIVE")
            .map(|e| e.value().clone())
            .collect())
    }

    async fn get_scheduled_jobs(&self, _current_user: &str, page: i64, size: i64) -> twerk_infrastructure::datastore::Result<Page<ScheduledJobSummary>> {
        let summaries: Vec<ScheduledJobSummary> = self.scheduled_jobs.iter()
            .map(|e| twerk_core::job::new_scheduled_job_summary(e.value()))
            .collect();
        Ok(Self::paginate(summaries, page, size))
    }

    async fn get_scheduled_job_by_id(&self, id: &str) -> twerk_infrastructure::datastore::Result<ScheduledJob> {
        self.scheduled_jobs.get(id).map(|r| r.value().clone()).ok_or(DatastoreError::ScheduledJobNotFound)
    }

    async fn update_scheduled_job(&self, id: &str, modify: Box<dyn FnOnce(ScheduledJob) -> twerk_infrastructure::datastore::Result<ScheduledJob> + Send>) -> twerk_infrastructure::datastore::Result<()> {
        let mut sj = self.scheduled_jobs.get(id).map(|r| r.value().clone()).ok_or(DatastoreError::ScheduledJobNotFound)?;
        sj = modify(sj)?;
        self.scheduled_jobs.insert(ScheduledJobId::new(id), sj);
        Ok(())
    }

    async fn delete_scheduled_job(&self, id: &str) -> twerk_infrastructure::datastore::Result<()> {
        self.scheduled_jobs.remove(id);
        Ok(())
    }

    async fn create_user(&self, user: &User) -> twerk_infrastructure::datastore::Result<()> {
        let id = user.id.clone().ok_or_else(|| DatastoreError::InvalidInput("id required".to_string()))?;
        if let Some(ref username) = user.username {
            self.users.insert(username.clone(), user.clone());
        }
        self.users.insert(id, user.clone());
        Ok(())
    }

    async fn get_user(&self, username: &str) -> twerk_infrastructure::datastore::Result<User> {
        self.users.get(username).map(|r| r.value().clone()).ok_or(DatastoreError::UserNotFound)
    }

    async fn create_role(&self, role: &Role) -> twerk_infrastructure::datastore::Result<()> {
        let id = role.id.clone().ok_or_else(|| DatastoreError::InvalidInput("id required".to_string()))?;
        self.roles.insert(id, role.clone());
        Ok(())
    }

    async fn get_role(&self, id: &str) -> twerk_infrastructure::datastore::Result<Role> {
        self.roles.get(id).map(|r| r.value().clone()).ok_or(DatastoreError::RoleNotFound)
    }

    async fn get_roles(&self) -> twerk_infrastructure::datastore::Result<Vec<Role>> {
        Ok(self.roles.iter().map(|e| e.value().clone()).collect())
    }

    async fn get_user_roles(&self, user_id: &str) -> twerk_infrastructure::datastore::Result<Vec<Role>> {
        let role_ids = self.user_roles.get(user_id).map(|r| r.value().clone()).unwrap_or_default();
        Ok(role_ids.iter().filter_map(|rid| self.roles.get(rid).map(|r| r.value().clone())).collect())
    }

    async fn assign_role(&self, user_id: &str, role_id: &str) -> twerk_infrastructure::datastore::Result<()> {
        self.user_roles.entry(user_id.to_string()).or_default().push(role_id.to_string());
        Ok(())
    }

    async fn unassign_role(&self, user_id: &str, role_id: &str) -> twerk_infrastructure::datastore::Result<()> {
        if let Some(mut roles) = self.user_roles.get_mut(user_id) {
            roles.retain(|r| r != role_id);
        }
        Ok(())
    }

    async fn get_metrics(&self) -> twerk_infrastructure::datastore::Result<twerk_core::stats::Metrics> {
        let jobs_running = self.jobs.iter().filter(|e| e.value().state == "RUNNING").count() as i32;
        let tasks_running = self.tasks.iter().filter(|e| e.value().state == "RUNNING").count() as i32;
        let nodes_running = self.nodes.len() as i32;
        Ok(twerk_core::stats::Metrics {
            jobs: twerk_core::stats::JobMetrics { running: jobs_running },
            tasks: twerk_core::stats::TaskMetrics { running: tasks_running },
            nodes: twerk_core::stats::NodeMetrics { running: nodes_running, cpu_percent: 0.0 },
        })
    }

    async fn with_tx(
        &self,
        f: Box<dyn for<'a> FnOnce(&'a dyn Datastore) -> futures_util::future::BoxFuture<'a, twerk_infrastructure::datastore::Result<()>> + Send>,
    ) -> twerk_infrastructure::datastore::Result<()> {
        // In-memory doesn't support transactions, just run it
        f(self).await
    }

    async fn health_check(&self) -> twerk_infrastructure::datastore::Result<()> {
        Ok(())
    }
}

impl Default for InMemoryDatastore {
    fn default() -> Self {
        Self::new()
    }
}

// ── Datastore factory ──────────────────────────────────────────

const DEFAULT_POSTGRES_DSN: &str = "host=localhost user=twerk password=twerk dbname=twerk port=5432 sslmode=disable";

pub async fn create_datastore() -> Result<Box<dyn Datastore + Send + Sync>> {
    let dstype = env_string_default("datastore.type", "postgres");

    match dstype.as_str() {
        "postgres" => {
            let dsn = env_string_default("datastore.postgres.dsn", DEFAULT_POSTGRES_DSN);
            let opts = twerk_infrastructure::datastore::Options {
                encryption_key: Some(env_string("datastore.encryption.key")).filter(|s| !s.is_empty()),
                ..Default::default()
            };
            let pg = twerk_infrastructure::datastore::postgres::PostgresDatastore::new(&dsn, opts).await
                .map_err(|e| anyhow::anyhow!("unable to connect to postgres: {}", e))?;
            Ok(Box::new(pg))
        }
        "inmemory" => Ok(Box::new(InMemoryDatastore::new())),
        other => Err(anyhow::anyhow!("unknown datastore type: {}", other)),
    }
}

fn env_string(key: &str) -> String {
    let env_key = format!("TWERK_{}", key.to_uppercase().replace('.', "_"));
    env::var(&env_key).unwrap_or_default()
}

fn env_string_default(key: &str, default: &str) -> String {
    let v = env_string(key);
    if v.is_empty() { default.to_string() } else { v }
}

#[must_use]
pub fn new_inmemory_datastore() -> Box<dyn Datastore + Send + Sync> {
    Box::new(InMemoryDatastore::new())
}

#[must_use]
pub fn new_inmemory_datastore_arc() -> std::sync::Arc<dyn Datastore> {
    std::sync::Arc::new(InMemoryDatastore::new())
}
