//! Coordinator — the "brain" of the Tork task queue system.
//!
//! Accepts tasks from clients, schedules tasks for workers, and exposes
//! cluster state to the outside world.
//!
//! # Go Parity
//!
//! 100% parity with Go `internal/coordinator/coordinator.go`:
//! - [`Config`] — mirrors `coordinator.Config`
//! - [`Coordinator::new`] — mirrors `NewCoordinator(cfg)`
//! - [`Coordinator::start`] — mirrors `Start()`
//! - [`Coordinator::stop`] — mirrors `Stop()`
//! - [`Coordinator::submit_job`] — mirrors `SubmitJob()`
//! - [`send_heartbeats`] — mirrors `sendHeartbeats()`
//!
//! # Architecture
//!
//! - **Data**: [`Config`], [`Coordinator`] structs
//! - **Calc**: Pure validation in constructor, queue defaulting
//! - **Actions**: All broker/datastore/HTTP I/O at the shell boundary

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use time::OffsetDateTime;
use tokio::sync::{watch, Mutex};
use tracing::{debug, error, info, warn};

use tork::broker::{
    is_coordinator_queue, queue, Broker, EventHandler, HeartbeatHandler, JobHandler,
    TaskHandler, TaskLogPartHandler, TaskProgressHandler,
};
use tork::datastore::Datastore;
use tork::job::{Job, ScheduledJob, JOB_STATE_FAILED};
use tork::node::{Node, HEARTBEAT_RATE_SECS, NODE_STATUS_UP};
use tork::task::{Task, TASK_STATE_FAILED};
use tork::version::VERSION;

use crate::api;
use crate::handlers::{
    completed::CompletedHandler, error::ErrorHandler, heartbeat::HeartbeatHandler as NodeHeartbeatHandler,
    job::JobHandler as JobEventHandler, log::LogHandler, pending::PendingHandler,
    progress::ProgressHandler, redelivered::RedeliveredHandler, schedule::ScheduleHandler,
    started::StartedHandler, HandlerError,
};

// ---------------------------------------------------------------------------
// ID generation
// ---------------------------------------------------------------------------

/// Generate a unique coordinator identifier.
///
/// Go parity: `uuid.NewShortUUID()` — produces a 22-char base62 ID
/// without hyphens. We use standard UUID v4 with hyphens stripped.
#[must_use]
fn new_coordinator_id() -> String {
    uuid::Uuid::new_v4()
        .to_string()
        .replace('-', "")
}

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

/// Topic for scheduled job events (Go: `broker.TOPIC_SCHEDULED_JOB`).
const TOPIC_SCHEDULED_JOB: &str = "scheduled-job";

/// Shutdown timeout in seconds (Go: 15 seconds).
const SHUTDOWN_TIMEOUT_SECS: u64 = 15;

/// Default concurrency for coordinator queues that aren't explicitly configured.
const DEFAULT_QUEUE_CONCURRENCY: i64 = 1;

/// Coordinator queues that need default concurrency when not specified.
const COORDINATOR_QUEUES: &[&str] = &[
    queue::QUEUE_COMPLETED,
    queue::QUEUE_ERROR,
    queue::QUEUE_PENDING,
    queue::QUEUE_STARTED,
    queue::QUEUE_HEARTBEAT,
    queue::QUEUE_JOBS,
    queue::QUEUE_LOGS,
    queue::QUEUE_PROGRESS,
    queue::QUEUE_REDELIVERIES,
];

// ---------------------------------------------------------------------------
// Error types
// ---------------------------------------------------------------------------

/// Errors that can occur during coordinator operations.
#[derive(Debug, thiserror::Error)]
pub enum CoordinatorError {
    #[error("validation error: {0}")]
    Validation(String),

    #[error("broker error: {0}")]
    Broker(String),

    #[error("datastore error: {0}")]
    Datastore(String),

    #[error("API error: {0}")]
    Api(String),

    #[error("handler error: {0}")]
    Handler(String),
}

// ---------------------------------------------------------------------------
// Middleware
// ---------------------------------------------------------------------------

/// Broker-level task handler type.
///
/// This is the handler signature expected by the broker's `subscribe_for_tasks`.
type BrokerTaskHandler = TaskHandler;

/// Broker-level job handler type.
type BrokerJobHandler = JobHandler;

/// Broker-level node handler type.
type BrokerNodeHandler = HeartbeatHandler;

/// Broker-level log handler type.
type BrokerLogHandler = TaskLogPartHandler;

/// Middleware chains for handler types.
///
/// Go parity: `coordinator.Middleware`
///
/// Each field holds a vector of middleware functions that wrap the corresponding
/// broker handler. Middleware is applied in order using a left fold, so
/// `vec![mw1, mw2]` produces: mw1 → mw2 → handler.
///
/// # Handler Signatures
///
/// The middleware functions take a broker handler and return a wrapped version:
/// - Task: `Arc<dyn Fn(Arc<Task>) -> Box<dyn Future<Output = ()> + Send>>`
/// - Job: `Arc<dyn Fn(Job) -> Box<dyn Future<Output = ()> + Send>>`
/// - Node: `Arc<dyn Fn(Node) -> Box<dyn Future<Output = ()> + Send>>`
/// - Log: `Arc<dyn Fn(TaskLogPart) -> Box<dyn Future<Output = ()> + Send>>`
#[derive(Clone, Default)]
pub struct Middleware {
    /// Middleware for job handlers (applied to `subscribe_for_jobs`)
    pub job: Vec<Arc<dyn Fn(BrokerJobHandler) -> BrokerJobHandler + Send + Sync>>,
    /// Middleware for task handlers (applied to `subscribe_for_tasks`)
    pub task: Vec<Arc<dyn Fn(BrokerTaskHandler) -> BrokerTaskHandler + Send + Sync>>,
    /// Middleware for node handlers (applied to `subscribe_for_heartbeats`)
    pub node: Vec<Arc<dyn Fn(BrokerNodeHandler) -> BrokerNodeHandler + Send + Sync>>,
    /// Middleware for log handlers (applied to `subscribe_for_task_log_part`)
    pub log: Vec<Arc<dyn Fn(BrokerLogHandler) -> BrokerLogHandler + Send + Sync>>,
}

impl std::fmt::Debug for Middleware {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Middleware")
            .field("job", &format!("[{} middleware fns]", self.job.len()))
            .field("task", &format!("[{} middleware fns]", self.task.len()))
            .field("node", &format!("[{} middleware fns]", self.node.len()))
            .field("log", &format!("[{} middleware fns]", self.log.len()))
            .finish()
    }
}

impl Middleware {
    /// Apply the job middleware chain to a handler.
    fn apply_job(&self, handler: BrokerJobHandler) -> BrokerJobHandler {
        self.job.iter().fold(handler, |h, mw| mw(h))
    }

    /// Apply the task middleware chain to a handler.
    fn apply_task(&self, handler: BrokerTaskHandler) -> BrokerTaskHandler {
        self.task.iter().fold(handler, |h, mw| mw(h))
    }

    /// Apply the node middleware chain to a handler.
    fn apply_node(&self, handler: BrokerNodeHandler) -> BrokerNodeHandler {
        self.node.iter().fold(handler, |h, mw| mw(h))
    }

    /// Apply the log middleware chain to a handler.
    fn apply_log(&self, handler: BrokerLogHandler) -> BrokerLogHandler {
        self.log.iter().fold(handler, |h, mw| mw(h))
    }
}

// ---------------------------------------------------------------------------
// Config
// ---------------------------------------------------------------------------

/// Coordinator configuration.
///
/// Go parity with `coordinator.Config`.
pub struct Config {
    /// Coordinator name
    pub name: String,
    /// Message broker
    pub broker: Arc<dyn Broker>,
    /// Persistent datastore
    pub datastore: Arc<dyn Datastore>,
    /// Distributed locker
    pub locker: Arc<dyn locker::Locker>,
    /// API listen address (e.g. "0.0.0.0:8000")
    pub address: String,
    /// Queue concurrency settings (queue name → number of consumers)
    pub queues: HashMap<String, i64>,
    /// Enabled API endpoints
    pub enabled: HashMap<String, bool>,
    /// Middleware chains for handlers
    ///
    /// Go parity: `cfg.Middleware`
    pub middleware: Middleware,
}

impl std::fmt::Debug for Config {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Config")
            .field("name", &self.name)
            .field("address", &self.address)
            .field("queues", &self.queues)
            .field("enabled", &self.enabled)
            .field("broker", &"<dyn Broker>")
            .field("datastore", &"<dyn Datastore>")
            .field("locker", &"<dyn Locker>")
            .finish()
    }
}

impl Clone for Config {
    fn clone(&self) -> Self {
        Self {
            name: self.name.clone(),
            broker: self.broker.clone(),
            datastore: self.datastore.clone(),
            locker: self.locker.clone(),
            address: self.address.clone(),
            queues: self.queues.clone(),
            enabled: self.enabled.clone(),
            middleware: self.middleware.clone(),
        }
    }
}

// ---------------------------------------------------------------------------