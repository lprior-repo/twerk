use crate::engine::BrokerProxy;
use anyhow::Result;
use dashmap::DashMap;
use std::collections::HashMap;
use std::sync::Arc;
use sysinfo::System;
use tokio::sync::broadcast;
use tracing::{debug, info, instrument, warn};
use twerk_core::id::{NodeId, TaskId};
use twerk_core::node::{Node, NodeStatus};
use twerk_core::task::{Task, TaskState};
use twerk_infrastructure::broker::{is_worker_queue, Broker};
use twerk_infrastructure::runtime::Runtime as RuntimeTrait;

// ── Typed error for worker execution ───────────────────────────────

#[derive(Debug, thiserror::Error)]
enum WorkerExecError {
    #[error("task ID required for execution")]
    TaskIdRequired,
}

pub use twerk_infrastructure::BoxedFuture;

pub mod docker;
pub mod mounter;
pub mod podman;
pub mod runtime_adapter;
pub mod shell;

pub trait Worker: Send + Sync {
    fn start(&self) -> BoxedFuture<()>;
    fn stop(&self) -> BoxedFuture<()>;
}

pub use Worker as WorkerTrait;

#[derive(Debug, Clone, Default)]
pub struct Limits {
    pub cpus: String,
    pub memory: String,
    pub timeout: String,
}

pub const DEFAULT_CPUS_LIMIT: &str = "1";
pub const DEFAULT_MEMORY_LIMIT: &str = "512m";
pub const DEFAULT_TIMEOUT: &str = "5m";

pub struct DefaultWorker {
    id: String,
    name: String,
    broker: BrokerProxy,
    runtime: Arc<dyn RuntimeTrait + Send + Sync>,
    queues: HashMap<String, i32>,
    #[allow(dead_code)]
    limits: Limits,
    terminate_tx: broadcast::Sender<()>,
    active_tasks: Arc<DashMap<TaskId, Arc<Task>>>,
}

impl DefaultWorker {
    pub fn new(
        id: String,
        name: String,
        broker: BrokerProxy,
        runtime: Arc<dyn RuntimeTrait + Send + Sync>,
        queues: HashMap<String, i32>,
        limits: Limits,
    ) -> Self {
        let (terminate_tx, _) = broadcast::channel(1);
        Self {
            id,
            name,
            broker,
            runtime,
            queues,
            limits,
            terminate_tx,
            active_tasks: Arc::new(DashMap::new()),
        }
    }
}

// ---------------------------------------------------------------------------
// Worker trait implementation — start delegates to focused helper functions
// ---------------------------------------------------------------------------

impl Worker for DefaultWorker {
    fn start(&self) -> BoxedFuture<()> {
        let (id, name, broker, runtime, queues, terminate_tx, active_tasks) = (
            self.id.clone(),
            self.name.clone(),
            self.broker.clone(),
            self.runtime.clone(),
            self.queues.clone(),
            self.terminate_tx.clone(),
            self.active_tasks.clone(),
        );
        Box::pin(async move {
            info!("Worker {} ({}) starting", name, id);
            let heartbeat_queue = primary_worker_queue(&queues);
            spawn_heartbeat_loop(
                broker.clone(),
                id.clone(),
                name.clone(),
                heartbeat_queue,
                runtime.clone(),
                terminate_tx.clone(),
            );
            spawn_queue_subscribers(&broker, &runtime, &queues, &terminate_tx, &active_tasks);
            spawn_cancel_listener(broker, id, runtime, active_tasks, terminate_tx);
            Ok(())
        })
    }

    fn stop(&self) -> BoxedFuture<()> {
        let (terminate_tx, active_tasks) = (self.terminate_tx.clone(), self.active_tasks.clone());
        Box::pin(async move {
            let _ = terminate_tx.send(());
            let start = std::time::Instant::now();
            while !active_tasks.is_empty() && start.elapsed() < std::time::Duration::from_secs(10) {
                tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
            }
            if !active_tasks.is_empty() {
                warn!("Worker stopping with {} tasks active", active_tasks.len());
            }
            Ok(())
        })
    }
}

// ---------------------------------------------------------------------------
// Helper 1: Heartbeat — periodic status publishing
// ---------------------------------------------------------------------------

fn spawn_heartbeat_loop(
    broker: BrokerProxy,
    id: String,
    name: String,
    queue: String,
    runtime: Arc<dyn RuntimeTrait + Send + Sync>,
    terminate_tx: broadcast::Sender<()>,
) {
    let mut terminate_rx = terminate_tx.subscribe();
    let identity = HeartbeatIdentity::new(id, name, queue);
    tokio::spawn(async move {
        let mut sys = System::new_all();
        loop {
            send_heartbeat(&broker, &identity, &mut sys, runtime.clone()).await;
            tokio::select! {
                _ = terminate_rx.recv() => break,
                _ = tokio::time::sleep(tokio::time::Duration::from_secs(30)) => {}
            }
        }
    });
}

// ---------------------------------------------------------------------------
// Helper 2: Queue subscribers — spawn one task per concurrency slot
// ---------------------------------------------------------------------------

fn spawn_queue_subscribers(
    broker: &BrokerProxy,
    runtime: &Arc<dyn RuntimeTrait + Send + Sync>,
    queues: &HashMap<String, i32>,
    terminate_tx: &broadcast::Sender<()>,
    active_tasks: &Arc<DashMap<TaskId, Arc<Task>>>,
) {
    for (qname, concurrency) in queues {
        if !is_worker_queue(qname) {
            continue;
        }
        for _ in 0..*concurrency {
            spawn_queue_worker(
                broker.clone(),
                runtime.clone(),
                qname.clone(),
                terminate_tx.subscribe(),
                active_tasks.clone(),
            );
        }
    }
}

fn primary_worker_queue(queues: &HashMap<String, i32>) -> String {
    match queues.keys().filter(|qname| is_worker_queue(qname)).min() {
        Some(queue) => queue.clone(),
        None => "default".to_string(),
    }
}

fn spawn_queue_worker(
    q_broker: BrokerProxy,
    q_runtime: Arc<dyn RuntimeTrait + Send + Sync>,
    q_name: String,
    mut q_terminate_rx: broadcast::Receiver<()>,
    q_active_tasks: Arc<DashMap<TaskId, Arc<Task>>>,
) {
    tokio::spawn(async move {
        let qb = q_broker.clone();
        let handler: twerk_infrastructure::broker::TaskHandler =
            Arc::new(move |task: Arc<Task>| {
                let (b, r, a) = (qb.clone(), q_runtime.clone(), q_active_tasks.clone());
                Box::pin(async move { execute_task(task, r, b, a).await })
            });
        if let Err(e) = q_broker.subscribe_for_tasks(q_name, handler).await {
            warn!(error = %e, "failed to subscribe for tasks on queue");
        }
        let _ = q_terminate_rx.recv().await;
    });
}

// ---------------------------------------------------------------------------
// Helper 3: Cancel listener — reacts to task cancellation requests
// ---------------------------------------------------------------------------

fn spawn_cancel_listener(
    broker: BrokerProxy,
    id: String,
    runtime: Arc<dyn RuntimeTrait + Send + Sync>,
    active_tasks: Arc<DashMap<TaskId, Arc<Task>>>,
    terminate_tx: broadcast::Sender<()>,
) {
    let (cancel_q, mut terminate_rx) = (format!("cancel.{}", id), terminate_tx.subscribe());
    tokio::spawn(async move {
        let handler: twerk_infrastructure::broker::TaskHandler =
            Arc::new(move |task: Arc<Task>| {
                let (r, a) = (runtime.clone(), active_tasks.clone());
                Box::pin(async move {
                    if let Some(tid) = &task.id {
                        if let Some((_, t)) = a.remove(tid) {
                            debug!("Cancelling task {}", tid);
                            let _ = r.stop(&t).await;
                        }
                    }
                    Ok(())
                })
            });
        if let Err(e) = broker.subscribe_for_tasks(cancel_q, handler).await {
            warn!(error = %e, "failed to subscribe for cancel requests");
        }
        let _ = terminate_rx.recv().await;
    });
}

// ---------------------------------------------------------------------------
// Heartbeat & task execution internals
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
struct HeartbeatIdentity {
    id: String,
    name: String,
    queue: String,
    hostname: String,
    version: String,
}

impl HeartbeatIdentity {
    fn new(id: String, name: String, queue: String) -> Self {
        Self {
            id,
            name,
            queue,
            hostname: hostname::get().map_or_else(
                |_| "unknown".to_string(),
                |hostname| hostname.to_string_lossy().into_owned(),
            ),
            version: env!("CARGO_PKG_VERSION").to_string(),
        }
    }
}

async fn send_heartbeat(
    broker: &BrokerProxy,
    identity: &HeartbeatIdentity,
    sys: &mut System,
    runtime: Arc<dyn RuntimeTrait + Send + Sync>,
) {
    sys.refresh_cpu_all();
    let status = match runtime.health_check().await {
        Ok(()) => NodeStatus::UP,
        Err(e) => {
            tracing::warn!("Runtime health check failed: {}", e);
            NodeStatus::DOWN
        }
    };
    let node = Node {
        id: Some(NodeId::from(identity.id.as_str())),
        name: Some(identity.name.clone()),
        hostname: Some(identity.hostname.clone()),
        cpu_percent: Some(sys.global_cpu_usage() as f64),
        status: Some(status),
        queue: Some(identity.queue.clone()),
        version: Some(identity.version.clone()),
        last_heartbeat_at: Some(time::OffsetDateTime::now_utc()),
        ..Default::default()
    };
    if let Err(e) = broker.publish_heartbeat(node).await {
        warn!(error = %e, "failed to publish heartbeat");
    }
}

#[instrument(skip_all, fields(task_id = %task.id.as_ref().map_or("unknown", |id| id.as_str())))]
async fn execute_task(
    task: Arc<Task>,
    runtime: Arc<dyn RuntimeTrait + Send + Sync>,
    broker: BrokerProxy,
    active_tasks: Arc<DashMap<TaskId, Arc<Task>>>,
) -> Result<()> {
    let mut t = (*task).clone();
    let tid = t.id.clone().ok_or(WorkerExecError::TaskIdRequired)?;
    active_tasks.insert(tid.clone(), task.clone());
    t.state = TaskState::Running;
    t.started_at = Some(time::OffsetDateTime::now_utc());

    broker.publish_task_progress(&t).await?;

    match runtime.run(&t).await {
        Ok(()) => {
            t.state = TaskState::Completed;
            t.completed_at = Some(time::OffsetDateTime::now_utc());
        }
        Err(e) => {
            t.state = TaskState::Failed;
            t.failed_at = Some(time::OffsetDateTime::now_utc());
            t.error = Some(e.to_string());
        }
    }
    active_tasks.remove(&tid);

    broker.publish_task_progress(&t).await
}

// ---------------------------------------------------------------------------
// Helper 4: Hostenv middleware registration
// ---------------------------------------------------------------------------

pub fn create_hostenv_middleware(vars: &[String]) -> Option<crate::engine::TaskMiddlewareFunc> {
    if vars.is_empty() {
        return None;
    }
    let var_map: HashMap<String, String> = vars
        .iter()
        .filter_map(|v| {
            let p: Vec<&str> = v.split(':').collect();
            match p.len() {
                1 if !p[0].is_empty() => Some((p[0].to_string(), p[0].to_string())),
                2 if !p[0].is_empty() && !p[1].is_empty() => {
                    Some((p[0].to_string(), p[1].to_string()))
                }
                _ => {
                    warn!("invalid env spec: {}", v);
                    None
                }
            }
        })
        .collect();
    if var_map.is_empty() {
        return None;
    }
    Some(Arc::new(
        move |next: crate::engine::TaskHandlerFunc| -> crate::engine::TaskHandlerFunc {
            let vm = var_map.clone();
            Arc::new(move |_ctx, et, task| {
                if et == crate::engine::TaskEventType::StateChange
                    && task.state == TaskState::Running
                {
                    if task.env.is_none() {
                        task.env = Some(HashMap::new());
                    }
                    if let Some(ref mut e) = task.env {
                        for (hn, tn) in &vm {
                            if let Ok(v) = std::env::var(hn) {
                                e.insert(tn.clone(), v);
                            }
                        }
                    }
                }
                next(_ctx, et, task)
            })
        },
    ))
}

fn register_hostenv_middleware(engine: &mut crate::engine::Engine, hostenv_vars: &[String]) {
    if let Some(middleware) = create_hostenv_middleware(hostenv_vars) {
        engine.register_task_middleware(middleware);
    }
}

// ---------------------------------------------------------------------------
// Helper 5: Worker config resolution from environment
// ---------------------------------------------------------------------------

/// Reads worker limits from the centralized config system.
///
/// This function uses `twerk_common::conf::worker_limits()` which:
/// 1. First checks environment variables with `TWERK_` prefix (via `var_with_twerk_prefix`)
/// 2. Falls back to config file values
/// 3. Falls back to hardcoded defaults if both are empty
pub fn read_limits() -> Limits {
    let config_limits = twerk_common::conf::worker_limits();
    Limits {
        cpus: if config_limits.cpus.is_empty() {
            DEFAULT_CPUS_LIMIT.to_string()
        } else {
            config_limits.cpus
        },
        memory: if config_limits.memory.is_empty() {
            DEFAULT_MEMORY_LIMIT.to_string()
        } else {
            config_limits.memory
        },
        timeout: if config_limits.timeout.is_empty() {
            DEFAULT_TIMEOUT.to_string()
        } else {
            config_limits.timeout
        },
    }
}

fn resolve_worker_config() -> (String, String, HashMap<String, i32>) {
    let id = std::env::var("TWERK_WORKER_ID").unwrap_or_else(|_| twerk_core::uuid::new_uuid());
    let name = std::env::var("TWERK_WORKER_NAME").unwrap_or_else(|_| "Worker".to_string());
    let queues = std::env::var("TWERK_WORKER_QUEUES").ok().map_or_else(
        || HashMap::from([("default".to_string(), 1)]),
        |s| {
            s.split(',')
                .filter_map(|q| {
                    let p: Vec<&str> = q.split(':').collect();
                    if p.len() == 2 {
                        p[1].trim()
                            .parse::<i32>()
                            .ok()
                            .map(|v| (p[0].trim().to_string(), v))
                    } else {
                        None
                    }
                })
                .collect()
        },
    );
    (id, name, queues)
}

// ---------------------------------------------------------------------------
// Worker factory — assembles the DefaultWorker from resolved config
// ---------------------------------------------------------------------------

pub async fn create_worker(
    engine: &mut crate::engine::Engine,
    broker: BrokerProxy,
    runtime: Option<Box<dyn RuntimeTrait + Send + Sync>>,
) -> Result<Box<dyn Worker + Send + Sync>> {
    use crate::engine::worker::runtime_adapter::{create_runtime_from_config, read_runtime_config};
    let config = read_runtime_config();
    let runtime_broker: Arc<dyn Broker + Send + Sync> = Arc::new(broker.clone());
    let rt: Arc<dyn RuntimeTrait + Send + Sync> = match runtime {
        Some(r) => Arc::from(r),
        None => Arc::from(create_runtime_from_config(&config, runtime_broker).await?),
    };
    rt.health_check().await?;
    register_hostenv_middleware(engine, &config.hostenv_vars);
    let (id, name, queues) = resolve_worker_config();
    Ok(Box::new(DefaultWorker::new(
        id,
        name,
        broker,
        rt,
        queues,
        read_limits(),
    )))
}

#[cfg(test)]
mod tests {
    use super::*;
    use async_trait::async_trait;
    use std::sync::Arc;
    use tokio::sync::RwLock;
    use twerk_core::node::Node;
    use twerk_core::task::Task;
    use twerk_infrastructure::broker::{
        Broker, EventHandler, HeartbeatHandler, JobHandler, TaskHandler, TaskLogPartHandler,
        TaskProgressHandler,
    };
    use twerk_infrastructure::runtime::{BoxedFuture, Runtime as RuntimeTrait, ShutdownResult};

    #[derive(Debug, Clone)]
    struct FakeRuntime {
        health_check_ok: bool,
    }

    impl FakeRuntime {
        fn new(health_check_ok: bool) -> Self {
            Self { health_check_ok }
        }
    }

    impl RuntimeTrait for FakeRuntime {
        fn run(&self, _task: &Task) -> BoxedFuture<()> {
            Box::pin(async { Ok(()) })
        }

        fn stop(&self, _task: &Task) -> BoxedFuture<ShutdownResult<std::process::ExitCode>> {
            Box::pin(async { Ok(Ok(std::process::ExitCode::SUCCESS)) })
        }

        fn health_check(&self) -> BoxedFuture<()> {
            if self.health_check_ok {
                Box::pin(async { Ok(()) })
            } else {
                Box::pin(async { Err(anyhow::anyhow!("runtime unavailable")) })
            }
        }
    }

    #[derive(Debug, Clone, Default)]
    struct SpyBroker {
        heartbeats: Arc<RwLock<Vec<Node>>>,
    }

    impl SpyBroker {
        fn new() -> Self {
            Self {
                heartbeats: Arc::new(RwLock::new(Vec::new())),
            }
        }

        async fn get_heartbeats(&self) -> Vec<Node> {
            self.heartbeats.read().await.clone()
        }
    }

    #[async_trait]
    impl Broker for SpyBroker {
        fn publish_task(&self, _qname: String, _task: &Task) -> BoxedFuture<()> {
            Box::pin(async { Ok(()) })
        }
        fn subscribe_for_tasks(&self, _qname: String, _handler: TaskHandler) -> BoxedFuture<()> {
            Box::pin(async { Ok(()) })
        }
        fn publish_task_progress(&self, _task: &Task) -> BoxedFuture<()> {
            Box::pin(async { Ok(()) })
        }
        fn subscribe_for_task_progress(&self, _handler: TaskProgressHandler) -> BoxedFuture<()> {
            Box::pin(async { Ok(()) })
        }
        fn publish_heartbeat(&self, node: Node) -> BoxedFuture<()> {
            let heartbeats = self.heartbeats.clone();
            Box::pin(async move {
                heartbeats.write().await.push(node);
                Ok(())
            })
        }
        fn subscribe_for_heartbeats(&self, _handler: HeartbeatHandler) -> BoxedFuture<()> {
            Box::pin(async { Ok(()) })
        }
        fn publish_job(&self, _job: &twerk_core::job::Job) -> BoxedFuture<()> {
            Box::pin(async { Ok(()) })
        }
        fn subscribe_for_jobs(&self, _handler: JobHandler) -> BoxedFuture<()> {
            Box::pin(async { Ok(()) })
        }
        fn publish_event(&self, _topic: String, _event: serde_json::Value) -> BoxedFuture<()> {
            Box::pin(async { Ok(()) })
        }
        fn subscribe_for_events(
            &self,
            _pattern: String,
            _handler: EventHandler,
        ) -> BoxedFuture<()> {
            Box::pin(async { Ok(()) })
        }
        fn subscribe(
            &self,
            _pattern: String,
        ) -> BoxedFuture<tokio::sync::broadcast::Receiver<twerk_core::job::JobEvent>> {
            let (tx, rx) = tokio::sync::broadcast::channel(256);
            drop(tx);
            Box::pin(async { Ok(rx) })
        }
        fn publish_task_log_part(&self, _part: &twerk_core::task::TaskLogPart) -> BoxedFuture<()> {
            Box::pin(async { Ok(()) })
        }
        fn subscribe_for_task_log_part(&self, _handler: TaskLogPartHandler) -> BoxedFuture<()> {
            Box::pin(async { Ok(()) })
        }
        fn health_check(&self) -> BoxedFuture<()> {
            Box::pin(async { Ok(()) })
        }
        fn queues(
            &self,
        ) -> twerk_infrastructure::broker::BoxedFuture<Vec<twerk_infrastructure::broker::QueueInfo>>
        {
            Box::pin(async { Ok(Vec::new()) })
        }
        fn queue_info(
            &self,
            _qname: String,
        ) -> twerk_infrastructure::broker::BoxedFuture<twerk_infrastructure::broker::QueueInfo>
        {
            Box::pin(async {
                Ok(twerk_infrastructure::broker::QueueInfo {
                    name: _qname,
                    size: 0,
                    subscribers: 0,
                    unacked: 0,
                })
            })
        }
        fn delete_queue(&self, _qname: String) -> twerk_infrastructure::broker::BoxedFuture<()> {
            Box::pin(async { Ok(()) })
        }
        fn shutdown(&self) -> twerk_infrastructure::broker::BoxedFuture<()> {
            Box::pin(async { Ok(()) })
        }
    }

    async fn create_broker_proxy(broker: SpyBroker) -> BrokerProxy {
        let proxy = BrokerProxy::new();
        let broker: Box<dyn Broker + Send + Sync> = Box::new(broker);
        proxy.set_broker(broker).await;
        proxy
    }

    #[tokio::test]
    async fn send_heartbeat_when_health_check_succeeds_returns_up_status() {
        // Given a healthy worker runtime subscribed to the default queue.
        let runtime = FakeRuntime::new(true);
        let spy = SpyBroker::new();
        let broker = create_broker_proxy(spy.clone()).await;
        let mut sys = System::new_all();
        let identity = HeartbeatIdentity::new(
            "node-1".to_string(),
            "test-node".to_string(),
            "default".to_string(),
        );

        // When the worker publishes its heartbeat.
        send_heartbeat(&broker, &identity, &mut sys, Arc::new(runtime)).await;

        // Then the coordinator receives a datastore-ready UP node record.
        let heartbeats = spy.get_heartbeats().await;
        assert_eq!(heartbeats.len(), 1);
        assert_eq!(heartbeats[0].status, Some(NodeStatus::UP));
        assert_eq!(heartbeats[0].queue.as_deref(), Some("default"));
        assert_eq!(
            heartbeats[0].version.as_deref(),
            Some(env!("CARGO_PKG_VERSION"))
        );
        assert!(heartbeats[0].last_heartbeat_at.is_some());
    }

    #[tokio::test]
    async fn send_heartbeat_when_health_check_fails_returns_down_status() {
        // Given a worker runtime that fails its health check.
        let runtime = FakeRuntime::new(false);
        let spy = SpyBroker::new();
        let broker = create_broker_proxy(spy.clone()).await;
        let mut sys = System::new_all();
        let identity = HeartbeatIdentity::new(
            "node-1".to_string(),
            "test-node".to_string(),
            "default".to_string(),
        );

        // When the worker publishes its heartbeat.
        send_heartbeat(&broker, &identity, &mut sys, Arc::new(runtime)).await;

        // Then the node is reported DOWN without dropping required node fields.
        let heartbeats = spy.get_heartbeats().await;
        assert_eq!(heartbeats.len(), 1);
        assert_eq!(heartbeats[0].status, Some(NodeStatus::DOWN));
        assert_eq!(heartbeats[0].queue.as_deref(), Some("default"));
        assert_eq!(
            heartbeats[0].version.as_deref(),
            Some(env!("CARGO_PKG_VERSION"))
        );
    }

    #[test]
    fn primary_worker_queue_uses_deterministic_worker_queue_for_heartbeats() {
        // Given a worker configured with multiple worker queues and a coordinator queue.
        let queues = HashMap::from([
            ("zeta".to_string(), 1),
            ("x-pending".to_string(), 1),
            ("alpha".to_string(), 1),
        ]);

        // When the heartbeat queue is selected.
        let queue = primary_worker_queue(&queues);

        // Then the selected queue is a stable worker queue, not a coordinator queue.
        assert_eq!(queue, "alpha");
    }
}
