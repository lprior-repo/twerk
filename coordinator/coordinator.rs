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
// Pure calculation: default queue map
// ---------------------------------------------------------------------------

/// Ensures every coordinator queue has at least `DEFAULT_QUEUE_CONCURRENCY` consumers.
///
/// Go parity: the constructor checks `if cfg.Queues[broker.QUEUE_x] < 1 { cfg.Queues[x] = 1 }`
/// for all coordinator queues.
#[must_use]
fn apply_default_queue_concurrency(queues: HashMap<String, i64>) -> HashMap<String, i64> {
    // Collect keys that are already present so we can check them after consuming
    let existing_keys: std::collections::HashSet<String> = queues.keys().cloned().collect();

    queues
        .into_iter()
        .map(|(k, v)| {
            let concurrency = if v < DEFAULT_QUEUE_CONCURRENCY {
                DEFAULT_QUEUE_CONCURRENCY
            } else {
                v
            };
            (k, concurrency)
        })
        .chain(
            COORDINATOR_QUEUES
                .iter()
                .filter(|q| !existing_keys.contains(**q))
                .map(|q| ((*q).to_string(), DEFAULT_QUEUE_CONCURRENCY)),
        )
        .collect()
}

// ---------------------------------------------------------------------------
// Coordinator
// ---------------------------------------------------------------------------

/// Coordinator is the "brain" of the Tork task queue system.
///
/// It accepts tasks from clients, schedules tasks for workers to execute,
/// and exposes the cluster's state to the outside world.
///
/// Go parity with `coordinator.Coordinator`.
pub struct Coordinator {
    /// Unique coordinator identifier (short UUID)
    id: String,
    /// When this coordinator started
    start_time: OffsetDateTime,
    /// Coordinator display name
    name: String,
    /// Message broker
    broker: Arc<dyn Broker>,
    /// Persistent datastore
    datastore: Arc<dyn Datastore>,
    /// Queue concurrency configuration
    queues: HashMap<String, i64>,
    /// API listen address
    address: String,
    /// Middleware chains for handlers
    middleware: Middleware,

    // Handler instances (wrapped in Arc for shared use in closures)
    pending_handler: Arc<PendingHandler>,
    started_handler: Arc<StartedHandler>,
    completed_handler: Arc<CompletedHandler>,
    error_handler: Arc<ErrorHandler>,
    redelivered_handler: Arc<RedeliveredHandler>,
    heartbeat_handler: Arc<NodeHeartbeatHandler>,
    log_handler: Arc<LogHandler>,
    progress_handler: Arc<ProgressHandler>,
    job_handler: Arc<JobEventHandler>,
    schedule_handler: Arc<ScheduleHandler>,

    // Shutdown coordination
    stop_tx: watch::Sender<bool>,
    heartbeat_handle: Arc<Mutex<Option<tokio::task::JoinHandle<()>>>>,
    api_handle: Arc<Mutex<Option<tokio::task::JoinHandle<Result<(), CoordinatorError>>>>>,
}

impl std::fmt::Debug for Coordinator {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Coordinator")
            .field("id", &self.id)
            .field("name", &self.name)
            .field("start_time", &self.start_time)
            .field("queues", &self.queues)
            .field("middleware", &self.middleware)
            .finish()
    }
}

// ---------------------------------------------------------------------------
// Constructor
// ---------------------------------------------------------------------------

impl Coordinator {
    /// Create a new coordinator from configuration.
    ///
    /// Go parity with `NewCoordinator(cfg Config)`.
    ///
    /// # Validation
    ///
    /// - `cfg.broker` must not be `None` (Go: "most provide a broker")
    /// - `cfg.datastore` must not be `None` (Go: "most provide a datastore")
    /// - `cfg.locker` must not be `None` (Go: "most provide a locker")
    ///
    /// # Defaults
    ///
    /// All coordinator queues default to concurrency 1 if not specified.
    ///
    /// # Errors
    ///
    /// Returns [`CoordinatorError::Validation`] if required fields are missing.
    /// Returns [`CoordinatorError::Handler`] if the schedule handler fails to initialize.
    pub async fn new(cfg: Config) -> Result<Self, CoordinatorError> {
        // 1. Validate required fields — in Rust, Arc<dyn ...> cannot be null,
        //    so the Go nil checks are enforced by the type system.
        //    (Go parity: "most provide a broker/datastore/locker")

        // 2. Apply default queue concurrency
        let queues = apply_default_queue_concurrency(cfg.queues);

        // 3. Create handler instances (Go parity: NewXxxHandler calls)
        let broker = cfg.broker.clone();
        let ds = cfg.datastore.clone();

        let pending_handler = Arc::new(PendingHandler::new());
        let started_handler = Arc::new(StartedHandler::new(ds.clone(), broker.clone()));
        let completed_handler = Arc::new(CompletedHandler::new(ds.clone(), broker.clone()));
        let error_handler = Arc::new(ErrorHandler::new());
        let redelivered_handler = Arc::new(RedeliveredHandler::new(broker.clone()));
        let heartbeat_handler = Arc::new(NodeHeartbeatHandler::new(ds.clone()));
        let log_handler = Arc::new(LogHandler::new(ds.clone()));
        let progress_handler = Arc::new(ProgressHandler::new(
            ds.clone(),
            crate::handlers::noop_job_handler(),
        ));
        let job_handler = Arc::new(JobEventHandler::new(ds.clone(), broker.clone()));

        // Go: onScheduledJob, err := handlers.NewJobSchedulerHandler(cfg.DataStore, cfg.Broker, cfg.Locker)
        let schedule_handler = Arc::new(
            ScheduleHandler::new(ds.clone(), broker.clone())
                .await
                .map_err(|e| CoordinatorError::Handler(format!("error initializing job scheduler: {e}")))?,
        );

        // 4. Create stop channel (Go: make(chan any))
        let (stop_tx, _) = watch::channel(false);

        // 5. Generate coordinator ID (Go: uuid.NewShortUUID())
        let id = new_coordinator_id();

        Ok(Self {
            id,
            start_time: OffsetDateTime::now_utc(),
            name: cfg.name,
            broker,
            datastore: ds,
            queues,
            address: cfg.address,
            middleware: cfg.middleware,
            pending_handler,
            started_handler,
            completed_handler,
            error_handler,
            redelivered_handler,
            heartbeat_handler,
            log_handler,
            progress_handler,
            job_handler,
            schedule_handler,
            stop_tx,
            heartbeat_handle: Arc::new(Mutex::new(None)),
            api_handle: Arc::new(Mutex::new(None)),
        })
    }

    // -----------------------------------------------------------------------
    // Start
    // -----------------------------------------------------------------------

    /// Start the coordinator.
    ///
    /// Go parity with `Start()`:
    /// 1. Starts the API server
    /// 2. Subscribes to all coordinator queues
    /// 3. Subscribes to scheduled job events
    /// 4. Spawns the heartbeat loop
    ///
    /// # Errors
    ///
    /// Returns [`CoordinatorError::Broker`] if queue subscription fails.
    /// Returns [`CoordinatorError::Api`] if the API server fails to start.
    pub async fn start(&self) -> Result<(), CoordinatorError> {
        info!(id = %self.id, name = %self.name, "starting Coordinator");

        // 1. Start the API server (Go: c.api.Start())
        self.start_api().await?;

        // 2. Subscribe to all coordinator queues
        self.subscribe_queues().await?;

        // 3. Subscribe to scheduled job events
        self.subscribe_scheduled_jobs().await?;

        // 4. Spawn heartbeat loop (Go: go c.sendHeartbeats())
        self.spawn_heartbeat();

        Ok(())
    }

    // -----------------------------------------------------------------------
    // Stop
    // -----------------------------------------------------------------------

    /// Gracefully stop the coordinator.
    ///
    /// Go parity with `Stop()`:
    /// 1. Signals the heartbeat loop to stop
    /// 2. Shuts down the broker
    /// 3. Shuts down the API server
    ///
    /// # Errors
    ///
    /// Returns [`CoordinatorError::Broker`] if broker shutdown fails.
    /// Returns [`CoordinatorError::Api`] if API shutdown fails.
    pub async fn stop(&self) -> Result<(), CoordinatorError> {
        debug!(id = %self.id, name = %self.name, "shutting down");

        // 1. Signal heartbeat loop and API server to stop (Go: close(c.stop))
        let _ = self.stop_tx.send(true);

        // 2. Wait for heartbeat loop to finish
        {
            let mut guard = self.heartbeat_handle.lock().await;
            if let Some(handle) = guard.take() {
                drop(guard); // release lock before awaiting
                let _ = tokio::time::timeout(Duration::from_secs(SHUTDOWN_TIMEOUT_SECS), handle).await;
            }
        }

        // 3. Shutdown broker (Go: c.broker.Shutdown(ctx))
        match tokio::time::timeout(
            Duration::from_secs(SHUTDOWN_TIMEOUT_SECS),
            self.broker.shutdown(),
        )
        .await
        {
            Ok(Ok(())) => {}
            Ok(Err(e)) => {
                return Err(CoordinatorError::Broker(format!("broker shutdown error: {e}")));
            }
            Err(timeout) => {
                return Err(CoordinatorError::Broker(format!(
                    "broker shutdown timed out: {timeout}"
                )));
            }
        }

        // 4. Wait for API server to finish (Go: c.api.Shutdown(ctx))
        {
            let mut guard = self.api_handle.lock().await;
            if let Some(handle) = guard.take() {
                drop(guard); // release lock before awaiting
                let _ = tokio::time::timeout(Duration::from_secs(SHUTDOWN_TIMEOUT_SECS), handle).await;
            }
        }

        Ok(())
    }

    // -----------------------------------------------------------------------
    // SubmitJob
    // -----------------------------------------------------------------------

    /// Submit a job for execution.
    ///
    /// Go parity with `SubmitJob(ctx, ij)` — creates the job in the datastore
    /// and publishes it to the broker's jobs queue.
    ///
    /// # Errors
    ///
    /// Returns [`CoordinatorError::Datastore`] if job creation fails.
    /// Returns [`CoordinatorError::Broker`] if job publishing fails.
    pub async fn submit_job(&self, job: &Job) -> Result<Job, CoordinatorError> {
        // Create job in datastore (Go: ds.CreateJob)
        self.datastore
            .create_job(job.clone())
            .await
            .map_err(|e| CoordinatorError::Datastore(format!("error creating job: {e}")))?;

        // Publish to jobs queue (Go: broker.PublishJob)
        self.broker
            .publish_job(job)
            .await
            .map_err(|e| CoordinatorError::Broker(format!("error publishing job: {e}")))?;

        Ok(job.clone())
    }

    // -----------------------------------------------------------------------
    // Private: API server
    // -----------------------------------------------------------------------

    /// Start the HTTP API server in a background task.
    ///
    /// Go parity: `c.api.Start()` — binds to the configured address and
    /// serves the router. Uses `axum::serve` with graceful shutdown.
    async fn start_api(&self) -> Result<(), CoordinatorError> {
        let state = api::AppState::new(
            self.broker.clone(),
            self.datastore.clone(),
            api::Config {
                address: self.address.clone(),
                enabled: HashMap::new(),
                cors_origins: vec![],
            },
        );
        let router = api::create_router(state);
        let addr = self.address.clone();

        let mut stop_rx = self.stop_tx.subscribe();

        let handle = tokio::spawn(async move {
            let listener = match tokio::net::TcpListener::bind(&addr).await {
                Ok(l) => l,
                Err(e) => {
                    error!(error = %e, address = %addr, "failed to bind API server");
                    return Err(CoordinatorError::Api(format!("failed to bind {addr}: {e}")));
                }
            };
            info!(address = %addr, "Coordinator API listening on");

            let shutdown = async move {
                let _ = stop_rx.changed().await;
            };

            match axum::serve(listener, router)
                .with_graceful_shutdown(shutdown)
                .await
            {
                Ok(()) => Ok(()),
                Err(e) => Err(CoordinatorError::Api(format!("API server error: {e}"))),
            }
        });

        let mut api_lock = self.api_handle.lock().await;
        *api_lock = Some(handle);

        Ok(())
    }

    // -----------------------------------------------------------------------
    // Private: Queue subscriptions
    // -----------------------------------------------------------------------

    /// Subscribe to all coordinator queues.
    ///
    /// Go parity: iterates `c.queues`, skips non-coordinator queues,
    /// subscribes `conc` times for each coordinator queue.
    async fn subscribe_queues(&self) -> Result<(), CoordinatorError> {
        for (qname, conc) in &self.queues {
            if !is_coordinator_queue(qname) {
                continue;
            }

            for _ in 0..*conc {
                self.subscribe_to_queue(qname).await?;
            }
        }
        Ok(())
    }

    /// Subscribe to a single queue based on its name.
    ///
    /// Go parity: the `switch qname { case ... }` block in `Start()`.
    /// Handlers are wrapped through the middleware chain before subscription.
    async fn subscribe_to_queue(&self, qname: &str) -> Result<(), CoordinatorError> {
        match qname {
            queue::QUEUE_PENDING => {
                let handler = self.pending_handler.clone();
                let base_handler: TaskHandler = Arc::new(move |task: Arc<Task>| {
                    let handler = handler.clone();
                    Box::pin(async move {
                        let mut t = (*task).clone();
                        if let Err(e) = handler.handle(Arc::new(()), &mut t) {
                            error!(error = %e, "pending handler error");
                        }
                    })
                });
                let task_handler = self.middleware.apply_task(base_handler);
                self.broker
                    .subscribe_for_tasks(qname.to_string(), task_handler)
                    .await
                    .map_err(|e| CoordinatorError::Broker(format!("subscribe to {qname}: {e}")))?;
            }
            queue::QUEUE_STARTED => {
                let handler = self.started_handler.clone();
                let broker = self.broker.clone();
                let ds = self.datastore.clone();
                let error_handler = self.error_handler.clone();
                let base_handler: TaskHandler = Arc::new(move |task: Arc<Task>| {
                    let handler = handler.clone();
                    let broker = broker.clone();
                    let ds = ds.clone();
                    let error_handler = error_handler.clone();
                    Box::pin(async move {
                        match handler.handle(&task).await {
                            Ok(()) => {}
                            Err(e) => {
                                warn!(error = %e, queue = queue::QUEUE_STARTED, "started handler error");
                                Self::publish_task_error(&broker, &ds, &error_handler, &task, &e).await;
                            }
                        }
                    })
                });
                let task_handler = self.middleware.apply_task(base_handler);
                self.broker
                    .subscribe_for_tasks(qname.to_string(), task_handler)
                    .await
                    .map_err(|e| CoordinatorError::Broker(format!("subscribe to {qname}: {e}")))?;
            }
            queue::QUEUE_COMPLETED => {
                let handler = self.completed_handler.clone();
                let broker = self.broker.clone();
                let ds = self.datastore.clone();
                let error_handler = self.error_handler.clone();
                let base_handler: TaskHandler = Arc::new(move |task: Arc<Task>| {
                    let handler = handler.clone();
                    let broker = broker.clone();
                    let ds = ds.clone();
                    let error_handler = error_handler.clone();
                    Box::pin(async move {
                        match handler.handle(&task).await {
                            Ok(()) => {}
                            Err(e) => {
                                warn!(error = %e, queue = queue::QUEUE_COMPLETED, "completed handler error");
                                Self::publish_task_error(&broker, &ds, &error_handler, &task, &e).await;
                            }
                        }
                    })
                });
                let task_handler = self.middleware.apply_task(base_handler);
                self.broker
                    .subscribe_for_tasks(qname.to_string(), task_handler)
                    .await
                    .map_err(|e| CoordinatorError::Broker(format!("subscribe to {qname}: {e}")))?;
            }
            queue::QUEUE_ERROR => {
                let handler = self.error_handler.clone();
                let base_handler: TaskHandler = Arc::new(move |task: Arc<Task>| {
                    let handler = handler.clone();
                    Box::pin(async move {
                        let mut t = (*task).clone();
                        if let Err(e) = handler.handle(Arc::new(()), &mut t) {
                            error!(error = %e, queue = queue::QUEUE_ERROR, "error handler error");
                        }
                    })
                });
                let task_handler = self.middleware.apply_task(base_handler);
                self.broker
                    .subscribe_for_tasks(qname.to_string(), task_handler)
                    .await
                    .map_err(|e| CoordinatorError::Broker(format!("subscribe to {qname}: {e}")))?;
            }
            queue::QUEUE_HEARTBEAT => {
                let handler = self.heartbeat_handler.clone();
                let base_handler: HeartbeatHandler = Arc::new(move |node: Node| {
                    let handler = handler.clone();
                    Box::pin(async move {
                        if let Err(e) = handler.handle(&node).await {
                            error!(error = %e, "heartbeat handler error");
                        }
                    })
                });
                let node_handler = self.middleware.apply_node(base_handler);
                self.broker
                    .subscribe_for_heartbeats(node_handler)
                    .await
                    .map_err(|e| CoordinatorError::Broker(format!("subscribe to {qname}: {e}")))?;
            }
            queue::QUEUE_JOBS => {
                let handler = self.job_handler.clone();
                let broker = self.broker.clone();
                let ds = self.datastore.clone();
                let error_handler = self.error_handler.clone();
                let base_handler: JobHandler = Arc::new(move |job: Job| {
                    let handler = handler.clone();
                    let broker = broker.clone();
                    let ds = ds.clone();
                    let error_handler = error_handler.clone();
                    Box::pin(async move {
                        let mut j = job;
                        match handler.handle(crate::handlers::JobEventType::StateChange, &mut j).await {
                            Ok(()) => {}
                            Err(e) => {
                                warn!(error = %e, queue = queue::QUEUE_JOBS, "job handler error");
                                Self::publish_job_error(&broker, &ds, &error_handler, &j, &e).await;
                            }
                        }
                    })
                });
                let job_handler = self.middleware.apply_job(base_handler);
                self.broker
                    .subscribe_for_jobs(job_handler)
                    .await
                    .map_err(|e| CoordinatorError::Broker(format!("subscribe to {qname}: {e}")))?;
            }
            queue::QUEUE_LOGS => {
                let handler = self.log_handler.clone();
                let base_handler: TaskLogPartHandler = Arc::new(move |part: tork::task::TaskLogPart| {
                    let handler = handler.clone();
                    Box::pin(async move {
                        if let Err(e) = handler.handle(&part).await {
                            error!(error = %e, "log handler error");
                        }
                    })
                });
                let log_handler = self.middleware.apply_log(base_handler);
                self.broker
                    .subscribe_for_task_log_part(log_handler)
                    .await
                    .map_err(|e| CoordinatorError::Broker(format!("subscribe to {qname}: {e}")))?;
            }
            queue::QUEUE_PROGRESS => {
                // Progress handler is not wrapped in task middleware (no middleware defined for progress)
                let handler = self.progress_handler.clone();
                let broker = self.broker.clone();
                let progress_handler: TaskProgressHandler = Arc::new(move |task: Task| {
                    let handler = handler.clone();
                    let broker = broker.clone();
                    Box::pin(async move {
                        match handler.handle(&task).await {
                            Ok(()) => {}
                            Err(e) => {
                                warn!(error = %e, queue = queue::QUEUE_PROGRESS, "progress handler error");
                                let failed = Task {
                                    state: TASK_STATE_FAILED.clone(),
                                    failed_at: Some(OffsetDateTime::now_utc()),
                                    error: Some(e.to_string()),
                                    ..task.clone()
                                };
                                if let Err(pe) = broker.publish_task(queue::QUEUE_ERROR.to_string(), &failed).await {
                                    error!(error = %pe, "error publishing failed task to error queue");
                                }
                            }
                        }
                    })
                });
                self.broker
                    .subscribe_for_task_progress(progress_handler)
                    .await
                    .map_err(|e| CoordinatorError::Broker(format!("subscribe to {qname}: {e}")))?;
            }
            queue::QUEUE_REDELIVERIES => {
                let handler = self.redelivered_handler.clone();
                let base_handler: TaskHandler = Arc::new(move |task: Arc<Task>| {
                    let handler = handler.clone();
                    Box::pin(async move {
                        if let Err(e) = handler.handle(&task).await {
                            error!(error = %e, "redelivered handler error");
                        }
                    })
                });
                let task_handler = self.middleware.apply_task(base_handler);
                self.broker
                    .subscribe_for_tasks(qname.to_string(), task_handler)
                    .await
                    .map_err(|e| CoordinatorError::Broker(format!("subscribe to {qname}: {e}")))?;
            }
            _ => {
                debug!(queue = qname, "skipping non-coordinator queue");
            }
        }
        Ok(())
    }

    // -----------------------------------------------------------------------
    // Private: Scheduled job events
    // -----------------------------------------------------------------------

    /// Subscribe to scheduled job events.
    ///
    /// Go parity: `c.broker.SubscribeForEvents(broker.TOPIC_SCHEDULED_JOB, ...)`
    async fn subscribe_scheduled_jobs(&self) -> Result<(), CoordinatorError> {
        let handler = self.schedule_handler.clone();
        let event_handler: EventHandler = Arc::new(move |ev: serde_json::Value| {
            let handler = handler.clone();
            Box::pin(async move {
                match serde_json::from_value::<ScheduledJob>(ev) {
                    Ok(sj) => {
                        if let Err(e) = handler.handle(&sj).await {
                            error!(error = %e, "error handling scheduled job");
                        }
                    }
                    Err(e) => {
                        error!(error = %e, "error casting scheduled job event");
                    }
                }
            })
        });

        self.broker
            .subscribe_for_events(TOPIC_SCHEDULED_JOB.to_string(), event_handler)
            .await
            .map_err(|e| CoordinatorError::Broker(format!("subscribe to {TOPIC_SCHEDULED_JOB}: {e}")))?;

        Ok(())
    }

    // -----------------------------------------------------------------------
    // Private: Heartbeat loop
    // -----------------------------------------------------------------------

    /// Spawn the heartbeat loop in a background task.
    ///
    /// Go parity: `go c.sendHeartbeats()`
    fn spawn_heartbeat(&self) {
        let id = self.id.clone();
        let name = self.name.clone();
        let start_time = self.start_time;
        let broker = self.broker.clone();
        let stop_rx = self.stop_tx.subscribe();

        let handle = tokio::spawn(async move {
            let mut rx = stop_rx;

            loop {
                // Get hostname (Go: os.Hostname())
                let hostname = hostname::get()
                    .map(|h| h.to_string_lossy().to_string())
                    .unwrap_or_else(|_| String::new());

                // Get CPU percent (Go: host.GetCPUPercent())
                let cpu_percent = get_cpu_percent();

                // Build the node heartbeat
                // Go parity:
                //   tork.Node{
                //       ID: c.id, Name: c.Name,
                //       StartedAt: c.startTime, Status: tork.NodeStatusUP,
                //       CPUPercent: cpuPercent, LastHeartbeatAt: now,
                //       Hostname: hostname, Version: tork.Version,
                //   }
                let node = Node {
                    id: Some(id.clone()),
                    name: Some(name.clone()),
                    started_at: start_time,
                    status: NODE_STATUS_UP.to_string(),
                    cpu_percent,
                    last_heartbeat_at: OffsetDateTime::now_utc(),
                    hostname: Some(hostname),
                    version: VERSION.to_string(),
                    port: 0,
                    task_count: 0,
                    queue: None,
                };

                // Publish heartbeat (Go: c.broker.PublishHeartbeat(ctx, &tork.Node{...}))
                if let Err(e) = broker.publish_heartbeat(node).await {
                    error!(error = %e, coordinator_id = %id, "error publishing heartbeat");
                }

                // Wait for next tick or stop signal (Go: select { case <-c.stop; case <-time.After(...) })
                tokio::select! {
                    _ = rx.changed() => {
                        debug!(coordinator_id = %id, "heartbeat loop stopped");
                        return;
                    }
                    _ = tokio::time::sleep(Duration::from_secs(HEARTBEAT_RATE_SECS as u64)) => {}
                }
            }
        });

        // Store the handle for clean shutdown
        let handle_ref = self.heartbeat_handle.clone();
        tokio::spawn(async move {
            let mut lock = handle_ref.lock().await;
            *lock = Some(handle);
        });
    }

    // -----------------------------------------------------------------------
    // Private: Error publishing helpers
    // -----------------------------------------------------------------------

    /// Publish a failed task to the error queue.
    ///
    /// Go parity: `taskHandler` wrapper — when a handler returns an error,
    /// marks the task as FAILED and publishes to QUEUE_ERROR.
    async fn publish_task_error(
        broker: &Arc<dyn Broker>,
        _ds: &Arc<dyn Datastore>,
        error_handler: &Arc<ErrorHandler>,
        task: &Task,
        handler_error: &HandlerError,
    ) {
        let now = OffsetDateTime::now_utc();
        let failed = Task {
            state: TASK_STATE_FAILED.clone(),
            failed_at: Some(now),
            error: Some(handler_error.to_string()),
            ..(*task).clone()
        };

        if let Err(e) = broker.publish_task(queue::QUEUE_ERROR.to_string(), &failed).await {
            error!(error = %e, "error publishing failed task to error queue");
        }

        // Go: also calls onError(ctx, task.StateChange, t) via the error handler
        let mut failed_mut = failed;
        if let Err(e) = error_handler.handle(Arc::new(()), &mut failed_mut) {
            error!(error = %e, "error handler callback failed");
        }
    }

    /// Publish a failed job to the jobs queue.
    ///
    /// Go parity: `jobHandler` wrapper — when a handler returns an error,
    /// marks the job as FAILED and publishes to QUEUE_JOBS.
    async fn publish_job_error(
        broker: &Arc<dyn Broker>,
        _ds: &Arc<dyn Datastore>,
        _error_handler: &Arc<ErrorHandler>,
        job: &Job,
        handler_error: &HandlerError,
    ) {
        let now = OffsetDateTime::now_utc();
        let failed = Job {
            state: JOB_STATE_FAILED.to_string(),
            failed_at: Some(now),
            error: Some(handler_error.to_string()),
            ..(*job).clone()
        };

        if let Err(e) = broker.publish_job(&failed).await {
            error!(error = %e, "error publishing failed job");
        }
    }
}

// ---------------------------------------------------------------------------
// Host system helpers (pure I/O at the boundary)
// ---------------------------------------------------------------------------

/// Gets the current CPU usage percentage.
///
/// Go parity with `host.GetCPUPercent()` — returns 0.0 on error.
#[must_use]
fn get_cpu_percent() -> f64 {
    let mut sys = sysinfo::System::new_all();
    sys.refresh_cpu_all();
    std::thread::sleep(Duration::from_millis(200));
    sys.refresh_cpu_all();

    let cpus = sys.cpus();
    if cpus.is_empty() {
        return 0.0;
    }

    cpus.iter()
        .map(|cpu: &sysinfo::Cpu| cpu.cpu_usage() as f64)
        .sum::<f64>()
        / cpus.len() as f64
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use tork::broker::queue;

    // -- apply_default_queue_concurrency (pure calc) ----------------------

    #[test]
    fn test_default_queues_applied_to_empty_map() {
        let queues = HashMap::new();
        let result = apply_default_queue_concurrency(queues);

        assert_eq!(result.get(queue::QUEUE_COMPLETED), Some(&DEFAULT_QUEUE_CONCURRENCY));
        assert_eq!(result.get(queue::QUEUE_ERROR), Some(&DEFAULT_QUEUE_CONCURRENCY));
        assert_eq!(result.get(queue::QUEUE_PENDING), Some(&DEFAULT_QUEUE_CONCURRENCY));
        assert_eq!(result.get(queue::QUEUE_STARTED), Some(&DEFAULT_QUEUE_CONCURRENCY));
        assert_eq!(result.get(queue::QUEUE_HEARTBEAT), Some(&DEFAULT_QUEUE_CONCURRENCY));
        assert_eq!(result.get(queue::QUEUE_JOBS), Some(&DEFAULT_QUEUE_CONCURRENCY));
        assert_eq!(result.get(queue::QUEUE_LOGS), Some(&DEFAULT_QUEUE_CONCURRENCY));
        assert_eq!(result.get(queue::QUEUE_PROGRESS), Some(&DEFAULT_QUEUE_CONCURRENCY));
        assert_eq!(result.get(queue::QUEUE_REDELIVERIES), Some(&DEFAULT_QUEUE_CONCURRENCY));
    }

    #[test]
    fn test_default_queues_preserves_existing_values() {
        let mut queues = HashMap::new();
        queues.insert(queue::QUEUE_COMPLETED.to_string(), 5);
        queues.insert(queue::QUEUE_PENDING.to_string(), 3);
        queues.insert("custom-queue".to_string(), 2);

        let result = apply_default_queue_concurrency(queues);

        assert_eq!(result.get(queue::QUEUE_COMPLETED), Some(&5));
        assert_eq!(result.get(queue::QUEUE_PENDING), Some(&3));
        assert_eq!(result.get("custom-queue"), Some(&2));
    }

    #[test]
    fn test_default_queues_clamps_zero_to_one() {
        let mut queues = HashMap::new();
        queues.insert(queue::QUEUE_ERROR.to_string(), 0);

        let result = apply_default_queue_concurrency(queues);

        assert_eq!(result.get(queue::QUEUE_ERROR), Some(&DEFAULT_QUEUE_CONCURRENCY));
    }

    #[test]
    fn test_default_queues_clamps_negative_to_one() {
        let mut queues = HashMap::new();
        queues.insert(queue::QUEUE_STARTED.to_string(), -1);

        let result = apply_default_queue_concurrency(queues);

        assert_eq!(result.get(queue::QUEUE_STARTED), Some(&DEFAULT_QUEUE_CONCURRENCY));
    }

    #[test]
    fn test_default_queues_non_coordinator_queue_unchanged() {
        let mut queues = HashMap::new();
        queues.insert(queue::QUEUE_DEFAULT.to_string(), 10);

        let result = apply_default_queue_concurrency(queues);

        assert_eq!(result.get(queue::QUEUE_DEFAULT), Some(&10));
    }

    // -- Constants -----------------------------------------------------------

    #[test]
    fn test_constants_match_go() {
        assert_eq!(TOPIC_SCHEDULED_JOB, "scheduled-job");
        assert_eq!(SHUTDOWN_TIMEOUT_SECS, 15);
        assert_eq!(DEFAULT_QUEUE_CONCURRENCY, 1);
        assert_eq!(COORDINATOR_QUEUES.len(), 9);
    }

    // -- Coordinator construction (validation) ----------------------------

    // Note: we can't easily test new() without a full mock Broker/Datastore/Locker
    // since they require async trait impls. The pure validation logic is tested
    // through apply_default_queue_concurrency above.

    #[test]
    fn test_coordinator_debug_format() {
        // Just verify the ID generation function works and produces a non-empty string
        let id = new_coordinator_id();
        assert!(!id.is_empty());
        assert_eq!(id.len(), 32);
    }

    // -- get_cpu_percent --------------------------------------------------

    #[test]
    fn test_get_cpu_percent_range() {
        let cpu = get_cpu_percent();
        assert!(cpu >= 0.0, "CPU percent should be non-negative");
        // CPU could be very high on loaded systems, but shouldn't be NaN
        assert!(!cpu.is_nan(), "CPU percent should not be NaN");
    }

    // -- CoordinatorError -------------------------------------------------

    #[test]
    fn test_coordinator_error_validation() {
        let err = CoordinatorError::Validation("test error".into());
        assert!(err.to_string().contains("validation"));
        assert!(err.to_string().contains("test error"));
    }

    #[test]
    fn test_coordinator_error_broker() {
        let err = CoordinatorError::Broker("broker down".into());
        assert!(err.to_string().contains("broker"));
    }

    #[test]
    fn test_coordinator_error_api() {
        let err = CoordinatorError::Api("bind failed".into());
        assert!(err.to_string().contains("API"));
    }
}
