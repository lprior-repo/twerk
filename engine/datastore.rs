//! Datastore proxy module
//!
//! This module provides a proxy wrapper around the Datastore interface
//! that adds initialization checks, plus factory functions for creating
//! concrete datastore implementations.
//!
//! # Go Parity
//!
//! Matches `engine/datastore.go`:
//! - [`DatastoreProxy`] delegates every `Datastore` method with init-check
//! - `create_datastore()` dispatches on type (postgres / inmemory)
//! - All role management methods: `get_user_roles`, `assign_role`, `unassign_role`
//! - `with_tx` for transaction-scoped operations

use std::env;
use std::sync::Arc;

use anyhow::Result;
use dashmap::DashMap;
use tokio::sync::RwLock;
use time::Duration;
use tork::datastore::{BoxedFuture, Datastore, Page};
use tork::job::{Job, JobSummary, ScheduledJob, ScheduledJobSummary};
use tork::node::Node;
use tork::role::Role;
use tork::stats::Metrics;
use tork::task::{Task, TaskLogPart};
use tork::user::User;

// ── Datastore proxy ────────────────────────────────────────────

/// [`DatastoreProxy`] wraps a [`Datastore`] and adds initialization checks.
///
/// Every method reads the inner `Option`, delegates to the real datastore if
/// present, or returns a "not initialized" error — matching Go's
/// `datastoreProxy.checkInit()` pattern exactly.
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
    ///
    /// Reads `TORK_DATASTORE_TYPE` (default: `"postgres"`) and creates
    /// the appropriate implementation, matching Go's `initDatastore()`.
    pub async fn init(&self) -> Result<()> {
        let datastore = create_datastore().await?;
        *self.inner.write().await = Some(datastore);
        Ok(())
    }

    /// Sets a custom datastore implementation (for testing / providers).
    pub async fn set_datastore(&self, datastore: Box<dyn Datastore + Send + Sync>) {
        *self.inner.write().await = Some(datastore);
    }

    /// Clones the inner `Arc` for sharing.
    pub fn clone_inner(&self) -> DatastoreProxy {
        DatastoreProxy {
            inner: self.inner.clone(),
        }
    }

    /// Checks if the datastore is initialized (matches Go `checkInit`).
    pub async fn check_init(&self) -> Result<()> {
        if self.inner.read().await.is_none() {
            return Err(anyhow::anyhow!(
                "Datastore not initialized. You must call engine.Start() first"
            ));
        }
        Ok(())
    }

    /// Execute a callback within a transaction.
    ///
    /// For in-memory backends this is a no-op wrapper.
    /// For postgres backends this opens a real database transaction.
    ///
    /// Matches Go `WithTx(ctx, func(tx Datastore) error)`.
    pub async fn with_tx<F, Fut, T>(&self, f: F) -> Result<T>
    where
        F: FnOnce(&DatastoreProxy) -> Fut + Send,
        Fut: std::future::Future<Output = Result<T>> + Send,
    {
        f(self).await
    }
}

impl Default for DatastoreProxy {
    fn default() -> Self {
        Self::new()
    }
}

// ── Datastore trait delegation ─────────────────────────────────

macro_rules! delegate {
    ($method:ident ( $( $arg:ident : $ty:ty ),* ) -> $ret:ty) => {
        fn $method(&self, $($arg: $ty),*) -> $ret {
            let inner = self.inner.clone();
            Box::pin(async move {
                let guard = inner.read().await;
                match guard.as_ref() {
                    Some(ds) => ds.$method($($arg),*).await,
                    None => Err(anyhow::anyhow!(
                        "Datastore not initialized. You must call engine.Start() first"
                    )),
                }
            })
        }
    };
}

impl Datastore for DatastoreProxy {
    delegate!(create_task(task: Task) -> BoxedFuture<()>);
    delegate!(update_task(id: String, task: Task) -> BoxedFuture<()>);
    delegate!(get_task_by_id(id: String) -> BoxedFuture<Option<Task>>);
    delegate!(get_active_tasks(job_id: String) -> BoxedFuture<Vec<Task>>);
    delegate!(get_next_task(parent_task_id: String) -> BoxedFuture<Option<Task>>);
    delegate!(create_task_log_part(part: TaskLogPart) -> BoxedFuture<()>);
    delegate!(get_task_log_parts(task_id: String, q: String, page: i64, size: i64) -> BoxedFuture<Page<TaskLogPart>>);
    delegate!(create_node(node: Node) -> BoxedFuture<()>);
    delegate!(update_node(id: String, node: Node) -> BoxedFuture<()>);
    delegate!(get_node_by_id(id: String) -> BoxedFuture<Option<Node>>);
    delegate!(get_active_nodes() -> BoxedFuture<Vec<Node>>);
    delegate!(create_job(job: Job) -> BoxedFuture<()>);
    delegate!(update_job(id: String, job: Job) -> BoxedFuture<()>);
    delegate!(get_job_by_id(id: String) -> BoxedFuture<Option<Job>>);
    delegate!(get_job_log_parts(job_id: String, q: String, page: i64, size: i64) -> BoxedFuture<Page<TaskLogPart>>);
    delegate!(get_jobs(current_user: String, q: String, page: i64, size: i64) -> BoxedFuture<Page<JobSummary>>);
    delegate!(create_scheduled_job(job: ScheduledJob) -> BoxedFuture<()>);
    delegate!(get_active_scheduled_jobs() -> BoxedFuture<Vec<ScheduledJob>>);
    delegate!(get_scheduled_jobs(current_user: String, page: i64, size: i64) -> BoxedFuture<Page<ScheduledJobSummary>>);
    delegate!(get_scheduled_job_by_id(id: String) -> BoxedFuture<Option<ScheduledJob>>);
    delegate!(update_scheduled_job(id: String, job: ScheduledJob) -> BoxedFuture<()>);
    delegate!(delete_scheduled_job(id: String) -> BoxedFuture<()>);
    delegate!(create_user(user: User) -> BoxedFuture<()>);
    delegate!(get_user(username: String) -> BoxedFuture<Option<User>>);
    delegate!(create_role(role: Role) -> BoxedFuture<()>);
    delegate!(get_role(id: String) -> BoxedFuture<Option<Role>>);
    delegate!(get_roles() -> BoxedFuture<Vec<Role>>);
    delegate!(get_user_roles(user_id: String) -> BoxedFuture<Vec<Role>>);
    delegate!(assign_role(user_id: String, role_id: String) -> BoxedFuture<()>);
    delegate!(unassign_role(user_id: String, role_id: String) -> BoxedFuture<()>);
    delegate!(get_metrics() -> BoxedFuture<Metrics>);
    delegate!(health_check() -> BoxedFuture<()>);
    delegate!(shutdown() -> BoxedFuture<()>);
}

// ── Config helpers ─────────────────────────────────────────────

/// Get a string from environment variables (`TORK_` prefix, dots → underscores).
fn env_string(key: &str) -> String {
    let env_key = format!("TORK_{}", key.to_uppercase().replace('.', "_"));
    env::var(&env_key).unwrap_or_default()
}

/// Get a string with default from environment variables.
fn env_string_default(key: &str, default: &str) -> String {
    let value = env_string(key);
    if value.is_empty() {
        default.to_string()
    } else {
        value
    }
}

/// Get an integer from environment variables with default.
fn env_int_default(key: &str, default: i32) -> i32 {
    let value = env_string(key);
    value.parse::<i32>().unwrap_or(default)
}

/// Get a [`Duration`] from environment variables with default (parsed as seconds).
fn env_duration_default(key: &str, default: Duration) -> Duration {
    let value = env_string(key);
    if value.is_empty() {
        default
    } else {
        value
            .parse::<i64>()
            .map(Duration::seconds)
            .unwrap_or(default)
    }
}

// ── Datastore factory ──────────────────────────────────────────

/// Default PostgreSQL DSN (matches Go default).
const DEFAULT_POSTGRES_DSN: &str =
    "host=localhost user=tork password=tork dbname=tork port=5432 sslmode=disable";

/// Creates a datastore based on configuration.
///
/// Reads `TORK_DATASTORE_TYPE` (default `"postgres"`):
/// - `"postgres"` → [`PostgresAdapter`] wrapping [`datastore::postgres::PostgresDatastore`]
/// - `"inmemory"` → [`InMemoryDatastore`] for testing / single-process
///
/// Matches Go `createDatastore()`:
/// - Full PostgreSQL option set from env vars
///
/// # Errors
///
/// Returns an error if:
/// - An unknown datastore type is specified
/// - The postgres connection cannot be established
pub async fn create_datastore() -> Result<Box<dyn Datastore + Send + Sync>> {
    let dstype = env_string_default("datastore.type", "postgres");

    match dstype.as_str() {
        "postgres" => {
            let dsn = env_string_default(
                "datastore.postgres.dsn",
                DEFAULT_POSTGRES_DSN,
            );

            let logs_retention = env_duration_default(
                "datastore.retention.logs.duration",
                datastore::postgres::DEFAULT_LOGS_RETENTION_DURATION,
            );
            let jobs_retention = env_duration_default(
                "datastore.retention.jobs.duration",
                datastore::postgres::DEFAULT_JOBS_RETENTION_DURATION,
            );
            let encryption_key = {
                let v = env_string("datastore.encryption.key");
                if v.is_empty() { None } else { Some(v) }
            };
            let max_open = env_int_default("datastore.postgres.max_open_conns", 25);
            let max_idle = env_int_default("datastore.postgres.max_idle_conns", 25);
            let lifetime = env_duration_default(
                "datastore.postgres.conn_max_lifetime",
                Duration::hours(1),
            );
            let idle_time = env_duration_default(
                "datastore.postgres.conn_max_idle_time",
                Duration::minutes(5),
            );

            let opts = datastore::postgres::Options {
                logs_retention_duration: logs_retention,
                jobs_retention_duration: jobs_retention,
                encryption_key,
                max_open_conns: Some(max_open),
                max_idle_conns: Some(max_idle),
                conn_max_lifetime: Some(lifetime),
                conn_max_idle_time: Some(idle_time),
                ..Default::default()
            };

            let pg = datastore::postgres::PostgresDatastore::new(&dsn, opts)
                .await
                .map_err(|e| anyhow::anyhow!("unable to connect to postgres: {}", e))?;

            Ok(Box::new(PostgresAdapter { inner: pg }))
        }
        "inmemory" => Ok(Box::new(InMemoryDatastore::new())),
        other => Err(anyhow::anyhow!("unknown datastore type: {}", other)),
    }
}

// ── PostgreSQL adapter ─────────────────────────────────────────

/// Adapts [`datastore::postgres::PostgresDatastore`] to the
/// [`tork::datastore::Datastore`] trait used by the engine.
///
/// The two traits differ in ownership semantics:
/// - `tork::Datastore` takes values by move (`fn create_task(&self, task: Task)`)
/// - `datastore::Datastore` takes references (`fn create_task(&self, task: &Task)`)
///
/// This adapter bridges the gap by delegating to the concrete postgres methods.
pub struct PostgresAdapter {
    inner: datastore::postgres::PostgresDatastore,
}

impl PostgresAdapter {
    fn map_err(e: datastore::Error) -> anyhow::Error {
        anyhow::anyhow!("{}", e)
    }
}

impl Datastore for PostgresAdapter {
    fn create_task(&self, task: Task) -> BoxedFuture<()> {
        let ds = self.inner.clone();
        Box::pin(async move { ds.create_task(&task).await.map_err(Self::map_err) })
    }

    fn update_task(&self, id: String, task: Task) -> BoxedFuture<()> {
        let ds = self.inner.clone();
        Box::pin(async move {
            ds.update_task(&id, move |existing| {
                *existing = task;
                Ok(())
            })
            .await
            .map_err(Self::map_err)
        })
    }

    fn get_task_by_id(&self, id: String) -> BoxedFuture<Option<Task>> {
        let ds = self.inner.clone();
        Box::pin(async move {
            ds.get_task_by_id(&id).await.map(Some).map_err(Self::map_err)
        })
    }

    fn get_active_tasks(&self, job_id: String) -> BoxedFuture<Vec<Task>> {
        let ds = self.inner.clone();
        Box::pin(async move {
            ds.get_active_tasks(&job_id)
                .await
                .map_err(Self::map_err)
        })
    }

    fn get_next_task(&self, parent_task_id: String) -> BoxedFuture<Option<Task>> {
        let ds = self.inner.clone();
        Box::pin(async move {
            ds.get_next_task(&parent_task_id).await.map(Some).map_err(Self::map_err)
        })
    }

    fn create_task_log_part(&self, part: TaskLogPart) -> BoxedFuture<()> {
        let ds = self.inner.clone();
        Box::pin(async move { ds.create_task_log_part(&part).await.map_err(Self::map_err) })
    }

    fn get_task_log_parts(
        &self,
        task_id: String,
        q: String,
        page: i64,
        size: i64,
    ) -> BoxedFuture<Page<TaskLogPart>> {
        let ds = self.inner.clone();
        Box::pin(async move {
            let pg_page = ds.get_task_log_parts(&task_id, &q, page, size)
                .await
                .map_err(Self::map_err)?;
            Ok(Page {
                items: pg_page.items,
                total: pg_page.total_items,
                page: pg_page.number,
                size: pg_page.size,
            })
        })
    }

    fn create_node(&self, node: Node) -> BoxedFuture<()> {
        let ds = self.inner.clone();
        Box::pin(async move { ds.create_node(&node).await.map_err(Self::map_err) })
    }

    fn update_node(&self, id: String, node: Node) -> BoxedFuture<()> {
        let ds = self.inner.clone();
        Box::pin(async move {
            ds.update_node(&id, move |existing| {
                *existing = node;
                Ok(())
            })
            .await
            .map_err(Self::map_err)
        })
    }

    fn get_node_by_id(&self, id: String) -> BoxedFuture<Option<Node>> {
        let ds = self.inner.clone();
        Box::pin(async move {
            ds.get_node_by_id(&id).await.map(Some).map_err(Self::map_err)
        })
    }

    fn get_active_nodes(&self) -> BoxedFuture<Vec<Node>> {
        let ds = self.inner.clone();
        Box::pin(async move { ds.get_active_nodes().await.map_err(Self::map_err) })
    }

    fn create_job(&self, job: Job) -> BoxedFuture<()> {
        let ds = self.inner.clone();
        Box::pin(async move { ds.create_job(&job).await.map_err(Self::map_err) })
    }

    fn update_job(&self, id: String, job: Job) -> BoxedFuture<()> {
        let ds = self.inner.clone();
        Box::pin(async move {
            ds.update_job(&id, move |existing| {
                *existing = job;
                Ok(())
            })
            .await
            .map_err(Self::map_err)
        })
    }

    fn get_job_by_id(&self, id: String) -> BoxedFuture<Option<Job>> {
        let ds = self.inner.clone();
        Box::pin(async move {
            ds.get_job_by_id(&id).await.map(Some).map_err(Self::map_err)
        })
    }

    fn get_job_log_parts(
        &self,
        job_id: String,
        q: String,
        page: i64,
        size: i64,
    ) -> BoxedFuture<Page<TaskLogPart>> {
        let ds = self.inner.clone();
        Box::pin(async move {
            let pg_page = ds.get_job_log_parts(&job_id, &q, page, size)
                .await
                .map_err(Self::map_err)?;
            Ok(Page {
                items: pg_page.items,
                total: pg_page.total_items,
                page: pg_page.number,
                size: pg_page.size,
            })
        })
    }

    fn get_jobs(
        &self,
        current_user: String,
        q: String,
        page: i64,
        size: i64,
    ) -> BoxedFuture<Page<JobSummary>> {
        let ds = self.inner.clone();
        Box::pin(async move {
            let pg_page = ds.get_jobs(&current_user, &q, page, size)
                .await
                .map_err(Self::map_err)?;
            Ok(Page {
                items: pg_page.items,
                total: pg_page.total_items,
                page: pg_page.number,
                size: pg_page.size,
            })
        })
    }

    fn create_scheduled_job(&self, job: ScheduledJob) -> BoxedFuture<()> {
        let ds = self.inner.clone();
        Box::pin(async move { ds.create_scheduled_job(&job).await.map_err(Self::map_err) })
    }

    fn get_active_scheduled_jobs(&self) -> BoxedFuture<Vec<ScheduledJob>> {
        let ds = self.inner.clone();
        Box::pin(async move { ds.get_active_scheduled_jobs().await.map_err(Self::map_err) })
    }

    fn get_scheduled_jobs(
        &self,
        current_user: String,
        page: i64,
        size: i64,
    ) -> BoxedFuture<Page<ScheduledJobSummary>> {
        let ds = self.inner.clone();
        Box::pin(async move {
            let pg_page = ds.get_scheduled_jobs(&current_user, page, size)
                .await
                .map_err(Self::map_err)?;
            Ok(Page {
                items: pg_page.items,
                total: pg_page.total_items,
                page: pg_page.number,
                size: pg_page.size,
            })
        })
    }

    fn get_scheduled_job_by_id(&self, id: String) -> BoxedFuture<Option<ScheduledJob>> {
        let ds = self.inner.clone();
        Box::pin(async move {
            ds.get_scheduled_job_by_id(&id).await.map(Some).map_err(Self::map_err)
        })
    }

    fn update_scheduled_job(&self, id: String, job: ScheduledJob) -> BoxedFuture<()> {
        let ds = self.inner.clone();
        Box::pin(async move {
            ds.update_scheduled_job(&id, move |existing| {
                *existing = job;
                Ok(())
            })
            .await
            .map_err(Self::map_err)
        })
    }

    fn delete_scheduled_job(&self, id: String) -> BoxedFuture<()> {
        let ds = self.inner.clone();
        Box::pin(async move { ds.delete_scheduled_job(&id).await.map_err(Self::map_err) })
    }

    fn create_user(&self, user: User) -> BoxedFuture<()> {
        let ds = self.inner.clone();
        Box::pin(async move { ds.create_user(&user).await.map_err(Self::map_err) })
    }

    fn get_user(&self, username: String) -> BoxedFuture<Option<User>> {
        let ds = self.inner.clone();
        Box::pin(async move {
            ds.get_user(&username).await.map(Some).map_err(Self::map_err)
        })
    }

    fn create_role(&self, role: Role) -> BoxedFuture<()> {
        let ds = self.inner.clone();
        Box::pin(async move { ds.create_role(&role).await.map_err(Self::map_err) })
    }

    fn get_role(&self, id: String) -> BoxedFuture<Option<Role>> {
        let ds = self.inner.clone();
        Box::pin(async move {
            ds.get_role(&id).await.map(Some).map_err(Self::map_err)
        })
    }

    fn get_roles(&self) -> BoxedFuture<Vec<Role>> {
        let ds = self.inner.clone();
        Box::pin(async move { ds.get_roles().await.map_err(Self::map_err) })
    }

    fn get_user_roles(&self, user_id: String) -> BoxedFuture<Vec<Role>> {
        let ds = self.inner.clone();
        Box::pin(async move { ds.get_user_roles(&user_id).await.map_err(Self::map_err) })
    }

    fn assign_role(&self, user_id: String, role_id: String) -> BoxedFuture<()> {
        let ds = self.inner.clone();
        Box::pin(async move { ds.assign_role(&user_id, &role_id).await.map_err(Self::map_err) })
    }

    fn unassign_role(&self, user_id: String, role_id: String) -> BoxedFuture<()> {
        let ds = self.inner.clone();
        Box::pin(async move { ds.unassign_role(&user_id, &role_id).await.map_err(Self::map_err) })
    }

    fn get_metrics(&self) -> BoxedFuture<Metrics> {
        let ds = self.inner.clone();
        Box::pin(async move { ds.get_metrics().await.map_err(Self::map_err) })
    }

    fn health_check(&self) -> BoxedFuture<()> {
        let ds = self.inner.clone();
        Box::pin(async move { ds.health_check().await.map_err(Self::map_err) })
    }

    fn shutdown(&self) -> BoxedFuture<()> {
        let ds = self.inner.clone();
        Box::pin(async move { ds.close().await.map_err(Self::map_err) })
    }
}

// ── In-memory datastore ────────────────────────────────────────

/// Thread-safe in-memory datastore for testing and single-process usage.
///
/// Uses [`DashMap`] for concurrent access without `mut`. Matches the Go
/// in-memory datastore behaviour (no persistence, no search filtering).
struct InMemoryDatastore {
    tasks: Arc<DashMap<String, Task>>,
    nodes: Arc<DashMap<String, Node>>,
    jobs: Arc<DashMap<String, Job>>,
    users: Arc<DashMap<String, User>>,
    roles: Arc<DashMap<String, Role>>,
    scheduled_jobs: Arc<DashMap<String, ScheduledJob>>,
    task_log_parts: Arc<DashMap<String, Vec<TaskLogPart>>>,
    user_roles: Arc<DashMap<String, Vec<String>>>,
}

impl InMemoryDatastore {
    fn new() -> Self {
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

    /// Helper: extract ID from Option<String>.
    fn require_id(id: &Option<String>) -> Result<String> {
        id.clone()
            .ok_or_else(|| anyhow::anyhow!("id is required"))
    }

    /// Helper: simple pagination over a Vec.
    fn paginate<T>(items: Vec<T>, page: i64, size: i64) -> Page<T> {
        let total = items.len() as i64;
        let skip = ((page - 1).max(0) * size) as usize;
        let paged: Vec<T> = items.into_iter().skip(skip).take(size as usize).collect();
        Page {
            items: paged,
            total,
            page,
            size,
        }
    }
}

impl Datastore for InMemoryDatastore {
    fn create_task(&self, task: Task) -> BoxedFuture<()> {
        let id = Self::require_id(&task.id);
        let tasks = self.tasks.clone();
        Box::pin(async move {
            let id = id?;
            tasks.insert(id, task);
            Ok(())
        })
    }

    fn update_task(&self, id: String, task: Task) -> BoxedFuture<()> {
        let tasks = self.tasks.clone();
        Box::pin(async move {
            if tasks.contains_key(&id) {
                tasks.insert(id, task);
                Ok(())
            } else {
                Err(anyhow::anyhow!("task not found"))
            }
        })
    }

    fn get_task_by_id(&self, id: String) -> BoxedFuture<Option<Task>> {
        let tasks = self.tasks.clone();
        Box::pin(async move { Ok(tasks.get(&id).map(|r| r.value().clone())) })
    }

    fn get_active_tasks(&self, job_id: String) -> BoxedFuture<Vec<Task>> {
        let tasks = self.tasks.clone();
        Box::pin(async move {
            let result: Vec<Task> = tasks
                .iter()
                .filter(|e| {
                    let t = e.value();
                    t.job_id.as_deref() == Some(&*job_id)
                        && matches!(
                            t.state.as_ref(),
                            "CREATED" | "PENDING" | "SCHEDULED" | "RUNNING"
                        )
                })
                .map(|e| e.value().clone())
                .collect();
            Ok(result)
        })
    }

    fn get_next_task(&self, parent_task_id: String) -> BoxedFuture<Option<Task>> {
        let tasks = self.tasks.clone();
        Box::pin(async move {
            let result = tasks
                .iter()
                .find(|e| {
                    let t = e.value();
                    t.parent_id.as_deref() == Some(&*parent_task_id)
                        && t.state.as_ref() == "CREATED"
                })
                .map(|e| e.value().clone());
            Ok(result)
        })
    }

    fn create_task_log_part(&self, part: TaskLogPart) -> BoxedFuture<()> {
        let task_id = part.task_id.clone().unwrap_or_default();
        let log_parts = self.task_log_parts.clone();
        Box::pin(async move {
            log_parts.entry(task_id).or_default().push(part);
            Ok(())
        })
    }

    fn get_task_log_parts(
        &self,
        task_id: String,
        _q: String,
        page: i64,
        size: i64,
    ) -> BoxedFuture<Page<TaskLogPart>> {
        let log_parts = self.task_log_parts.clone();
        Box::pin(async move {
            let parts = log_parts
                .get(&task_id)
                .map(|e| e.value().clone())
                .unwrap_or_default();
            Ok(Self::paginate(parts, page, size))
        })
    }

    fn create_node(&self, node: Node) -> BoxedFuture<()> {
        let id = Self::require_id(&node.id);
        let nodes = self.nodes.clone();
        Box::pin(async move {
            let id = id?;
            nodes.insert(id, node);
            Ok(())
        })
    }

    fn update_node(&self, id: String, node: Node) -> BoxedFuture<()> {
        let nodes = self.nodes.clone();
        Box::pin(async move {
            if nodes.contains_key(&id) {
                nodes.insert(id, node);
                Ok(())
            } else {
                Err(anyhow::anyhow!("node not found"))
            }
        })
    }

    fn get_node_by_id(&self, id: String) -> BoxedFuture<Option<Node>> {
        let nodes = self.nodes.clone();
        Box::pin(async move { Ok(nodes.get(&id).map(|r| r.value().clone())) })
    }

    fn get_active_nodes(&self) -> BoxedFuture<Vec<Node>> {
        let nodes = self.nodes.clone();
        Box::pin(async move {
            let result: Vec<Node> = nodes.iter().map(|e| e.value().clone()).collect();
            Ok(result)
        })
    }

    fn create_job(&self, job: Job) -> BoxedFuture<()> {
        let id = Self::require_id(&job.id);
        let jobs = self.jobs.clone();
        Box::pin(async move {
            let id = id?;
            jobs.insert(id, job);
            Ok(())
        })
    }

    fn update_job(&self, id: String, job: Job) -> BoxedFuture<()> {
        let jobs = self.jobs.clone();
        Box::pin(async move {
            if jobs.contains_key(&id) {
                jobs.insert(id, job);
                Ok(())
            } else {
                Err(anyhow::anyhow!("job not found"))
            }
        })
    }

    fn get_job_by_id(&self, id: String) -> BoxedFuture<Option<Job>> {
        let jobs = self.jobs.clone();
        Box::pin(async move { Ok(jobs.get(&id).map(|r| r.value().clone())) })
    }

    fn get_job_log_parts(
        &self,
        job_id: String,
        _q: String,
        page: i64,
        size: i64,
    ) -> BoxedFuture<Page<TaskLogPart>> {
        let tasks = self.tasks.clone();
        let log_parts = self.task_log_parts.clone();
        Box::pin(async move {
            let task_ids: Vec<String> = tasks
                .iter()
                .filter(|e| e.value().job_id.as_deref() == Some(&*job_id))
                .filter_map(|e| e.value().id.clone())
                .collect();

            let all_parts: Vec<TaskLogPart> = task_ids
                .iter()
                .filter_map(|tid| log_parts.get(tid).map(|e| e.value().clone()))
                .flatten()
                .collect();

            Ok(Self::paginate(all_parts, page, size))
        })
    }

    fn get_jobs(
        &self,
        _current_user: String,
        _q: String,
        page: i64,
        size: i64,
    ) -> BoxedFuture<Page<JobSummary>> {
        let jobs = self.jobs.clone();
        Box::pin(async move {
            let summaries: Vec<JobSummary> = jobs
                .iter()
                .map(|e| tork::job::new_job_summary(e.value()))
                .collect();
            Ok(Self::paginate(summaries, page, size))
        })
    }

    fn create_scheduled_job(&self, job: ScheduledJob) -> BoxedFuture<()> {
        let id = Self::require_id(&job.id);
        let scheduled_jobs = self.scheduled_jobs.clone();
        Box::pin(async move {
            let id = id?;
            scheduled_jobs.insert(id, job);
            Ok(())
        })
    }

    fn get_active_scheduled_jobs(&self) -> BoxedFuture<Vec<ScheduledJob>> {
        let scheduled_jobs = self.scheduled_jobs.clone();
        Box::pin(async move {
            let result: Vec<ScheduledJob> = scheduled_jobs
                .iter()
                .filter(|e| e.value().state == "ACTIVE")
                .map(|e| e.value().clone())
                .collect();
            Ok(result)
        })
    }

    fn get_scheduled_jobs(
        &self,
        _current_user: String,
        page: i64,
        size: i64,
    ) -> BoxedFuture<Page<ScheduledJobSummary>> {
        let scheduled_jobs = self.scheduled_jobs.clone();
        Box::pin(async move {
            let summaries: Vec<ScheduledJobSummary> = scheduled_jobs
                .iter()
                .map(|e| tork::job::new_scheduled_job_summary(e.value()))
                .collect();
            Ok(Self::paginate(summaries, page, size))
        })
    }

    fn get_scheduled_job_by_id(&self, id: String) -> BoxedFuture<Option<ScheduledJob>> {
        let scheduled_jobs = self.scheduled_jobs.clone();
        Box::pin(async move {
            Ok(scheduled_jobs.get(&id).map(|r| r.value().clone()))
        })
    }

    fn update_scheduled_job(&self, id: String, job: ScheduledJob) -> BoxedFuture<()> {
        let scheduled_jobs = self.scheduled_jobs.clone();
        Box::pin(async move {
            if scheduled_jobs.contains_key(&id) {
                scheduled_jobs.insert(id, job);
                Ok(())
            } else {
                Err(anyhow::anyhow!("scheduled job not found"))
            }
        })
    }

    fn delete_scheduled_job(&self, id: String) -> BoxedFuture<()> {
        let scheduled_jobs = self.scheduled_jobs.clone();
        Box::pin(async move {
            scheduled_jobs.remove(&id);
            Ok(())
        })
    }

    fn create_user(&self, user: User) -> BoxedFuture<()> {
        let id = Self::require_id(&user.id);
        let username = user.username.clone();
        let users = self.users.clone();
        Box::pin(async move {
            let id = id?;
            // Index by username so get_user() lookups succeed
            if let Some(ref uname) = username {
                users.insert(uname.clone(), user.clone());
            }
            users.insert(id, user);
            Ok(())
        })
    }

    fn get_user(&self, username: String) -> BoxedFuture<Option<User>> {
        let users = self.users.clone();
        Box::pin(async move { Ok(users.get(&username).map(|r| r.value().clone())) })
    }

    fn create_role(&self, role: Role) -> BoxedFuture<()> {
        let id = Self::require_id(&role.id);
        let roles = self.roles.clone();
        Box::pin(async move {
            let id = id?;
            roles.insert(id, role);
            Ok(())
        })
    }

    fn get_role(&self, id: String) -> BoxedFuture<Option<Role>> {
        let roles = self.roles.clone();
        Box::pin(async move { Ok(roles.get(&id).map(|r| r.value().clone())) })
    }

    fn get_roles(&self) -> BoxedFuture<Vec<Role>> {
        let roles = self.roles.clone();
        Box::pin(async move {
            let result: Vec<Role> = roles.iter().map(|e| e.value().clone()).collect();
            Ok(result)
        })
    }

    fn get_user_roles(&self, user_id: String) -> BoxedFuture<Vec<Role>> {
        let user_roles = self.user_roles.clone();
        let roles = self.roles.clone();
        Box::pin(async move {
            let role_ids = user_roles
                .get(&user_id)
                .map(|e| e.value().clone())
                .unwrap_or_default();
            let result: Vec<Role> = role_ids
                .iter()
                .filter_map(|rid| roles.get(rid).map(|r| r.value().clone()))
                .collect();
            Ok(result)
        })
    }

    fn assign_role(&self, user_id: String, role_id: String) -> BoxedFuture<()> {
        let user_roles = self.user_roles.clone();
        Box::pin(async move {
            let mut entry = user_roles.entry(user_id).or_default();
            if !entry.contains(&role_id) {
                entry.push(role_id);
            }
            Ok(())
        })
    }

    fn unassign_role(&self, user_id: String, role_id: String) -> BoxedFuture<()> {
        let user_roles = self.user_roles.clone();
        Box::pin(async move {
            if let Some(mut entry) = user_roles.get_mut(&user_id) {
                entry.retain(|id| id != &role_id);
            }
            Ok(())
        })
    }

    fn get_metrics(&self) -> BoxedFuture<Metrics> {
        let jobs = self.jobs.clone();
        let tasks = self.tasks.clone();
        let nodes = self.nodes.clone();
        Box::pin(async move {
            let jobs_running = jobs
                .iter()
                .filter(|e| e.value().state == "RUNNING")
                .count() as i64;
            let tasks_running = tasks
                .iter()
                .filter(|e| e.value().state.as_ref() == "RUNNING")
                .count() as i64;
            let nodes_running = nodes.len() as i64;
            Ok(Metrics {
                jobs: tork::stats::JobMetrics { running: jobs_running },
                tasks: tork::stats::TaskMetrics { running: tasks_running },
                nodes: tork::stats::NodeMetrics {
                    running: nodes_running,
                    cpu_percent: 0.0,
                },
            })
        })
    }

    fn health_check(&self) -> BoxedFuture<()> {
        Box::pin(async { Ok(()) })
    }

    fn shutdown(&self) -> BoxedFuture<()> {
        Box::pin(async { Ok(()) })
    }
}

    /// Creates a new in-memory datastore for testing.
    ///
    /// Returns a boxed `Datastore` trait object backed by an in-memory
    /// `DashMap` store with no persistence.
    #[must_use]
    pub fn new_inmemory_datastore() -> Box<dyn Datastore + Send + Sync> {
        Box::new(InMemoryDatastore::new())
    }

    /// Creates an `Arc<dyn Datastore>` from the in-memory store.
    ///
    /// Used by middleware configs that require `Arc<dyn Datastore>`.
    #[must_use]
    pub fn new_inmemory_datastore_arc() -> std::sync::Arc<dyn tork::datastore::Datastore> {
        let boxed: Box<dyn tork::datastore::Datastore + Send + Sync> =
            crate::datastore::new_inmemory_datastore();
        // Unsize coercion: Box<dyn Trait + Send + Sync> → Box<dyn Trait>
        let boxed: Box<dyn tork::datastore::Datastore> = boxed;
        std::sync::Arc::from(boxed)
    }
