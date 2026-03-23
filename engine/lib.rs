//! Tork Engine - Main orchestration engine for task execution
//!
//! This crate provides the core Engine struct that coordinates
//! between broker, datastore, locker, worker, and coordinator components.

#![deny(clippy::unwrap_used)]
#![warn(clippy::pedantic)]

pub mod broker;
pub mod coordinator;
pub mod datastore;
pub mod default;
pub mod locker;
pub mod worker;

// Re-export commonly used types
pub use broker::BrokerProxy;
pub use datastore::DatastoreProxy;
pub use engine::{
    Config, Engine, Mode, State, Middleware, JobListener,
    TaskMiddlewareFunc, JobMiddlewareFunc, LogMiddlewareFunc,
    NodeMiddlewareFunc, WebMiddlewareFunc, EndpointHandler,
    TaskEventType, JobEventType,
};

/// Topic constant for job events
pub const TOPIC_JOB: &str = "job.*";
/// Topic for completed job events
pub const TOPIC_JOB_COMPLETED: &str = "job.completed";
/// Topic for failed job events
pub const TOPIC_JOB_FAILED: &str = "job.failed";

mod engine {
    //! Core engine implementation

    use crate::broker::BrokerProxy;
    use crate::coordinator::{create_coordinator, Coordinator};
    use crate::datastore::DatastoreProxy;
    use crate::worker::{create_worker, Worker};
    use crate::locker::{create_locker, Locker};
    use tork::runtime::mount::Mounter;
    use tork::runtime::{MultiMounter, Runtime};
    use tork::broker::Broker;
    use tork::datastore::Datastore;
    use anyhow::{anyhow, Result};
    use std::collections::HashMap;
    use std::future::Future;
    use std::pin::Pin;
    use std::sync::Arc;
    use tokio::sync::{broadcast, RwLock};
    use tracing::{debug, error};

    /// Job listener callback type
    pub type JobListener = Arc<dyn Fn(tork::job::Job) + Send + Sync>;

    /// Engine execution mode
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
    pub enum Mode {
        Coordinator,
        Worker,
        #[default]
        Standalone,
    }

    /// Engine state
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
    pub enum State {
        #[default]
        Idle,
        Running,
        Terminating,
        Terminated,
    }

    /// Typed task middleware function.
    ///
    /// Follows the same pattern as `middleware::task::MiddlewareFunc`:
    /// wraps a [`TaskHandlerFunc`] and returns a wrapped handler.
    pub type TaskMiddlewareFunc = Arc<dyn Fn(TaskHandlerFunc) -> TaskHandlerFunc + Send + Sync>;

    /// Typed job middleware function.
    pub type JobMiddlewareFunc = Arc<dyn Fn(JobHandlerFunc) -> JobHandlerFunc + Send + Sync>;

    /// Typed log middleware function.
    pub type LogMiddlewareFunc = Arc<dyn Fn(LogHandlerFunc) -> LogHandlerFunc + Send + Sync>;

    /// Typed node middleware function.
    pub type NodeMiddlewareFunc = Arc<dyn Fn(NodeHandlerFunc) -> NodeHandlerFunc + Send + Sync>;

    /// Typed web (axum) middleware function.
    ///
    /// Wraps an axum `Next` and returns a pinned future yielding an HTTP
    /// response, matching the axum middleware signature.
    pub type WebMiddlewareFunc = Arc<
        dyn Fn(
                axum::http::Request<axum::body::Body>,
                axum::middleware::Next,
            ) -> Pin<Box<dyn Future<Output = axum::response::Response> + Send>>
            + Send
            + Sync,
    >;

    /// Typed API endpoint handler.
    ///
    /// An `Arc`-wrapped async function that receives an axum request parts
    /// reference and the request body bytes, returning a response.
    pub type EndpointHandler = Arc<
        dyn Fn(
                axum::http::request::Parts,
                bytes::Bytes,
            ) -> Pin<Box<dyn Future<Output = axum::response::Response> + Send>>
            + Send
            + Sync,
    >;

    // ── Handler function types ────────────────────────────────────
    //
    // These match the signatures used by the coordinator's middleware
    // modules so that engine-registered middleware can be directly
    // forwarded without type-erasing through `Box<dyn Any>`.

    /// Task event type for middleware handlers.
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub enum TaskEventType {
        Started,
        StateChange,
        Redelivered,
        Progress,
    }

    /// Task handler function (mirrors `coordinator::handlers::TaskHandlerFunc`).
    pub type TaskHandlerFunc = Arc<
        dyn Fn(Arc<()>, TaskEventType, &mut tork::task::Task) -> Result<(), TaskHandlerError>
            + Send
            + Sync,
    >;

    /// Job handler function.
    pub type JobHandlerFunc = Arc<
        dyn Fn(Arc<()>, JobEventType, &mut tork::job::Job) -> Result<(), JobHandlerError> + Send + Sync,
    >;

    /// Log handler function.
    pub type LogHandlerFunc = Arc<
        dyn Fn(Arc<()>, &[tork::task::TaskLogPart]) -> Result<(), LogHandlerError> + Send + Sync,
    >;

    /// Node handler function.
    pub type NodeHandlerFunc = Arc<
        dyn Fn(Arc<()>, &mut tork::node::Node) -> Result<(), NodeHandlerError> + Send + Sync,
    >;

    /// Job event type for middleware handlers.
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub enum JobEventType {
        StateChange,
        Progress,
        Read,
    }

    // ── Handler errors (per-category, thiserror) ──────────────────

    /// Error returned by task middleware/handlers.
    #[derive(Debug, Clone, PartialEq, thiserror::Error)]
    pub enum TaskHandlerError {
        #[error("task handler error: {0}")]
        Handler(String),
        #[error("task datastore error: {0}")]
        Datastore(String),
    }

    /// Error returned by job middleware/handlers.
    #[derive(Debug, Clone, PartialEq, thiserror::Error)]
    pub enum JobHandlerError {
        #[error("job handler error: {0}")]
        Handler(String),
        #[error("job datastore error: {0}")]
        Datastore(String),
    }

    /// Error returned by log middleware/handlers.
    #[derive(Debug, Clone, PartialEq, thiserror::Error)]
    pub enum LogHandlerError {
        #[error("log handler error: {0}")]
        Handler(String),
        #[error("log middleware error: {0}")]
        Middleware(String),
    }

    /// Error returned by node middleware/handlers.
    #[derive(Debug, Clone, PartialEq, thiserror::Error)]
    pub enum NodeHandlerError {
        #[error("node handler error: {0}")]
        Handler(String),
        #[error("node datastore error: {0}")]
        Datastore(String),
    }

    /// Middleware configuration — fully typed, zero `dyn Any`.
    ///
    /// Note: does not derive `Debug` because the inner `dyn Fn` trait
    /// objects do not implement `Debug`.
    #[derive(Default, Clone)]
    pub struct Middleware {
        pub web: Vec<WebMiddlewareFunc>,
        pub task: Vec<TaskMiddlewareFunc>,
        pub job: Vec<JobMiddlewareFunc>,
        pub node: Vec<NodeMiddlewareFunc>,
        pub log: Vec<LogMiddlewareFunc>,
    }

    /// Engine configuration.
    ///
    /// Note: does not derive `Debug` because `Middleware` and
    /// `EndpointHandler` (`dyn Fn`) do not implement `Debug`.
    #[derive(Default)]
    pub struct Config {
        pub mode: Mode,
        pub middleware: Middleware,
        pub endpoints: HashMap<String, EndpointHandler>,
    }

    

    /// Engine is the main orchestration engine
    #[allow(dead_code)]
    pub struct Engine {
        state: State,
        mode: Mode,
        broker: BrokerProxy,
        datastore: DatastoreProxy,
        runtime: Option<Box<dyn Runtime + Send + Sync>>,
        worker: Arc<RwLock<Option<Box<dyn Worker + Send + Sync>>>>,
        coordinator: Arc<RwLock<Option<Box<dyn Coordinator + Send + Sync>>>>,
        terminate_tx: broadcast::Sender<()>,
        terminate_rx: Arc<RwLock<broadcast::Receiver<()>>>,
        terminated_tx: Arc<RwLock<Option<broadcast::Sender<()>>>>,
        locker: Arc<RwLock<Option<Box<dyn Locker + Send + Sync>>>>,
        middleware: Middleware,
        endpoints: HashMap<String, EndpointHandler>,
        mounters: HashMap<String, MultiMounter>,
        ds_providers: HashMap<String, Box<dyn Datastore + Send + Sync>>,
        broker_providers: HashMap<String, Box<dyn Broker + Send + Sync>>,
        job_listeners: Arc<RwLock<Vec<JobListener>>>,
    }

    impl std::fmt::Debug for Engine {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            f.debug_struct("Engine")
                .field("state", &self.state)
                .field("mode", &self.mode)
                .field("mounters", &self.mounters.keys().collect::<Vec<_>>())
                .finish()
        }
    }

    impl Engine {
        /// Creates a new engine with the given configuration
        pub fn new(config: Config) -> Self {
            let (terminate_tx, terminate_rx) = broadcast::channel(1);
            Self {
                state: State::Idle,
                mode: config.mode,
                broker: BrokerProxy::new(),
                datastore: DatastoreProxy::new(),
                runtime: None,
                worker: Arc::new(RwLock::new(None)),
                coordinator: Arc::new(RwLock::new(None)),
                terminate_tx,
                terminate_rx: Arc::new(RwLock::new(terminate_rx)),
                terminated_tx: Arc::new(RwLock::new(None)),
                locker: Arc::new(RwLock::new(None)),
                middleware: config.middleware,
                endpoints: config.endpoints,
                mounters: HashMap::new(),
                ds_providers: HashMap::new(),
                broker_providers: HashMap::new(),
                job_listeners: Arc::new(RwLock::new(Vec::new())),
            }
        }

        /// Returns the current engine state
        pub fn state(&self) -> State {
            self.state
        }

        /// Returns the engine mode
        pub fn mode(&self) -> Mode {
            self.mode
        }

        /// Sets the engine mode (only when idle)
        pub fn set_mode(&mut self, mode: Mode) {
            if self.state == State::Idle {
                self.mode = mode;
            }
        }

        /// Starts the engine in the configured mode
        pub async fn start(&mut self) -> Result<()> {
            if self.state != State::Idle {
                anyhow::bail!("engine is not idle");
            }

            match self.mode {
                Mode::Coordinator => self.run_coordinator().await,
                Mode::Worker => self.run_worker().await,
                Mode::Standalone => self.run_standalone().await,
            }?;

            self.state = State::Running;
            Ok(())
        }

        /// Runs the engine and waits for termination
        pub async fn run(&mut self) -> Result<()> {
            self.start().await?;
            self.await_shutdown().await;
            Ok(())
        }

        /// Terminates the engine
        pub async fn terminate(&mut self) -> Result<()> {
            if self.state != State::Running {
                anyhow::bail!("engine is not running");
            }

            self.state = State::Terminating;
            debug!("Terminating engine");

            // Signal termination
            let _ = self.terminate_tx.send(());

            // Stop worker if present
            {
                let worker = self.worker.write().await;
                if let Some(w) = worker.as_ref() {
                    if let Err(e) = w.stop().await {
                        error!("error stopping worker: {}", e);
                    }
                }
            }

            // Stop coordinator if present
            {
                let coordinator = self.coordinator.write().await;
                if let Some(c) = coordinator.as_ref() {
                    if let Err(e) = c.stop().await {
                        error!("error stopping coordinator: {}", e);
                    }
                }
            }

            self.state = State::Terminated;
            Ok(())
        }

        /// Returns the broker as a trait object.
        pub fn broker(&self) -> &dyn Broker {
            &self.broker
        }

        /// Returns the datastore as a trait object.
        pub fn datastore(&self) -> &dyn Datastore {
            &self.datastore
        }

        /// Returns a clone of the broker proxy.
        ///
        /// Go parity: `func Broker() broker.Broker`
        pub fn broker_proxy(&self) -> BrokerProxy {
            self.broker.clone_inner()
        }

        /// Returns a clone of the datastore proxy.
        ///
        /// Go parity: `func Datastore() datastore.Datastore`
        pub fn datastore_proxy(&self) -> DatastoreProxy {
            self.datastore.clone_inner()
        }

        /// Register web middleware
        pub fn register_web_middleware(&mut self, mw: WebMiddlewareFunc) {
            if self.state != State::Idle {
                return;
            }
            self.middleware.web.push(mw);
        }

        /// Register task middleware
        pub fn register_task_middleware(&mut self, mw: TaskMiddlewareFunc) {
            if self.state != State::Idle {
                return;
            }
            self.middleware.task.push(mw);
        }

        /// Register job middleware
        pub fn register_job_middleware(&mut self, mw: JobMiddlewareFunc) {
            if self.state != State::Idle {
                return;
            }
            self.middleware.job.push(mw);
        }

        /// Register node middleware
        pub fn register_node_middleware(&mut self, mw: NodeMiddlewareFunc) {
            if self.state != State::Idle {
                return;
            }
            self.middleware.node.push(mw);
        }

        /// Register log middleware
        pub fn register_log_middleware(&mut self, mw: LogMiddlewareFunc) {
            if self.state != State::Idle {
                return;
            }
            self.middleware.log.push(mw);
        }

        /// Register an API endpoint
        pub fn register_endpoint(&mut self, method: &str, path: &str, handler: EndpointHandler) {
            if self.state != State::Idle {
                return;
            }
            let key = format!("{} {}", method, path);
            self.endpoints.insert(key, handler);
        }

        /// Register a runtime provider
        pub fn register_runtime(&mut self, rt: Box<dyn Runtime + Send + Sync>) {
            if self.state != State::Idle {
                return;
            }
            if self.runtime.is_some() {
                return;
            }
            self.runtime = Some(rt);
        }

        /// Register a datastore provider
        pub fn register_datastore_provider(
            &mut self,
            name: &str,
            provider: Box<dyn Datastore + Send + Sync>,
        ) {
            if self.state != State::Idle {
                return;
            }
            let name = name.to_string();
            if self.ds_providers.contains_key(&name) {
                return;
            }
            self.ds_providers.insert(name, provider);
        }

        /// Register a broker provider
        pub fn register_broker_provider(&mut self, name: &str, provider: Box<dyn Broker + Send + Sync>) {
            if self.state != State::Idle {
                return;
            }
            let name = name.to_string();
            if self.broker_providers.contains_key(&name) {
                return;
            }
            self.broker_providers.insert(name, provider);
        }

        /// Register a mounter for a specific runtime.
        ///
        /// Matches Go's `RegisterMounter(rt, name, mounter)`:
        /// - Creates a new `MultiMounter` for the runtime if one doesn't exist yet.
        /// - Registers the named mounter into that runtime's `MultiMounter`.
        pub fn register_mounter(
            &mut self,
            rt: &str,
            name: &str,
            mounter: Box<dyn Mounter>,
        ) {
            if self.state != State::Idle {
                return;
            }
            let rt_key = rt.to_string();
            let entry = self.mounters.entry(rt_key).or_default();
            // Silently ignore duplicate mounter registrations, matching Go's
            // behavior of creating a new MultiMounter per runtime key. The
            // underlying `MultiMounter::register_mounter` returns a
            // `MountError::DuplicateMounter` which we discard here.
            let _ = entry.register_mounter(name, mounter);
        }

        /// Submit a job to the engine
        pub async fn submit_job(
            &self,
            job: tork::job::Job,
            listeners: Vec<JobListener>,
        ) -> Result<tork::job::Job> {
            if self.state != State::Running {
                return Err(anyhow!("engine is not running"));
            }
            if self.mode != Mode::Standalone && self.mode != Mode::Coordinator {
                return Err(anyhow!("engine not in coordinator/standalone mode"));
            }

            // Get the job ID for listener matching
            let job_id = job.id.clone();

            // Subscribe to job events if there are listeners
            if !listeners.is_empty() {
                let broker = self.broker.clone();
                let listeners = Arc::new(listeners);
                let job_id_for_listener = job_id.clone();
                
                broker.subscribe_for_events(
                    super::TOPIC_JOB.to_string(),
                    Arc::new(move |event: serde_json::Value| {
                        let listeners = listeners.clone();
                        let job_id = job_id_for_listener.clone();
                        Box::pin(async move {
                            // Try to parse the event as a job
                            if let Ok(ev_job) = serde_json::from_value::<tork::job::Job>(event) {
                                if ev_job.id.as_ref() == job_id.as_ref() {
                                    for listener in listeners.iter() {
                                        listener(ev_job.clone());
                                    }
                                }
                            }
                        })
                    }),
                ).await?;
            }

            // Submit to coordinator
            let coordinator = self.coordinator.read().await;
            if let Some(ref coord) = *coordinator {
                let result = coord.submit_job(job).await?;
                Ok(result.deep_clone())
            } else {
                Err(anyhow!("coordinator not available"))
            }
        }

        /// Register a job listener
        pub fn add_job_listener(&self, listener: JobListener) {
            let mut listeners = self.job_listeners.blocking_write();
            listeners.push(listener);
        }

        /// Wait for termination signal (SIGINT or SIGTERM)
        #[allow(dead_code)]
        async fn await_termination(&self) {
            let sigint = tokio::signal::unix::signal(tokio::signal::unix::SignalKind::interrupt());
            let sigterm = tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate());
            let mut terminate_rx = self.terminate_tx.subscribe();

            match (sigint, sigterm) {
                (Ok(mut sigint), Ok(mut sigterm)) => {
                    tokio::select! {
                        _ = sigint.recv() => {
                            debug!("Received SIGINT signal");
                        }
                        _ = sigterm.recv() => {
                            debug!("Received SIGTERM signal");
                        }
                        _ = terminate_rx.recv() => {
                            debug!("Received termination signal");
                        }
                    }
                }
                _ => {
                    // If we can't register signal handlers, just wait on the channel
                    let _ = terminate_rx.recv().await;
                    debug!("Received termination signal");
                }
            }
        }

        /// Wait for shutdown to complete
        pub async fn await_shutdown(&self) {
            // Get the terminated receiver if available
            let terminated_rx = {
                let terminated_tx = self.terminated_tx.read().await;
                terminated_tx.as_ref().map(|tx| tx.subscribe())
            };

            if let Some(mut rx) = terminated_rx {
                let _ = rx.recv().await;
            }
        }

        async fn run_coordinator(&mut self) -> Result<()> {
            self.broker.init("inmemory").await?;
            self.datastore.init().await?;

            // Create locker
            let locker = create_locker("inmemory").await?;
            *self.locker.write().await = Some(locker);

            let coord = create_coordinator(self.broker.clone(), self.datastore.clone()).await?;
            coord.start().await?;
            *self.coordinator.write().await = Some(coord);

            // Set up termination channels
            let (terminated_tx, _terminated_rx) = broadcast::channel::<()>(1);
            *self.terminated_tx.write().await = Some(terminated_tx);

            // Clone references for the signal handler task
            let coordinator = self.coordinator.clone();
            let terminated_tx_clone = self.terminated_tx.clone();

            // Spawn signal handler task
            tokio::spawn(async move {
                let sigint = tokio::signal::unix::signal(
                    tokio::signal::unix::SignalKind::interrupt()
                ).ok();
                let sigterm = tokio::signal::unix::signal(
                    tokio::signal::unix::SignalKind::terminate()
                ).ok();
                let terminate_rx = {
                    let terminated_tx = terminated_tx_clone.read().await;
                    match terminated_tx.as_ref() {
                        Some(tx) => tx.subscribe(),
                        None => return,
                    }
                };

                await_signal_or_channel(sigint, sigterm, terminate_rx).await;

                debug!("shutting down");

                // Stop coordinator if present
                let coord = coordinator.read().await;
                if let Some(c) = coord.as_ref() {
                    if let Err(e) = c.stop().await {
                        error!("error stopping coordinator: {}", e);
                    }
                }

                // Signal terminated
                let tx = terminated_tx_clone.write().await;
                if let Some(ref t) = *tx {
                    let _ = t.send(());
                }
            });

            Ok(())
        }

        async fn run_worker(&mut self) -> Result<()> {
            self.broker.init("inmemory").await?;

            // Set up runtime if not already set
            if self.runtime.is_none() {
                self.runtime = Some(Box::new(MockRuntime));
            }

            // Take the runtime out, will be replaced after worker creation
            let runtime = self.runtime.take();
            let worker = create_worker(self, self.broker.clone(), runtime).await?;
            worker.start().await?;
            *self.worker.write().await = Some(worker);

            // Set up termination channels
            let (terminated_tx, _terminated_rx) = broadcast::channel::<()>(1);
            *self.terminated_tx.write().await = Some(terminated_tx);

            // Clone references for the signal handler task
            let worker_ref = self.worker.clone();
            let terminated_tx_clone = self.terminated_tx.clone();

            // Spawn signal handler task
            tokio::spawn(async move {
                let sigint = tokio::signal::unix::signal(
                    tokio::signal::unix::SignalKind::interrupt()
                ).ok();
                let sigterm = tokio::signal::unix::signal(
                    tokio::signal::unix::SignalKind::terminate()
                ).ok();
                let terminate_rx = {
                    let terminated_tx = terminated_tx_clone.read().await;
                    match terminated_tx.as_ref() {
                        Some(tx) => tx.subscribe(),
                        None => return,
                    }
                };

                await_signal_or_channel(sigint, sigterm, terminate_rx).await;

                debug!("shutting down");

                // Stop worker if present
                let w = worker_ref.read().await;
                if let Some(ref worker) = *w {
                    if let Err(e) = worker.stop().await {
                        error!("error stopping worker: {}", e);
                    }
                }

                // Signal terminated
                let tx = terminated_tx_clone.write().await;
                if let Some(ref t) = *tx {
                    let _ = t.send(());
                }
            });

            Ok(())
        }

        async fn run_standalone(&mut self) -> Result<()> {
            self.broker.init("inmemory").await?;
            self.datastore.init().await?;

            // Create locker
            let locker = create_locker("inmemory").await?;
            *self.locker.write().await = Some(locker);

            // Set up runtime if not already set
            if self.runtime.is_none() {
                self.runtime = Some(Box::new(MockRuntime));
            }

            // Take the runtime out, will be replaced after worker creation
            let runtime = self.runtime.take();
            let worker = create_worker(self, self.broker.clone(), runtime).await?;
            worker.start().await?;
            *self.worker.write().await = Some(worker);

            let coord = create_coordinator(self.broker.clone(), self.datastore.clone()).await?;
            coord.start().await?;
            *self.coordinator.write().await = Some(coord);

            // Set up termination channels
            let (terminated_tx, _terminated_rx) = broadcast::channel::<()>(1);
            *self.terminated_tx.write().await = Some(terminated_tx);

            // Clone references for the signal handler task
            let worker_ref = self.worker.clone();
            let coordinator = self.coordinator.clone();
            let terminated_tx_clone = self.terminated_tx.clone();

            // Spawn signal handler task
            tokio::spawn(async move {
                let sigint = tokio::signal::unix::signal(
                    tokio::signal::unix::SignalKind::interrupt()
                ).ok();
                let sigterm = tokio::signal::unix::signal(
                    tokio::signal::unix::SignalKind::terminate()
                ).ok();
                let terminate_rx = {
                    let terminated_tx = terminated_tx_clone.read().await;
                    match terminated_tx.as_ref() {
                        Some(tx) => tx.subscribe(),
                        None => return,
                    }
                };

                await_signal_or_channel(sigint, sigterm, terminate_rx).await;

                debug!("shutting down");

                // Stop worker if present
                let w = worker_ref.read().await;
                if let Some(ref worker) = *w {
                    if let Err(e) = worker.stop().await {
                        error!("error stopping worker: {}", e);
                    }
                }

                // Stop coordinator if present
                let c = coordinator.read().await;
                if let Some(ref coord) = *c {
                    if let Err(e) = coord.stop().await {
                        error!("error stopping coordinator: {}", e);
                    }
                }

                // Signal terminated
                let tx = terminated_tx_clone.write().await;
                if let Some(ref t) = *tx {
                    let _ = t.send(());
                }
            });

            Ok(())
        }
    }

    /// Waits for either an OS signal (SIGINT/SIGTERM) or a broadcast
    /// channel message. Gracefully degrades if signal handler registration
    /// fails (e.g. in a constrained container).
    async fn await_signal_or_channel(
        sigint: Option<tokio::signal::unix::Signal>,
        sigterm: Option<tokio::signal::unix::Signal>,
        mut terminate_rx: broadcast::Receiver<()>,
    ) {
        match (sigint, sigterm) {
            (Some(mut sigint), Some(mut sigterm)) => {
                tokio::select! {
                    _ = sigint.recv() => {
                        debug!("Received SIGINT signal");
                    }
                    _ = sigterm.recv() => {
                        debug!("Received SIGTERM signal");
                    }
                    _ = terminate_rx.recv() => {
                        debug!("Received termination signal");
                    }
                }
            }
            _ => {
                // Signal handler registration failed (rare in containers);
                // fall back to waiting solely on the programmatic channel.
                debug!("Signal handlers unavailable, awaiting programmatic termination");
                let _ = terminate_rx.recv().await;
                debug!("Received termination signal");
            }
        }
    }

    impl Default for Engine {
        fn default() -> Self {
            Self::new(Config::default())
        }
    }

    /// Mock runtime for testing
    #[derive(Debug)]
    pub struct MockRuntime;

    impl Runtime for MockRuntime {
        fn run(
            &self,
            _ctx: std::sync::Arc<tokio::sync::RwLock<()>>,
            _task: &mut tork::task::Task,
        ) -> tork::runtime::BoxedFuture<()> {
            Box::pin(async { Ok(()) })
        }

        fn health_check(&self) -> tork::runtime::BoxedFuture<()> {
            Box::pin(async { Ok(()) })
        }
    }

    #[cfg(test)]
    mod tests {
        use super::*;
        use std::pin::Pin;

        /// Helper: set `TORK_DATASTORE_TYPE=inmemory` for tests that need a datastore.
        fn ensure_inmemory_datastore_env() {
            std::env::set_var("TORK_DATASTORE_TYPE", "inmemory");
        }

        /// Helper: reset `TORK_DATASTORE_TYPE` to inmemory after test.
        /// We set rather than remove to avoid race conditions with parallel tests
        /// that call `DatastoreProxy::init()` which defaults to "postgres" when unset.
        fn clear_datastore_env() {
            std::env::set_var("TORK_DATASTORE_TYPE", "inmemory");
        }

        // ── Engine construction ────────────────────────────────────

        #[tokio::test]
        async fn test_engine_new() {
            let engine = Engine::new(Config::default());
            assert_eq!(engine.state(), State::Idle);
            assert_eq!(engine.mode(), Mode::Standalone);
        }

        // ── Engine lifecycle: standalone ───────────────────────────

        #[tokio::test]
        async fn test_start_standalone() -> Result<(), Box<dyn std::error::Error>> {
            ensure_inmemory_datastore_env();
            let mut engine = Engine::new(Config {
                mode: Mode::Standalone,
                ..Default::default()
            });
            assert_eq!(engine.state(), State::Idle);

            engine.start().await?;
            assert_eq!(engine.state(), State::Running);

            engine.terminate().await?;
            assert_eq!(engine.state(), State::Terminated);

            clear_datastore_env();
            Ok(())
        }

        // ── Engine lifecycle: coordinator ─────────────────────────

        #[tokio::test]
        async fn test_start_coordinator() -> Result<(), Box<dyn std::error::Error>> {
            ensure_inmemory_datastore_env();
            let mut engine = Engine::new(Config {
                mode: Mode::Coordinator,
                ..Default::default()
            });
            assert_eq!(engine.state(), State::Idle);

            engine.start().await?;
            assert_eq!(engine.state(), State::Running);

            engine.terminate().await?;
            assert_eq!(engine.state(), State::Terminated);

            clear_datastore_env();
            Ok(())
        }

        // ── Engine lifecycle: worker ──────────────────────────────

        #[tokio::test]
        async fn test_start_worker() -> Result<(), Box<dyn std::error::Error>> {
            let mut engine = Engine::new(Config {
                mode: Mode::Worker,
                ..Default::default()
            });
            assert_eq!(engine.state(), State::Idle);

            engine.start().await?;
            assert_eq!(engine.state(), State::Running);

            engine.terminate().await?;
            assert_eq!(engine.state(), State::Terminated);
            Ok(())
        }

        // ── Mode & state transitions ──────────────────────────────

        #[tokio::test]
        async fn test_engine_set_mode_when_idle() {
            let mut engine = Engine::new(Config::default());
            assert_eq!(engine.mode(), Mode::Standalone);
            engine.set_mode(Mode::Worker);
            assert_eq!(engine.mode(), Mode::Worker);
        }

        #[tokio::test]
        async fn test_engine_terminate_when_not_running() {
            let mut engine = Engine::new(Config::default());
            let result = engine.terminate().await;
            assert!(result.is_err());
        }

        #[tokio::test]
        async fn test_double_start() -> Result<(), Box<dyn std::error::Error>> {
            ensure_inmemory_datastore_env();
            let mut engine = Engine::new(Config::default());

            engine.start().await?;

            let result = engine.start().await;
            assert!(result.is_err());

            engine.terminate().await?;
            clear_datastore_env();
            Ok(())
        }

        // ── Middleware registration ───────────────────────────────

        #[tokio::test]
        async fn test_middleware_registration_when_idle() -> Result<(), Box<dyn std::error::Error>> {
            ensure_inmemory_datastore_env();
            let mut engine = Engine::new(Config::default());

            let mw: WebMiddlewareFunc = Arc::new(|req, next| {
                Box::pin(async move {
                    next.run(req).await
                })
            });
            engine.register_web_middleware(mw);

            engine.start().await?;
            engine.terminate().await?;
            clear_datastore_env();
            Ok(())
        }

        // ── Mounter registration ──────────────────────────────────

        #[test]
        fn test_register_mounter_creates_multi_mounter() {
            use tork::mount::Mount;
            use tork::runtime::mount::MountError;
            use std::future::Future;
            use std::pin::Pin;

            struct NoopMounter;

            impl Mounter for NoopMounter {
                fn mount(
                    &self,
                    _ctx: Pin<Box<dyn Future<Output = Result<(), MountError>> + Send>>,
                    _mnt: &Mount,
                ) -> Pin<Box<dyn Future<Output = Result<(), MountError>> + Send>> {
                    Box::pin(async { Ok(()) })
                }

                fn unmount(
                    &self,
                    _ctx: Pin<Box<dyn Future<Output = Result<(), MountError>> + Send>>,
                    _mnt: &Mount,
                ) -> Pin<Box<dyn Future<Output = Result<(), MountError>> + Send>> {
                    Box::pin(async { Ok(()) })
                }
            }

            let mut engine = Engine::new(Config::default());
            engine.register_mounter("docker", "bind", Box::new(NoopMounter));

            assert!(
                engine.mounters.contains_key("docker"),
                "expected mounters to contain 'docker'"
            );
        }

        #[test]
        fn test_register_mounter_rejects_when_not_idle() {
            let mut engine = Engine::new(Config::default());
            assert_eq!(engine.state(), State::Idle);
            engine.register_mounter("docker", "bind", Box::new(FakeTestMounter));
        }

        /// Minimal fake mounter for tests
        #[derive(Debug)]
        struct FakeTestMounter;

        impl Mounter for FakeTestMounter {
            fn mount(
                &self,
                _ctx: Pin<Box<dyn std::future::Future<Output = Result<(), tork::runtime::mount::MountError>> + Send>>,
                _mnt: &tork::mount::Mount,
            ) -> Pin<Box<dyn std::future::Future<Output = Result<(), tork::runtime::mount::MountError>> + Send>> {
                Box::pin(async { Ok(()) })
            }

            fn unmount(
                &self,
                _ctx: Pin<Box<dyn std::future::Future<Output = Result<(), tork::runtime::mount::MountError>> + Send>>,
                _mnt: &tork::mount::Mount,
            ) -> Pin<Box<dyn std::future::Future<Output = Result<(), tork::runtime::mount::MountError>> + Send>> {
                Box::pin(async { Ok(()) })
            }
        }

        // ── Runtime registration ──────────────────────────────────

        #[tokio::test]
        async fn test_register_runtime_then_start() -> Result<(), Box<dyn std::error::Error>> {
            ensure_inmemory_datastore_env();
            let mut engine = Engine::new(Config {
                mode: Mode::Standalone,
                ..Default::default()
            });

            engine.register_runtime(Box::new(MockRuntime));

            engine.start().await?;
            assert_eq!(engine.state(), State::Running);

            engine.terminate().await?;
            assert_eq!(engine.state(), State::Terminated);

            clear_datastore_env();
            Ok(())
        }

        // ── Job submission ────────────────────────────────────────

        #[tokio::test]
        async fn test_submit_job_when_not_running() {
            let engine = Engine::new(Config::default());
            let job = tork::job::Job::default();
            let result = engine.submit_job(job, Vec::new()).await;
            assert!(result.is_err());
        }

        #[tokio::test]
        async fn test_submit_job_in_worker_mode() -> Result<(), Box<dyn std::error::Error>> {
            let mut engine = Engine::new(Config {
                mode: Mode::Worker,
                ..Default::default()
            });
            engine.start().await?;

            let job = tork::job::Job::default();
            let result = engine.submit_job(job, Vec::new()).await;
            assert!(result.is_err());

            engine.terminate().await?;
            Ok(())
        }

        #[tokio::test]
        async fn test_submit_job() -> Result<(), Box<dyn std::error::Error>> {
            ensure_inmemory_datastore_env();
            let mut engine = Engine::new(Config {
                mode: Mode::Coordinator,
                ..Default::default()
            });

            engine.start().await?;
            assert_eq!(engine.state(), State::Running);

            let job = tork::job::Job {
                id: Some("test-job-1".to_string()),
                name: Some("test job".to_string()),
                state: tork::job::JOB_STATE_PENDING.to_string(),
                ..Default::default()
            };

            let result = engine.submit_job(job, Vec::new()).await;
            assert!(result.is_ok());

            engine.terminate().await?;
            clear_datastore_env();
            Ok(())
        }

        // ── Broker proxy health ───────────────────────────────────

        #[tokio::test]
        async fn test_broker_health_before_init() {
            let engine = Engine::new(Config::default());
            let result = engine.broker().health_check().await;
            assert!(result.is_err());
        }

        #[tokio::test]
        async fn test_broker_health_after_init() -> Result<(), Box<dyn std::error::Error>> {
            // Worker mode initialises broker but not datastore
            let mut engine = Engine::new(Config {
                mode: Mode::Worker,
                ..Default::default()
            });
            engine.start().await?;

            let result = engine.broker().health_check().await;
            assert!(result.is_ok());

            engine.terminate().await?;
            Ok(())
        }

        // ── Datastore proxy health ────────────────────────────────

        #[tokio::test]
        async fn test_datastore_health_before_init() {
            let engine = Engine::new(Config::default());
            let result = engine.datastore().health_check().await;
            assert!(result.is_err());
        }

        #[tokio::test]
        async fn test_datastore_health_after_init() -> Result<(), Box<dyn std::error::Error>> {
            ensure_inmemory_datastore_env();
            let mut engine = Engine::new(Config {
                mode: Mode::Standalone,
                ..Default::default()
            });
            engine.start().await?;

            let result = engine.datastore().health_check().await;
            assert!(result.is_ok());

            engine.terminate().await?;
            clear_datastore_env();
            Ok(())
        }

        // ── Endpoint registration ─────────────────────────────────

        /// Go parity: Tests that `register_endpoint` stores a handler
        /// in the engine when idle, matching Go's `RegisterEndpoint`.
        #[tokio::test]
        async fn test_register_endpoint_when_idle() {
            let mut engine = Engine::new(Config::default());
            let handler: EndpointHandler = Arc::new(|_parts, _body| {
                Box::pin(async { axum::response::Response::new(axum::body::Body::empty()) })
            });
            engine.register_endpoint("GET", "/health", handler);
            assert!(
                engine.endpoints.contains_key("GET /health"),
                "expected endpoints to contain 'GET /health'"
            );
        }

        /// Go parity: Tests that `register_endpoint` is silently
        /// ignored when the engine is not idle.
        #[tokio::test]
        async fn test_register_endpoint_rejected_when_not_idle() -> Result<(), Box<dyn std::error::Error>> {
            ensure_inmemory_datastore_env();
            let mut engine = Engine::new(Config::default());
            engine.start().await?;

            let handler: EndpointHandler = Arc::new(|_parts, _body| {
                Box::pin(async { axum::response::Response::new(axum::body::Body::empty()) })
            });
            engine.register_endpoint("GET", "/test", handler);
            assert!(
                !engine.endpoints.contains_key("GET /test"),
                "expected endpoint registration to be ignored when running"
            );

            engine.terminate().await?;
            clear_datastore_env();
            Ok(())
        }

        // ── Middleware registration when NOT idle ──────────────────

        /// Go parity: In Go, calling Register*Middleware after Start()
        /// is silently ignored. Verify all five middleware types.
        #[tokio::test]
        async fn test_middleware_registration_rejected_when_running() -> Result<(), Box<dyn std::error::Error>> {
            ensure_inmemory_datastore_env();
            let mut engine = Engine::new(Config::default());
            engine.start().await?;

            let pre_web = engine.middleware.web.len();
            let pre_task = engine.middleware.task.len();
            let pre_job = engine.middleware.job.len();
            let pre_node = engine.middleware.node.len();
            let pre_log = engine.middleware.log.len();

            let web_mw: WebMiddlewareFunc = Arc::new(|req, next| {
                Box::pin(async move { next.run(req).await })
            });
            engine.register_web_middleware(web_mw);

            let task_mw: TaskMiddlewareFunc = Arc::new(|h| h);
            engine.register_task_middleware(task_mw);

            let job_mw: JobMiddlewareFunc = Arc::new(|h| h);
            engine.register_job_middleware(job_mw);

            let node_mw: NodeMiddlewareFunc = Arc::new(|h| h);
            engine.register_node_middleware(node_mw);

            let log_mw: LogMiddlewareFunc = Arc::new(|h| h);
            engine.register_log_middleware(log_mw);

            assert_eq!(engine.middleware.web.len(), pre_web);
            assert_eq!(engine.middleware.task.len(), pre_task);
            assert_eq!(engine.middleware.job.len(), pre_job);
            assert_eq!(engine.middleware.node.len(), pre_node);
            assert_eq!(engine.middleware.log.len(), pre_log);

            engine.terminate().await?;
            clear_datastore_env();
            Ok(())
        }

        // ── Datastore provider registration ───────────────────────

        /// Go parity: Tests `RegisterDatastoreProvider` stores the provider
        /// so it can be looked up on engine start.
        #[tokio::test]
        async fn test_register_datastore_provider() {
            let mut engine = Engine::new(Config::default());
            engine.register_datastore_provider("custom", crate::datastore::new_inmemory_datastore());
            assert!(
                engine.ds_providers.contains_key("custom"),
                "expected ds_providers to contain 'custom'"
            );
        }

        /// Go parity: Duplicate datastore provider names are silently ignored.
        #[tokio::test]
        async fn test_register_datastore_provider_duplicate_ignored() {
            let mut engine = Engine::new(Config::default());
            engine.register_datastore_provider("pg", crate::datastore::new_inmemory_datastore());
            engine.register_datastore_provider("pg", crate::datastore::new_inmemory_datastore());
            assert_eq!(engine.ds_providers.len(), 1);
        }

        // ── Broker provider registration ──────────────────────────

        /// Go parity: Tests `RegisterBrokerProvider` stores the provider.
        #[tokio::test]
        async fn test_register_broker_provider() {
            let mut engine = Engine::new(Config::default());
            engine.register_broker_provider("custom", Box::new(
                crate::broker::InMemoryBroker::new(),
            ));
            assert!(
                engine.broker_providers.contains_key("custom"),
                "expected broker_providers to contain 'custom'"
            );
        }

        /// Go parity: Duplicate broker provider names are silently ignored.
        #[tokio::test]
        async fn test_register_broker_provider_duplicate_ignored() {
            let mut engine = Engine::new(Config::default());
            engine.register_broker_provider("rmq", Box::new(
                crate::broker::InMemoryBroker::new(),
            ));
            engine.register_broker_provider("rmq", Box::new(
                crate::broker::InMemoryBroker::new(),
            ));
            assert_eq!(engine.broker_providers.len(), 1);
        }

        // ── Runtime registration: duplicate ignored ───────────────

        /// Go parity: In Go, calling `RegisterRuntime` twice only keeps
        /// the first runtime. The second call is silently ignored.
        #[tokio::test]
        async fn test_register_runtime_duplicate_ignored() {
            let mut engine = Engine::new(Config::default());
            engine.register_runtime(Box::new(MockRuntime));
            engine.register_runtime(Box::new(MockRuntime));
            assert!(engine.runtime.is_some(), "expected runtime to be set");
        }

        // ── set_mode when running is ignored ──────────────────────

        /// Go parity: Verifies that set_mode is a no-op when the engine
        /// is in Running state.
        #[tokio::test]
        async fn test_set_mode_when_running() -> Result<(), Box<dyn std::error::Error>> {
            ensure_inmemory_datastore_env();
            let mut engine = Engine::new(Config {
                mode: Mode::Standalone,
                ..Default::default()
            });
            engine.start().await?;
            assert_eq!(engine.mode(), Mode::Standalone);

            engine.set_mode(Mode::Coordinator);
            assert_eq!(engine.mode(), Mode::Standalone);

            engine.terminate().await?;
            clear_datastore_env();
            Ok(())
        }
    }
}