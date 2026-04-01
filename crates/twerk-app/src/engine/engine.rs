//! Twerk Engine - Core Engine struct definition

use super::broker::BrokerProxy;
use super::datastore::DatastoreProxy;
use super::state::{Mode, State};
use super::types::{Config, Middleware};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::broadcast;
use tokio::sync::Notify;
use tokio::sync::RwLock;
use twerk_infrastructure::broker::{Broker, Broker as BrokerTrait};
use twerk_infrastructure::datastore::{Datastore, Datastore as DatastoreTrait};
use twerk_infrastructure::runtime::{MultiMounter, Runtime};

/// Engine is the main orchestration engine
#[allow(dead_code)]
pub struct Engine {
    pub(crate) state: State,
    pub(crate) mode: Mode,
    pub(crate) engine_id: String,
    pub(crate) broker: BrokerProxy,
    pub(crate) datastore: DatastoreProxy,
    pub(crate) runtime: Option<Box<dyn Runtime + Send + Sync>>,
    pub(crate) worker: Arc<RwLock<Option<Box<dyn super::worker::Worker + Send + Sync>>>>,
    pub(crate) coordinator:
        Arc<RwLock<Option<Box<dyn super::coordinator::Coordinator + Send + Sync>>>>,
    /// Broadcast sender for termination request - we store the sender to create subscriptions
    pub(crate) terminate_tx: broadcast::Sender<()>,
    /// Broadcast sender used by signal handler tasks to subscribe for termination
    pub(crate) terminate_broadcaster: Arc<broadcast::Sender<()>>,
    /// Notify for termination completion - properly wakes late waiters
    pub(crate) terminated_notify: Arc<Notify>,
    pub(crate) locker: Arc<RwLock<Option<Box<dyn super::locker::Locker + Send + Sync>>>>,
    pub(crate) middleware: Middleware,
    pub(crate) endpoints: HashMap<String, super::types::EndpointHandler>,
    pub(crate) mounters: HashMap<String, MultiMounter>,
    pub(crate) ds_providers: HashMap<String, Box<dyn Datastore + Send + Sync>>,
    pub(crate) broker_providers: HashMap<String, Box<dyn Broker + Send + Sync>>,
    pub(crate) job_listeners: Arc<RwLock<Vec<super::types::JobListener>>>,
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
        let (terminate_tx, _terminate_rx) = broadcast::channel(1);
        // Generate UUID if engine_id is None or empty
        let engine_id = config
            .engine_id
            .filter(|id| !id.is_empty())
            .unwrap_or_else(twerk_core::uuid::new_uuid);
        Self {
            state: State::Idle,
            mode: config.mode,
            engine_id,
            broker: BrokerProxy::new(),
            datastore: DatastoreProxy::new(),
            runtime: None,
            worker: Arc::new(RwLock::new(None)),
            coordinator: Arc::new(RwLock::new(None)),
            terminate_tx: terminate_tx.clone(),
            terminate_broadcaster: Arc::new(terminate_tx),
            terminated_notify: Arc::new(Notify::new()),
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

    /// Returns the engine ID
    pub fn engine_id(&self) -> &str {
        &self.engine_id
    }

    /// Sets the engine mode (only when idle)
    pub fn set_mode(&mut self, mode: Mode) {
        if self.state == State::Idle {
            self.mode = mode;
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
