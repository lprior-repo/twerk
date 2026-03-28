use anyhow::anyhow;
use bollard::query_parameters::RemoveContainerOptions;
use bollard::Docker;
use dashmap::DashMap;
use std::pin::Pin;
use std::sync::Arc;
use twerk_core::id::TaskId;
use twerk_core::mount::Mount;
use twerk_core::task::Task;
use std::process::ExitCode;
use twerk_infrastructure::runtime::{BoxedFuture, Runtime as RuntimeTrait, ShutdownResult};
use twerk_infrastructure::runtime::docker::create_task_container;
use twerk_infrastructure::runtime::docker::mounters::Mounter as DockerMounter;
use twerk_infrastructure::runtime::Mounter;
use twerk_infrastructure::broker::Broker;

struct DockerMounterAdapter {
    inner: Arc<dyn Mounter + Send + Sync>,
}

impl DockerMounterAdapter {
    fn new(inner: Arc<dyn Mounter + Send + Sync>) -> Self {
        Self { inner }
    }
}

impl DockerMounter for DockerMounterAdapter {
    fn mount(&self, mnt: &Mount) -> Pin<Box<dyn std::future::Future<Output = std::result::Result<(), String>> + Send + '_>> {
        let inner = self.inner.clone();
        let mnt = mnt.clone();
        Box::pin(async move {
            inner.mount(&mnt).await.map_err(|e| e.to_string())
        })
    }

    fn unmount(&self, mnt: &Mount) -> Pin<Box<dyn std::future::Future<Output = std::result::Result<(), String>> + Send + '_>> {
        let inner = self.inner.clone();
        let mnt = mnt.clone();
        Box::pin(async move {
            inner.unmount(&mnt).await.map_err(|e| e.to_string())
        })
    }
}

#[derive(Clone)]
#[allow(dead_code)]
pub struct DockerRuntimeAdapter {
    privileged: bool,
    image_ttl_secs: u64,
    active_tasks: Arc<DashMap<TaskId, String>>,
    mounter: Arc<dyn Mounter + Send + Sync>,
    broker: Arc<dyn Broker>,
}

impl DockerRuntimeAdapter {
    #[must_use]
    pub fn new(privileged: bool, image_ttl_secs: u64, mounter: Arc<dyn Mounter + Send + Sync>, broker: Arc<dyn Broker>) -> Self {
        Self { privileged, image_ttl_secs, active_tasks: Arc::new(DashMap::new()), mounter, broker }
    }
}

impl DockerRuntimeAdapter {
    pub fn execute_task(self, task: Task) -> BoxedFuture<()> {
        let active_tasks = self.active_tasks.clone();
        let mounter = self.mounter.clone();
        let broker = self.broker.clone();
        Box::pin(async move {
            if task.id.as_ref().is_none_or(|id| id.is_empty()) {
                return Err(anyhow!("task id required"));
            }
            if task.image.as_ref().is_none_or(|img| img.is_empty()) {
                return Err(anyhow!("task image required"));
            }

            let client = match Docker::connect_with_local_defaults() {
                Ok(c) => c,
                Err(e) => return Err(anyhow!("failed to connect to docker: {}", e)),
            };
            let logger = Box::new(std::io::sink());
            let mounter = Arc::new(DockerMounterAdapter::new(mounter));

            let tc = match create_task_container(&client, mounter, broker, &task, logger).await {
                Ok(tc) => tc,
                Err(e) => return Err(anyhow!("failed to create container: {}", e)),
            };

            let tc_id = tc.id.clone();
            active_tasks.insert(task.id.clone().unwrap(), tc_id.clone());

            let start_result = tc.start().await;

            if let Err(e) = start_result {
                active_tasks.remove(&task.id.clone().unwrap());
                tc.remove().await.map_err(|e| anyhow!("failed to remove container: {}", e)).ok();
                return Err(anyhow!("failed to start container: {}", e));
            }

            let wait_result = tc.wait().await;
            active_tasks.remove(&task.id.clone().unwrap());

            if let Err(e) = wait_result {
                tc.remove().await.map_err(|e| anyhow!("failed to remove container after wait error: {}", e)).ok();
                return Err(anyhow!("container wait error: {}", e));
            }

            tc.remove().await.map_err(|e| anyhow!("failed to remove container: {}", e)).ok();
            Ok(())
        })
    }
}

impl RuntimeTrait for DockerRuntimeAdapter {
    fn run(&self, task: &Task) -> BoxedFuture<()> {
        self.clone().execute_task(task.clone())
    }

    fn stop(&self, task: &Task) -> BoxedFuture<ShutdownResult<ExitCode>> {
        let tid = task.id.clone().unwrap_or_default();
        let active = self.active_tasks.clone();
        Box::pin(async move {
            if let Some((_, cid)) = active.remove(&tid) {
                let d = Docker::connect_with_local_defaults()?;
                let _ = d.stop_container(&cid, None).await;
                let _ = d.remove_container(&cid, Some(RemoveContainerOptions { force: true, ..Default::default() })).await;
            }
            Ok(Ok(ExitCode::SUCCESS))
        })
    }

    fn health_check(&self) -> BoxedFuture<()> {
        Box::pin(async { Docker::connect_with_local_defaults()?.ping().await.map(|_| ()).map_err(|e| anyhow!(e)) })
    }
}
