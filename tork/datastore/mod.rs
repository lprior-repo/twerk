//! Datastore module for persistent storage
//!
//! This module provides the datastore interface for persisting
//! tasks, jobs, nodes, users, and other entities.

use crate::job::{Job, JobSummary, ScheduledJob, ScheduledJobSummary};
use crate::node::Node;
use crate::role::Role;
use crate::stats::Metrics;
use crate::task::{Task, TaskLogPart};
use crate::user::User;
use std::pin::Pin;

/// Boxed future type for datastore operations
pub type BoxedFuture<T> =
    Pin<Box<dyn std::future::Future<Output = Result<T, anyhow::Error>> + Send>>;

/// Page represents a paginated result set
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Page<T> {
    /// Items in the current page
    pub items: Vec<T>,
    /// Total number of items
    pub total: i64,
    /// Current page number
    pub page: i64,
    /// Page size
    pub size: i64,
}

/// Datastore is the persistent storage interface
pub trait Datastore: Send + Sync {
    // Task operations
    /// Creates a new task
    fn create_task(&self, task: Task) -> BoxedFuture<()>;
    /// Updates an existing task
    fn update_task(&self, id: String, task: Task) -> BoxedFuture<()>;
    /// Gets a task by ID
    fn get_task_by_id(&self, id: String) -> BoxedFuture<Option<Task>>;
    /// Gets all active tasks for a job
    fn get_active_tasks(&self, job_id: String) -> BoxedFuture<Vec<Task>>;
    /// Gets the next pending task
    fn get_next_task(&self, parent_task_id: String) -> BoxedFuture<Option<Task>>;

    // Task log operations
    /// Creates a task log part
    fn create_task_log_part(&self, part: TaskLogPart) -> BoxedFuture<()>;
    /// Gets task log parts with pagination
    fn get_task_log_parts(
        &self,
        task_id: String,
        q: String,
        page: i64,
        size: i64,
    ) -> BoxedFuture<Page<TaskLogPart>>;

    // Node operations
    /// Creates a new node
    fn create_node(&self, node: Node) -> BoxedFuture<()>;
    /// Updates an existing node
    fn update_node(&self, id: String, node: Node) -> BoxedFuture<()>;
    /// Gets a node by ID
    fn get_node_by_id(&self, id: String) -> BoxedFuture<Option<Node>>;
    /// Gets all active nodes
    fn get_active_nodes(&self) -> BoxedFuture<Vec<Node>>;

    // Job operations
    /// Creates a new job
    fn create_job(&self, job: Job) -> BoxedFuture<()>;
    /// Updates an existing job
    fn update_job(&self, id: String, job: Job) -> BoxedFuture<()>;
    /// Gets a job by ID
    fn get_job_by_id(&self, id: String) -> BoxedFuture<Option<Job>>;
    /// Gets job log parts with pagination
    fn get_job_log_parts(
        &self,
        job_id: String,
        q: String,
        page: i64,
        size: i64,
    ) -> BoxedFuture<Page<TaskLogPart>>;
    /// Gets jobs with pagination
    fn get_jobs(
        &self,
        current_user: String,
        q: String,
        page: i64,
        size: i64,
    ) -> BoxedFuture<Page<JobSummary>>;

    // Scheduled job operations
    /// Creates a scheduled job
    fn create_scheduled_job(&self, job: ScheduledJob) -> BoxedFuture<()>;
    /// Gets all active scheduled jobs
    fn get_active_scheduled_jobs(&self) -> BoxedFuture<Vec<ScheduledJob>>;
    /// Gets scheduled jobs with pagination
    fn get_scheduled_jobs(
        &self,
        current_user: String,
        page: i64,
        size: i64,
    ) -> BoxedFuture<Page<ScheduledJobSummary>>;
    /// Gets a scheduled job by ID
    fn get_scheduled_job_by_id(&self, id: String) -> BoxedFuture<Option<ScheduledJob>>;
    /// Updates a scheduled job
    fn update_scheduled_job(&self, id: String, job: ScheduledJob) -> BoxedFuture<()>;
    /// Deletes a scheduled job
    fn delete_scheduled_job(&self, id: String) -> BoxedFuture<()>;

    // User operations
    /// Creates a user
    fn create_user(&self, user: User) -> BoxedFuture<()>;
    /// Gets a user by username
    fn get_user(&self, username: String) -> BoxedFuture<Option<User>>;

    // Role operations
    /// Creates a role
    fn create_role(&self, role: Role) -> BoxedFuture<()>;
    /// Gets a role by ID
    fn get_role(&self, id: String) -> BoxedFuture<Option<Role>>;
    /// Gets all roles
    fn get_roles(&self) -> BoxedFuture<Vec<Role>>;

    /// Gets roles assigned to a user
    fn get_user_roles(&self, user_id: String) -> BoxedFuture<Vec<Role>>;

    /// Assigns a role to a user
    fn assign_role(&self, user_id: String, role_id: String) -> BoxedFuture<()>;

    /// Unassigns a role from a user
    fn unassign_role(&self, user_id: String, role_id: String) -> BoxedFuture<()>;

    // Metrics
    /// Gets system metrics
    fn get_metrics(&self) -> BoxedFuture<Metrics>;

    // Health operations
    /// Performs a health check
    fn health_check(&self) -> BoxedFuture<()>;
    /// Shuts down the datastore
    fn shutdown(&self) -> BoxedFuture<()>;
}
