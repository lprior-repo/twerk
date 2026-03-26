use crate::engine::BrokerProxy;
use anyhow::Result;
use dashmap::DashMap;
use std::collections::HashMap;
use std::sync::Arc;
use sysinfo::System;
use tokio::sync::broadcast;
use tracing::{debug, info, warn};
use twerk_core::id::{TaskId, NodeId};
use twerk_core::node::{Node, NodeStatus};
use twerk_core::task::{Task, TASK_STATE_COMPLETED, TASK_STATE_FAILED, TASK_STATE_RUNNING};
use twerk_infrastructure::broker::Broker;
use twerk_infrastructure::runtime::Runtime as RuntimeTrait;

pub mod mounter;
pub mod runtime_adapter;
pub mod docker;
pub mod shell;
pub mod podman;

pub type BoxedFuture<T> = std::pin::Pin<Box<dyn std::future::Future<Output = Result<T>> + Send>>;

pub trait Worker: Send + Sync {
    fn start(&self) -> BoxedFuture<()>;
    fn stop(&self) -> BoxedFuture<()>;
}

pub use Worker as WorkerTrait;

#[derive(Debug, Clone, Default)]
pub struct Limits { pub cpus: String, pub memory: String, pub timeout: String }

pub const DEFAULT_CPUS_LIMIT: &str = "1";
pub const DEFAULT_MEMORY_LIMIT: &str = "512m";
pub const DEFAULT_TIMEOUT: &str = "5m";

pub struct DefaultWorker {
    id: String, name: String, broker: BrokerProxy, runtime: Arc<dyn RuntimeTrait + Send + Sync>,
    queues: HashMap<String, i32>, limits: Limits, terminate_tx: broadcast::Sender<()>, active_tasks: Arc<DashMap<TaskId, Arc<Task>>>,
}

impl DefaultWorker {
    pub fn new(id: String, name: String, broker: BrokerProxy, runtime: Arc<dyn RuntimeTrait + Send + Sync>, queues: HashMap<String, i32>, limits: Limits) -> Self {
        let (terminate_tx, _) = broadcast::channel(1);
        Self { id, name, broker, runtime, queues, limits, terminate_tx, active_tasks: Arc::new(DashMap::new()) }
    }
}

impl Worker for DefaultWorker {
    fn start(&self) -> BoxedFuture<()> {
        let (id, name, broker, runtime, queues, terminate_tx, active_tasks) = (self.id.clone(), self.name.clone(), self.broker.clone(), self.runtime.clone(), self.queues.clone(), self.terminate_tx.clone(), self.active_tasks.clone());
        Box::pin(async move {
            info!("Worker {} ({}) starting", name, id);
            let hb_broker = broker.clone();
            let (hb_id, hb_name) = (id.clone(), name.clone());
            let mut hb_terminate_rx = terminate_tx.subscribe();
            tokio::spawn(async move {
                let mut sys = System::new_all();
                loop {
                    send_heartbeat(&hb_broker, &hb_id, &hb_name, &mut sys).await;
                    tokio::select! { _ = hb_terminate_rx.recv() => break, _ = tokio::time::sleep(tokio::time::Duration::from_secs(30)) => {} }
                }
            });
            for (qname, concurrency) in queues {
                for _ in 0..concurrency {
                    let (q_broker, q_runtime, q_name, mut q_terminate_rx, q_active_tasks) = (broker.clone(), runtime.clone(), qname.clone(), terminate_tx.subscribe(), active_tasks.clone());
                    tokio::spawn(async move {
                        let qb = q_broker.clone();
                        let handler: twerk_infrastructure::broker::TaskHandler = Arc::new(move |task: Arc<Task>| {
                            let (b, r, a) = (qb.clone(), q_runtime.clone(), q_active_tasks.clone());
                            Box::pin(async move { execute_task(task, r, b, a).await })
                        });
                        let _ = q_broker.subscribe_for_tasks(q_name, handler).await;
                        let _ = q_terminate_rx.recv().await;
                    });
                }
            }
            let cancel_q = format!("cancel.{}", id);
            let (c_runtime, c_active_tasks, mut c_terminate_rx) = (runtime.clone(), active_tasks.clone(), terminate_tx.subscribe());
            tokio::spawn(async move {
                let handler: twerk_infrastructure::broker::TaskHandler = Arc::new(move |task: Arc<Task>| {
                    let (r, a) = (c_runtime.clone(), c_active_tasks.clone());
                    Box::pin(async move {
                        if let Some(tid) = &task.id { if let Some((_, t)) = a.remove(tid) { debug!("Cancelling task {}", tid); let _ = r.stop(&t).await; } }
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
            while !active_tasks.is_empty() && start.elapsed() < std::time::Duration::from_secs(10) { tokio::time::sleep(tokio::time::Duration::from_millis(100)).await; }
            if !active_tasks.is_empty() { warn!("Worker stopping with {} tasks active", active_tasks.len()); }
            Ok(())
        })
    }
}

async fn send_heartbeat(broker: &BrokerProxy, id: &str, name: &str, sys: &mut System) {
    sys.refresh_cpu_all();
    let node = Node { id: Some(NodeId::from(id)), name: Some(name.to_string()), hostname: Some(hostname::get().map(|h| h.to_string_lossy().into_owned()).unwrap_or_else(|_| "unknown".to_string())), cpu_percent: Some(sys.global_cpu_usage() as f64), status: Some(NodeStatus::UP), ..Default::default() };
    let _ = broker.publish_heartbeat(node).await;
}

async fn execute_task(task: Arc<Task>, runtime: Arc<dyn RuntimeTrait + Send + Sync>, broker: BrokerProxy, active_tasks: Arc<DashMap<TaskId, Arc<Task>>>) -> Result<()> {
    let mut t = (*task).clone();
    let tid = t.id.clone().map_or_else(TaskId::default, |id| id);
    active_tasks.insert(tid.clone(), task.clone());
    t.state = TASK_STATE_RUNNING.to_string();
    t.started_at = Some(time::OffsetDateTime::now_utc());
    let _ = broker.publish_task_progress(&t).await;
    match runtime.run(&t).await {
        Ok(()) => { t.state = TASK_STATE_COMPLETED.to_string(); t.completed_at = Some(time::OffsetDateTime::now_utc()); }
        Err(e) => { t.state = TASK_STATE_FAILED.to_string(); t.failed_at = Some(time::OffsetDateTime::now_utc()); t.error = Some(e.to_string()); }
    }
    active_tasks.remove(&tid);
    let _ = broker.publish_task_progress(&t).await;
    Ok(())
}

pub fn create_hostenv_middleware(vars: &[String]) -> Option<crate::engine::TaskMiddlewareFunc> {
    if vars.is_empty() { return None; }
    let var_map: HashMap<String, String> = vars.iter().filter_map(|v| {
        let p: Vec<&str> = v.split(':').collect();
        match p.len() { 1 if !p[0].is_empty() => Some((p[0].to_string(), p[0].to_string())), 2 if !p[0].is_empty() && !p[1].is_empty() => Some((p[0].to_string(), p[1].to_string())), _ => { warn!("invalid env spec: {}", v); None } }
    }).collect();
    if var_map.is_empty() { return None; }
    Some(Arc::new(move |next: crate::engine::TaskHandlerFunc| -> crate::engine::TaskHandlerFunc {
        let vm = var_map.clone();
        Arc::new(move |_ctx, et, task| {
            if et == crate::engine::TaskEventType::StateChange && task.state == TASK_STATE_RUNNING {
                if task.env.is_none() { task.env = Some(HashMap::new()); }
                if let Some(ref mut e) = task.env { for (hn, tn) in &vm { if let Ok(v) = std::env::var(hn) { e.insert(tn.clone(), v); } } }
            }
            next(_ctx, et, task)
        })
    }))
}

pub fn read_limits() -> Limits {
    Limits {
        cpus: std::env::var("TWERK_WORKER_LIMITS_CPUS").map_or_else(|_| DEFAULT_CPUS_LIMIT.to_string(), |v| v),
        memory: std::env::var("TWERK_WORKER_LIMITS_MEMORY").map_or_else(|_| DEFAULT_MEMORY_LIMIT.to_string(), |v| v),
        timeout: std::env::var("TWERK_WORKER_LIMITS_TIMEOUT").map_or_else(|_| DEFAULT_TIMEOUT.to_string(), |v| v),
    }
}

pub async fn create_worker(engine: &mut crate::engine::Engine, broker: BrokerProxy, runtime: Option<Box<dyn RuntimeTrait + Send + Sync>>) -> Result<Box<dyn Worker + Send + Sync>> {
    use crate::engine::worker::runtime_adapter::{read_runtime_config, create_runtime_from_config};
    let config = read_runtime_config();
    let rt = match runtime { Some(r) => Arc::from(r), None => Arc::from(create_runtime_from_config(&config).await?) };
    if let Some(h) = create_hostenv_middleware(&config.hostenv_vars) { engine.register_task_middleware(h); }
    let id = std::env::var("TWERK_WORKER_ID").map_or_else(|_| twerk_core::uuid::new_uuid(), |v| v);
    let name = std::env::var("TWERK_WORKER_NAME").map_or_else(|_| "Worker".to_string(), |v| v);
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
    Ok(Box::new(DefaultWorker::new(id, name, broker, rt, queues, read_limits())))
}
