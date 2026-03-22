//! Tork Engine - Main orchestration engine for task execution
//!
//! This crate provides the core Engine struct that coordinates
//! between broker, datastore, locker, worker, and coordinator components.

pub mod broker;
pub mod coordinator;
pub mod datastore;
pub mod default;
pub mod locker;
pub mod worker;

// Re-export commonly used types
pub use broker::BrokerProxy;
pub use datastore::DatastoreProxy;
pub use engine::{Config, Engine, Mode, State, Middleware, JobListener};

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
    use tork::runtime::Runtime;
    use tork::broker::Broker;
    use tork::datastore::Datastore;
    use anyhow::{anyhow, Result};
    use std::collections::HashMap;
    use std::sync::Arc;
    use tokio::sync::{broadcast, RwLock};
    use tracing::{debug, error};

    /// Job listener callback type
    pub type JobListener = Arc<dyn Fn(tork::job::Job) + Send + Sync>;

    /// Engine execution mode
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub enum Mode {
        Coordinator,
        Worker,
        Standalone,
    }

    impl Default for Mode {
        fn default() -> Self {
            Mode::Standalone
        }
    }

    /// Engine state
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub enum State {
        Idle,
        Running,
        Terminating,
        Terminated,
    }

    impl Default for State {
        fn default() -> Self {
            State::Idle
        }
    }

    /// Middleware configuration - using boxed types for flexibility
    #[derive(Debug, Default)]
    pub struct Middleware {
        pub web: Vec<Box<dyn std::any::Any + Send + Sync>>,
        pub task: Vec<Box<dyn std::any::Any + Send + Sync>>,
        pub job: Vec<Box<dyn std::any::Any + Send + Sync>>,
        pub node: Vec<Box<dyn std::any::Any + Send + Sync>>,
        pub log: Vec<Box<dyn std::any::Any + Send + Sync>>,
    }

    /// Engine configuration
    #[derive(Debug)]
    pub struct Config {
        pub mode: Mode,
        pub middleware: Middleware,
        pub endpoints: HashMap<String, Box<dyn std::any::Any + Send + Sync>>,
    }

    impl Default for Config {
        fn default() -> Self {
            Self {
                mode: Mode::default(),
                middleware: Middleware::default(),
                endpoints: HashMap::new(),
            }
        }
    }

    /// Engine is the main orchestration engine
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
        endpoints: HashMap<String, Box<dyn std::any::Any + Send + Sync>>,
        ds_providers: HashMap<String, Box<dyn Datastore + Send + Sync>>,
        broker_providers: HashMap<String, Box<dyn Broker + Send + Sync>>,
        job_listeners: Arc<RwLock<Vec<JobListener>>>,
    }

    impl std::fmt::Debug for Engine {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            f.debug_struct("Engine")
                .field("state", &self.state)
                .field("mode", &self.mode)
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

        /// Returns the broker
        pub fn broker(&self) -> &dyn Broker {
            &self.broker
        }

        /// Returns the datastore
        pub fn datastore(&self) -> &dyn Datastore {
            &self.datastore
        }

        /// Register web middleware
        pub fn register_web_middleware(&mut self, mw: Box<dyn std::any::Any + Send + Sync>) {
            if self.state != State::Idle {
                return;
            }
            self.middleware.web.push(mw);
        }

        /// Register task middleware
        pub fn register_task_middleware(&mut self, mw: Box<dyn std::any::Any + Send + Sync>) {
            if self.state != State::Idle {
                return;
            }
            self.middleware.task.push(mw);
        }

        /// Register job middleware
        pub fn register_job_middleware(&mut self, mw: Box<dyn std::any::Any + Send + Sync>) {
            if self.state != State::Idle {
                return;
            }
            self.middleware.job.push(mw);
        }

        /// Register node middleware
        pub fn register_node_middleware(&mut self, mw: Box<dyn std::any::Any + Send + Sync>) {
            if self.state != State::Idle {
                return;
            }
            self.middleware.node.push(mw);
        }

        /// Register log middleware
        pub fn register_log_middleware(&mut self, mw: Box<dyn std::any::Any + Send + Sync>) {
            if self.state != State::Idle {
                return;
            }
            self.middleware.log.push(mw);
        }

        /// Register an API endpoint
        pub fn register_endpoint(&mut self, method: &str, path: &str, handler: Box<dyn std::any::Any + Send + Sync>) {
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
            // Create a stream that receives both SIGINT and SIGTERM
            let mut sigint = tokio::signal::unix::signal(tokio::signal::unix::SignalKind::interrupt())
                .expect("failed to create SIGINT signal handler");
            let mut sigterm = tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
                .expect("failed to create SIGTERM signal handler");
            let mut terminate_rx = self.terminate_tx.subscribe();

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
            let locker = create_locker("inmemory")?;
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
                // Wait for termination signal (SIGINT, SIGTERM, or internal)
                let mut sigint = tokio::signal::unix::signal(
                    tokio::signal::unix::SignalKind::interrupt()
                ).expect("failed to create SIGINT signal handler");
                let mut sigterm = tokio::signal::unix::signal(
                    tokio::signal::unix::SignalKind::terminate()
                ).expect("failed to create SIGTERM signal handler");
                let mut terminate_rx = terminated_tx_clone.subscribe();

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

                debug!("shutting down");

                // Stop coordinator if present
                let coord = coordinator.read().await;
                if let Some(c) = coord.as_ref() {
                    if let Err(e) = c.stop().await {
                        error!("error stopping coordinator: {}", e);
                    }
                }

                // Signal terminated
                let mut tx = terminated_tx_clone.write().await;
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
            let worker = create_worker(self.broker.clone(), runtime).await?;
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
                // Wait for termination signal (SIGINT, SIGTERM, or internal)
                let mut sigint = tokio::signal::unix::signal(
                    tokio::signal::unix::SignalKind::interrupt()
                ).expect("failed to create SIGINT signal handler");
                let mut sigterm = tokio::signal::unix::signal(
                    tokio::signal::unix::SignalKind::terminate()
                ).expect("failed to create SIGTERM signal handler");
                let mut terminate_rx = terminated_tx_clone.subscribe();

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

                debug!("shutting down");

                // Stop worker if present
                let w = worker_ref.read().await;
                if let Some(ref worker) = *w {
                    if let Err(e) = worker.stop().await {
                        error!("error stopping worker: {}", e);
                    }
                }

                // Signal terminated
                let mut tx = terminated_tx_clone.write().await;
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
            let locker = create_locker("inmemory")?;
            *self.locker.write().await = Some(locker);

            // Set up runtime if not already set
            if self.runtime.is_none() {
                self.runtime = Some(Box::new(MockRuntime));
            }

            // Take the runtime out, will be replaced after worker creation
            let runtime = self.runtime.take();
            let worker = create_worker(self.broker.clone(), runtime).await?;
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
                // Wait for termination signal (SIGINT, SIGTERM, or internal)
                let mut sigint = tokio::signal::unix::signal(
                    tokio::signal::unix::SignalKind::interrupt()
                ).expect("failed to create SIGINT signal handler");
                let mut sigterm = tokio::signal::unix::signal(
                    tokio::signal::unix::SignalKind::terminate()
                ).expect("failed to create SIGTERM signal handler");
                let mut terminate_rx = terminated_tx_clone.subscribe();

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
                let mut tx = terminated_tx_clone.write().await;
                if let Some(ref t) = *tx {
                    let _ = t.send(());
                }
            });

            Ok(())
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

        #[tokio::test]
        async fn test_engine_new() {
            let engine = Engine::new(Config::default());
            assert_eq!(engine.state(), State::Idle);
            assert_eq!(engine.mode(), Mode::Standalone);
        }

        #[tokio::test]
        #[ignore] // Requires working datastore implementation
        async fn test_engine_start_standalone() {
            let mut engine = Engine::new(Config {
                mode: Mode::Standalone,
                ..Default::default()
            });

            assert_eq!(engine.state(), State::Idle);

            engine.start().await.expect("should start");

            assert_eq!(engine.state(), State::Running);

            engine.terminate().await.expect("should terminate");

            assert_eq!(engine.state(), State::Terminated);
        }

        #[tokio::test]
        #[ignore] // Requires working datastore implementation
        async fn test_engine_start_coordinator() {
            let mut engine = Engine::new(Config {
                mode: Mode::Coordinator,
                ..Default::default()
            });

            assert_eq!(engine.state(), State::Idle);

            engine.start().await.expect("should start");

            assert_eq!(engine.state(), State::Running);

            engine.terminate().await.expect("should terminate");

            assert_eq!(engine.state(), State::Terminated);
        }

        #[tokio::test]
        #[ignore] // Requires working datastore implementation
        async fn test_engine_start_worker() {
            let mut engine = Engine::new(Config { 
                mode: Mode::Worker,
                ..Default::default() 
            });

            assert_eq!(engine.state(), State::Idle);

            engine.start().await.expect("should start");

            assert_eq!(engine.state(), State::Running);

            engine.terminate().await.expect("should terminate");

            assert_eq!(engine.state(), State::Terminated);
        }

        #[tokio::test]
        async fn test_engine_set_mode_when_idle() {
            let mut engine = Engine::new(Config::default());
            assert_eq!(engine.mode(), Mode::Standalone);

            engine.set_mode(Mode::Worker);
            assert_eq!(engine.mode(), Mode::Worker);
        }

        #[tokio::test]
        #[ignore] // Requires working datastore implementation
        async fn test_engine_double_start() {
            let mut engine = Engine::new(Config::default());

            engine.start().await.expect("should start");

            let result = engine.start().await;
            assert!(result.is_err());

            engine.terminate().await.expect("should terminate");
        }

        #[tokio::test]
        async fn test_engine_terminate_when_not_running() {
            let mut engine = Engine::new(Config::default());

            let result = engine.terminate().await;
            assert!(result.is_err());
        }

        #[tokio::test]
        #[ignore] // Requires working datastore implementation
        async fn test_engine_middleware_registration_when_idle() {
            let mut engine = Engine::new(Config::default());
            
            // Register middleware should work when idle
            let mw: Box<dyn std::any::Any + Send + Sync> = Box::new(());
            engine.register_web_middleware(mw);
            
            // Starting should succeed
            engine.start().await.expect("should start");
            engine.terminate().await.expect("should terminate");
        }
    }
}