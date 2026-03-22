//! Datastore trait and error types for data persistence.
//!
//! This module defines the `Datastore` trait that abstracts over different
//! storage implementations (postgres, etc.).

#![deny(clippy::unwrap_used)]
#![deny(clippy::expect_used)]
#![deny(clippy::panic)]
#![warn(clippy::pedantic)]
#![forbid(unsafe_code)]

pub mod postgres;

use std::error::Error as StdError;
use tork::{
    job::{Job, ScheduledJob},
    task::{Task, TaskLogPart},
    user::User,
    role::Role,
    Node,
};
use thiserror::Error;

/// Result type for datastore operations
pub type Result<T> = std::result::Result<T, Error>;

/// Errors that can occur during datastore operations
#[derive(Debug, Error)]
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

impl PartialEq for Error {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Error::TaskNotFound, Error::TaskNotFound) => true,
            (Error::NodeNotFound, Error::NodeNotFound) => true,
            (Error::JobNotFound, Error::JobNotFound) => true,
            (Error::ScheduledJobNotFound, Error::ScheduledJobNotFound) => true,
            (Error::UserNotFound, Error::UserNotFound) => true,
            (Error::RoleNotFound, Error::RoleNotFound) => true,
            (Error::ContextNotFound, Error::ContextNotFound) => true,
            _ => false,
        }
    }
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

/// Datastore is the interface for data persistence operations.
///
/// All methods take a context and return Result for proper error handling.
/// Implementations should not panic but return appropriate errors.
pub trait Datastore: Send + Sync {
    // Task operations
    fn create_task(&self, task: &Task) -> impl StdError + Send + Sync + '_;
    fn update_task(&self, id: &str, modify: impl Fn(&mut Task) -> Result<()>) -> impl StdError + Send + Sync + '_;
    fn get_task_by_id(&self, id: &str) -> impl StdError + Send + Sync + '_;
    fn get_active_tasks(&self, job_id: &str) -> impl StdError + Send + Sync + '_;
    fn get_next_task(&self, parent_task_id: &str) -> impl StdError + Send + Sync + '_;
    fn create_task_log_part(&self, part: &TaskLogPart) -> impl StdError + Send + Sync + '_;
    fn get_task_log_parts(&self, task_id: &str, q: &str, page: i64, size: i64) -> impl StdError + Send + Sync + '_;

    // Node operations
    fn create_node(&self, node: &Node) -> impl StdError + Send + Sync + '_;
    fn update_node(&self, id: &str, modify: impl Fn(&mut Node) -> Result<()>) -> impl StdError + Send + Sync + '_;
    fn get_node_by_id(&self, id: &str) -> impl StdError + Send + Sync + '_;
    fn get_active_nodes(&self) -> impl StdError + Send + Sync + '_;

    // Job operations
    fn create_job(&self, job: &Job) -> impl StdError + Send + Sync + '_;
    fn update_job(&self, id: &str, modify: impl Fn(&mut Job) -> Result<()>) -> impl StdError + Send + Sync + '_;
    fn get_job_by_id(&self, id: &str) -> impl StdError + Send + Sync + '_;
    fn get_job_log_parts(&self, job_id: &str, q: &str, page: i64, size: i64) -> impl StdError + Send + Sync + '_;
    fn get_jobs(&self, current_user: &str, q: &str, page: i64, size: i64) -> impl StdError + Send + Sync + '_;

    // Scheduled job operations
    fn create_scheduled_job(&self, sj: &ScheduledJob) -> impl StdError + Send + Sync + '_;
    fn get_active_scheduled_jobs(&self) -> impl StdError + Send + Sync + '_;
    fn get_scheduled_jobs(&self, current_user: &str, page: i64, size: i64) -> impl StdError + Send + Sync + '_;
    fn get_scheduled_job_by_id(&self, id: &str) -> impl StdError + Send + Sync + '_;
    fn update_scheduled_job(&self, id: &str, modify: impl Fn(&mut ScheduledJob) -> Result<()>) -> impl StdError + Send + Sync + '_;
    fn delete_scheduled_job(&self, id: &str) -> impl StdError + Send + Sync + '_;

    // User and role operations
    fn create_user(&self, user: &User) -> impl StdError + Send + Sync + '_;
    fn get_user(&self, username: &str) -> impl StdError + Send + Sync + '_;

    fn create_role(&self, role: &Role) -> impl StdError + Send + Sync + '_;
    fn get_role(&self, id: &str) -> impl StdError + Send + Sync + '_;
    fn get_roles(&self) -> impl StdError + Send + Sync + '_;
    fn get_user_roles(&self, user_id: &str) -> impl StdError + Send + Sync + '_;
    fn assign_role(&self, user_id: &str, role_id: &str) -> impl StdError + Send + Sync + '_;
    fn unassign_role(&self, user_id: &str, role_id: &str) -> impl StdError + Send + Sync + '_;

    // Metrics
    fn get_metrics(&self) -> impl StdError + Send + Sync + '_;

    // Health check
    fn health_check(&self) -> impl StdError + Send + Sync + '_;
}
