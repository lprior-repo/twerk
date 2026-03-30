//! Repository trait and error types for data persistence.

use async_trait::async_trait;
use thiserror::Error;
use time::Duration;

use crate::{
    job::{Job, JobSummary, ScheduledJob, ScheduledJobSummary},
    node::Node,
    role::Role,
    task::{Task, TaskLogPart},
    user::User,
};

/// Result type for repository operations
pub type Result<T> = std::result::Result<T, Error>;

/// Errors that can occur during repository operations
#[derive(Debug, Error, PartialEq)]
pub enum Error {
    #[error("task not found")]
    TaskNotFound,

    #[error("node not found")]
    NodeNotFound,

    #[error("job not found")]
    JobNotFound,

    #[error("scheduled job not found")]
    ScheduledJobNotFound,

    #[error("user not found")]
    UserNotFound,

    #[error("role not found")]
    RoleNotFound,

    #[error("context not found")]
    ContextNotFound,

    #[error("database error: {0}")]
    Database(String),

    #[error("serialization error: {0}")]
    Serialization(String),

    #[error("encryption error: {0}")]
    Encryption(String),

    #[error("invalid input: {0}")]
    InvalidInput(String),

    #[error("transaction error: {0}")]
    Transaction(String),
}

/// Configuration options for Repository
#[derive(Clone, Default)]
pub struct Options {
    pub logs_retention_duration: Duration,
    pub jobs_retention_duration: Duration,
    pub cleanup_interval: Duration,
    pub disable_cleanup: bool,
    pub encryption_key: Option<String>,
    pub max_open_conns: Option<i32>,
    pub max_idle_conns: Option<i32>,
    pub conn_max_lifetime: Option<Duration>,
    pub conn_max_idle_time: Option<Duration>,
}

/// Page represents a paginated result set
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Page<T> {
    /// Items in the current page
    pub items: Vec<T>,
    /// Current page number
    pub number: i64,
    /// Page size
    pub size: i64,
    /// Total number of pages
    pub total_pages: i64,
    /// Total number of items
    pub total_items: i64,
}

/// Repository is the interface for data persistence operations.
#[async_trait]
pub trait Repository: Send + Sync {
    // Task operations
    async fn create_task(&self, task: &Task) -> Result<()>;
    async fn update_task(
        &self,
        id: &str,
        modify: Box<dyn FnOnce(Task) -> Result<Task> + Send>,
    ) -> Result<()>;
    async fn get_task_by_id(&self, id: &str) -> Result<Task>;
    async fn get_active_tasks(&self, job_id: &str) -> Result<Vec<Task>>;
    async fn get_next_task(&self, parent_task_id: &str) -> Result<Task>;
    async fn create_task_log_part(&self, part: &TaskLogPart) -> Result<()>;
    async fn get_task_log_parts(
        &self,
        task_id: &str,
        q: &str,
        page: i64,
        size: i64,
    ) -> Result<Page<TaskLogPart>>;

    // Node operations
    async fn create_node(&self, node: &Node) -> Result<()>;
    async fn update_node(
        &self,
        id: &str,
        modify: Box<dyn FnOnce(Node) -> Result<Node> + Send>,
    ) -> Result<()>;
    async fn get_node_by_id(&self, id: &str) -> Result<Node>;
    async fn get_active_nodes(&self) -> Result<Vec<Node>>;

    // Job operations
    async fn create_job(&self, job: &Job) -> Result<()>;
    async fn update_job(
        &self,
        id: &str,
        modify: Box<dyn FnOnce(Job) -> Result<Job> + Send>,
    ) -> Result<()>;
    async fn get_job_by_id(&self, id: &str) -> Result<Job>;
    async fn get_job_log_parts(
        &self,
        job_id: &str,
        q: &str,
        page: i64,
        size: i64,
    ) -> Result<Page<TaskLogPart>>;
    async fn get_jobs(
        &self,
        current_user: &str,
        q: &str,
        page: i64,
        size: i64,
    ) -> Result<Page<JobSummary>>;

    // Scheduled job operations
    async fn create_scheduled_job(&self, sj: &ScheduledJob) -> Result<()>;
    async fn get_active_scheduled_jobs(&self) -> Result<Vec<ScheduledJob>>;
    async fn get_scheduled_jobs(
        &self,
        current_user: &str,
        page: i64,
        size: i64,
    ) -> Result<Page<ScheduledJobSummary>>;
    async fn get_scheduled_job_by_id(&self, id: &str) -> Result<ScheduledJob>;
    async fn update_scheduled_job(
        &self,
        id: &str,
        modify: Box<dyn FnOnce(ScheduledJob) -> Result<ScheduledJob> + Send>,
    ) -> Result<()>;
    async fn delete_scheduled_job(&self, id: &str) -> Result<()>;

    // User and role operations
    async fn create_user(&self, user: &User) -> Result<()>;
    async fn get_user(&self, username: &str) -> Result<User>;

    async fn create_role(&self, role: &Role) -> Result<()>;
    async fn get_role(&self, id: &str) -> Result<Role>;
    async fn get_roles(&self) -> Result<Vec<Role>>;
    async fn get_user_roles(&self, user_id: &str) -> Result<Vec<Role>>;
    async fn assign_role(&self, user_id: &str, role_id: &str) -> Result<()>;
    async fn unassign_role(&self, user_id: &str, role_id: &str) -> Result<()>;

    // Metrics
    async fn get_metrics(&self) -> Result<crate::stats::Metrics>;

    async fn with_tx(
        &self,
        f: Box<
            dyn for<'a> FnOnce(
                    &'a dyn Repository,
                ) -> futures_util::future::BoxFuture<'a, Result<()>>
                + Send,
        >,
    ) -> Result<()>;

    // Health check
    async fn health_check(&self) -> Result<()>;
}
