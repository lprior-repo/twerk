use crate::engine::BrokerProxy;
use anyhow::Result;
use dashmap::DashMap;
use std::collections::HashMap;
use std::sync::Arc;
use sysinfo::System;
use tokio::sync::broadcast;
use tracing::{debug, info, warn};
use twerk_core::id::{NodeId, TaskId};
use twerk_core::node::{Node, NodeStatus};
use twerk_core::task::{Task, TASK_STATE_COMPLETED, TASK_STATE_FAILED, TASK_STATE_RUNNING};
use twerk_infrastructure::broker::Broker;
use twerk_infrastructure::runtime::Runtime as RuntimeTrait;

pub mod docker;
pub mod mounter;
pub mod podman;
pub mod runtime_adapter;
pub mod shell;

pub type BoxedFuture<T> = std::pin::Pin<Box<dyn std::future::Future<Output = Result<T>> + Send>>;

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
            let hb_broker = broker.clone();
            let (hb_id, hb_name) = (id.clone(), name.clone());
            let mut hb_terminate_rx = terminate_tx.subscribe();
            let runtime_hb = runtime.clone();
            tokio::spawn(async move {
                let mut sys = System::new_all();
                loop {
                    send_heartbeat(&hb_broker, &hb_id, &hb_name, &mut sys, runtime_hb.clone())
                        .await;
                    tokio::select! { _ = hb_terminate_rx.recv() => break, _ = tokio::time::sleep(tokio::time::Duration::from_secs(30)) => {} }
                }
            });
            for (qname, concurrency) in queues {
                for _ in 0..concurrency {
                    let (q_broker, q_runtime, q_name, mut q_terminate_rx, q_active_tasks) = (
                        broker.clone(),
                        runtime.clone(),
                        qname.clone(),
                        terminate_tx.subscribe(),
                        active_tasks.clone(),
                    );
                    tokio::spawn(async move {
                        let qb = q_broker.clone();
                        let handler: twerk_infrastructure::broker::TaskHandler =
                            Arc::new(move |task: Arc<Task>| {
                                let (b, r, a) =
                                    (qb.clone(), q_runtime.clone(), q_active_tasks.clone());
                                Box::pin(async move { execute_task(task, r, b, a).await })
                            });
                        let _ = q_broker.subscribe_for_tasks(q_name, handler).await;
                        let _ = q_terminate_rx.recv().await;
                    });
                }
            }
            let cancel_q = format!("cancel.{}", id);
            let (c_runtime, c_active_tasks, mut c_terminate_rx) = (
                runtime.clone(),
                active_tasks.clone(),
                terminate_tx.subscribe(),
            );
            tokio::spawn(async move {
                let handler: twerk_infrastructure::broker::TaskHandler =
                    Arc::new(move |task: Arc<Task>| {
                        let (r, a) = (c_runtime.clone(), c_active_tasks.clone());
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
                let _ = broker.subscribe_for_tasks(cancel_q, handler).await;
                let _ = c_terminate_rx.recv().await;
            });
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

async fn send_heartbeat(
    broker: &BrokerProxy,
    id: &str,
    name: &str,
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
        id: Some(NodeId::from(id)),
        name: Some(name.to_string()),
        hostname: Some(
            hostname::get()
                .map(|h| h.to_string_lossy().into_owned())
                .unwrap_or_else(|_| "unknown".to_string()),
        ),
        cpu_percent: Some(sys.global_cpu_usage() as f64),
        status: Some(status),
        ..Default::default()
    };
    let _ = broker.publish_heartbeat(node).await;
}

async fn execute_task(
    task: Arc<Task>,
    runtime: Arc<dyn RuntimeTrait + Send + Sync>,
    broker: BrokerProxy,
    active_tasks: Arc<DashMap<TaskId, Arc<Task>>>,
) -> Result<()> {
    let mut t = (*task).clone();
    let tid = t.id.clone().unwrap_or_default();
    active_tasks.insert(tid.clone(), task.clone());
    t.state = TASK_STATE_RUNNING.to_string();
    t.started_at = Some(time::OffsetDateTime::now_utc());
    let _ = broker.publish_task_progress(&t).await;
    match runtime.run(&t).await {
        Ok(()) => {
            t.state = TASK_STATE_COMPLETED.to_string();
            t.completed_at = Some(time::OffsetDateTime::now_utc());
        }
        Err(e) => {
            t.state = TASK_STATE_FAILED.to_string();
            t.failed_at = Some(time::OffsetDateTime::now_utc());
            t.error = Some(e.to_string());
        }
    }
    active_tasks.remove(&tid);
    let _ = broker.publish_task_progress(&t).await;
    Ok(())
}

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
                    && task.state == TASK_STATE_RUNNING
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

pub fn read_limits() -> Limits {
    Limits {
        cpus: std::env::var("TWERK_WORKER_LIMITS_CPUS")
            .unwrap_or_else(|_| DEFAULT_CPUS_LIMIT.to_string()),
        memory: std::env::var("TWERK_WORKER_LIMITS_MEMORY")
            .unwrap_or_else(|_| DEFAULT_MEMORY_LIMIT.to_string()),
        timeout: std::env::var("TWERK_WORKER_LIMITS_TIMEOUT")
            .unwrap_or_else(|_| DEFAULT_TIMEOUT.to_string()),
    }
}

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
    if let Some(h) = create_hostenv_middleware(&config.hostenv_vars) {
        engine.register_task_middleware(h);
    }
    let id = std::env::var("TWERK_WORKER_ID").unwrap_or_else(|_| twerk_core::uuid::new_uuid());
    let name = std::env::var("TWERK_WORKER_NAME").unwrap_or_else(|_| "Worker".to_string());
    let queues = std::env::var("TWERK_WORKER_QUEUES").ok().map_or_else(
        || {
            let mut m = HashMap::new();
            m.insert("default".to_string(), 1);
            m
        },
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
    struct MockRuntime {
        health_check_ok: bool,
    }

    impl MockRuntime {
        fn new(health_check_ok: bool) -> Self {
            Self { health_check_ok }
        }
    }

    impl RuntimeTrait for MockRuntime {
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
        let runtime = MockRuntime::new(true);
        let spy = SpyBroker::new();
        let broker = create_broker_proxy(spy.clone()).await;
        let mut sys = System::new_all();

        send_heartbeat(&broker, "node-1", "test-node", &mut sys, Arc::new(runtime)).await;

        let heartbeats = spy.get_heartbeats().await;
        assert_eq!(heartbeats.len(), 1);
        assert_eq!(heartbeats[0].status, Some(NodeStatus::UP));
    }

    #[tokio::test]
    async fn send_heartbeat_when_health_check_fails_returns_down_status() {
        let runtime = MockRuntime::new(false);
        let spy = SpyBroker::new();
        let broker = create_broker_proxy(spy.clone()).await;
        let mut sys = System::new_all();

        send_heartbeat(&broker, "node-1", "test-node", &mut sys, Arc::new(runtime)).await;

        let heartbeats = spy.get_heartbeats().await;
        assert_eq!(heartbeats.len(), 1);
        assert_eq!(heartbeats[0].status, Some(NodeStatus::DOWN));
    }
}
