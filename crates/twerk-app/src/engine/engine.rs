//! Twerk Engine - Core Engine struct and all implementations

use super::broker::BrokerProxy;
use super::coordinator::create_coordinator;
use super::datastore::DatastoreProxy;
use super::engine_helpers::{ensure_config_loaded, resolve_broker_type, resolve_locker_type};
use super::locker::create_locker;
use super::signals::await_signal_or_channel;
use super::state::{Mode, State};
use super::types::{
    Config, EndpointHandler, JobListener, JobMiddlewareFunc, LogMiddlewareFunc, Middleware,
    NodeMiddlewareFunc, TaskMiddlewareFunc, WebMiddlewareFunc,
};
use super::worker::create_worker;
use anyhow::{anyhow, Result};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::broadcast;
use tokio::sync::RwLock;
use tracing::{debug, error};
use twerk_infrastructure::broker::{Broker, Broker as BrokerTrait};
use twerk_infrastructure::datastore::{Datastore, Datastore as DatastoreTrait};
use twerk_infrastructure::runtime::{Mounter, MultiMounter, Runtime};

/// Engine is the main orchestration engine
#[allow(dead_code)]
pub struct Engine {
    state: State,
    mode: Mode,
    broker: BrokerProxy,
    datastore: DatastoreProxy,
    runtime: Option<Box<dyn Runtime + Send + Sync>>,
    worker: Arc<RwLock<Option<Box<dyn super::worker::Worker + Send + Sync>>>>,
    coordinator: Arc<RwLock<Option<Box<dyn super::coordinator::Coordinator + Send + Sync>>>>,
    terminate_tx: broadcast::Sender<()>,
    terminate_rx: Arc<RwLock<broadcast::Receiver<()>>>,
    terminated_tx: Arc<RwLock<Option<broadcast::Sender<()>>>>,
    locker: Arc<RwLock<Option<Box<dyn super::locker::Locker + Send + Sync>>>>,
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
}

impl Default for Engine {
    fn default() -> Self {
        Self::new(Config::default())
    }
}

impl Engine {
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

        ensure_config_loaded();

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
        if let Err(e) = self.terminate_tx.send(()) {
            debug!("Termination broadcast failed (no listeners): {}", e);
        }

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

    /// Returns the broker as a trait object.
    pub fn broker(&self) -> &dyn BrokerTrait {
        &self.broker
    }

    /// Returns the datastore as a trait object.
    pub fn datastore(&self) -> &dyn DatastoreTrait {
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
}

impl Engine {
    async fn run_coordinator(&mut self) -> Result<()> {
        self.broker.init(&resolve_broker_type()).await?;
        self.datastore.init().await?;

        // Create locker — resolve type from env (locker.type → datastore.type → inmemory)
        let locker_type = resolve_locker_type();
        let locker = create_locker(&locker_type).await?;
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
            let sigint =
                match tokio::signal::unix::signal(tokio::signal::unix::SignalKind::interrupt()) {
                    Ok(s) => Some(s),
                    Err(e) => {
                        error!("Failed to register SIGINT: {e}");
                        None
                    }
                };
            let sigterm =
                match tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate()) {
                    Ok(s) => Some(s),
                    Err(e) => {
                        error!("Failed to register SIGTERM: {e}");
                        None
                    }
                };
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
                if let Err(e) = t.send(()) {
                    error!("failed to broadcast termination: {e}");
                }
            }
        });

        Ok(())
    }

    async fn run_worker(&mut self) -> Result<()> {
        self.broker.init(&resolve_broker_type()).await?;

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
            let sigint =
                match tokio::signal::unix::signal(tokio::signal::unix::SignalKind::interrupt()) {
                    Ok(s) => Some(s),
                    Err(e) => {
                        error!("Failed to register SIGINT: {e}");
                        None
                    }
                };
            let sigterm =
                match tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate()) {
                    Ok(s) => Some(s),
                    Err(e) => {
                        error!("Failed to register SIGTERM: {e}");
                        None
                    }
                };
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
                if let Err(e) = t.send(()) {
                    error!("failed to broadcast termination: {e}");
                }
            }
        });

        Ok(())
    }

    async fn run_standalone(&mut self) -> Result<()> {
        self.broker.init(&resolve_broker_type()).await?;
        self.datastore.init().await?;

        // Create locker — resolve type from env (locker.type → datastore.type → inmemory)
        let locker_type = resolve_locker_type();
        let locker = create_locker(&locker_type).await?;
        *self.locker.write().await = Some(locker);

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
            let sigint =
                match tokio::signal::unix::signal(tokio::signal::unix::SignalKind::interrupt()) {
                    Ok(s) => Some(s),
                    Err(e) => {
                        error!("Failed to register SIGINT: {e}");
                        None
                    }
                };
            let sigterm =
                match tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate()) {
                    Ok(s) => Some(s),
                    Err(e) => {
                        error!("Failed to register SIGTERM: {e}");
                        None
                    }
                };
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
                if let Err(e) = t.send(()) {
                    error!("failed to broadcast termination: {e}");
                }
            }
        });

        Ok(())
    }
}

impl Engine {
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
    pub fn register_broker_provider(
        &mut self,
        name: &str,
        provider: Box<dyn Broker + Send + Sync>,
    ) {
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
    pub fn register_mounter(&mut self, rt: &str, name: &str, mounter: Box<dyn Mounter>) {
        if self.state != State::Idle {
            return;
        }
        let rt_key = rt.to_string();
        let entry = self.mounters.entry(rt_key).or_default();
        // Silently ignore duplicate mounter registrations, matching Go's
        // behavior of creating a new MultiMounter per runtime key. The
        // underlying `MultiMounter::register_mounter` returns a
        // `MountError::DuplicateMounter` which we log if it's not expected.
        if let Err(e) = entry.register_mounter(name, mounter) {
            error!("failed to register mounter {name} for runtime {rt}: {e}");
        }
    }

    /// Submit a job to the engine
    pub async fn submit_job(
        &self,
        job: twerk_core::job::Job,
        listeners: Vec<JobListener>,
    ) -> Result<twerk_core::job::Job> {
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

            broker
                .subscribe_for_events(
                    super::TOPIC_JOB.to_string(),
                    Arc::new(move |event: serde_json::Value| {
                        let listeners = listeners.clone();
                        let job_id = job_id_for_listener.clone();
                        Box::pin(async move {
                            // Try to parse the event as a job
                            if let Ok(ev_job) =
                                serde_json::from_value::<twerk_core::job::Job>(event)
                            {
                                if ev_job.id.as_ref() == job_id.as_ref() {
                                    for listener in listeners.iter() {
                                        listener(ev_job.clone());
                                    }
                                }
                            }
                            Ok(())
                        })
                    }),
                )
                .await?;
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
}
