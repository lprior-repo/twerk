use bollard::query_parameters::RemoveContainerOptions;
use bollard::Docker;
use dashmap::DashMap;
use std::pin::Pin;
use std::process::ExitCode;
use std::sync::Arc;
use tracing::warn;
use twerk_core::id::TaskId;
use twerk_core::mount::Mount;
use twerk_core::task::Task;
use twerk_infrastructure::broker::Broker;
use twerk_infrastructure::runtime::docker::create_task_container;
use twerk_infrastructure::runtime::docker::mounters::Mounter as DockerMounter;
use twerk_infrastructure::runtime::Mounter;
use twerk_infrastructure::runtime::{BoxedFuture, Runtime as RuntimeTrait, ShutdownResult};

// ── Typed errors for Docker runtime ────────────────────────────────

#[derive(Debug, thiserror::Error)]
enum DockerWorkerError {
    #[error("task id required")]
    TaskIdRequired,
    #[error("task image required")]
    TaskImageRequired,
    #[error("failed to connect to docker: {0}")]
    ConnectionFailed(String),
    #[error("failed to create container: {0}")]
    ContainerCreateFailed(String),
    #[error("failed to start container: {0}")]
    ContainerStartFailed(String),
    #[error("container wait error: {0}")]
    ContainerWaitError(String),
    #[error("task has no ID for stop operation")]
    MissingTaskIdForStop,
}

struct DockerMounterAdapter {
    inner: Arc<dyn Mounter + Send + Sync>,
}

impl DockerMounterAdapter {
    fn new(inner: Arc<dyn Mounter + Send + Sync>) -> Self {
        Self { inner }
    }
}

impl DockerMounter for DockerMounterAdapter {
    fn mount(
        &self,
        mnt: &Mount,
    ) -> Pin<Box<dyn std::future::Future<Output = std::result::Result<(), String>> + Send + '_>>
    {
        let inner = self.inner.clone();
        let mnt = mnt.clone();
        Box::pin(async move { inner.mount(&mnt).await.map_err(|e| e.to_string()) })
    }

    fn unmount(
        &self,
        mnt: &Mount,
    ) -> Pin<Box<dyn std::future::Future<Output = std::result::Result<(), String>> + Send + '_>>
    {
        let inner = self.inner.clone();
        let mnt = mnt.clone();
        Box::pin(async move { inner.unmount(&mnt).await.map_err(|e| e.to_string()) })
    }
}

#[derive(Clone)]
pub struct DockerRuntimeAdapter {
    #[allow(dead_code)]
    privileged: bool,
    #[allow(dead_code)]
    image_ttl_secs: u64,
    active_tasks: Arc<DashMap<TaskId, String>>,
    mounter: Arc<dyn Mounter + Send + Sync>,
    broker: Arc<dyn Broker>,
}

impl DockerRuntimeAdapter {
    #[must_use]
    pub fn new(
        privileged: bool,
        image_ttl_secs: u64,
        mounter: Arc<dyn Mounter + Send + Sync>,
        broker: Arc<dyn Broker>,
    ) -> Self {
        Self {
            privileged,
            image_ttl_secs,
            active_tasks: Arc::new(DashMap::new()),
            mounter,
            broker,
        }
    }
}

impl DockerRuntimeAdapter {
    pub fn execute_task(self, task: Task) -> BoxedFuture<()> {
        let active_tasks = self.active_tasks.clone();
        let mounter = self.mounter.clone();
        let broker = self.broker.clone();
        Box::pin(async move {
            let task_id = task.id.clone().ok_or(DockerWorkerError::TaskIdRequired)?;
            if task_id.is_empty() {
                return Err(DockerWorkerError::TaskIdRequired.into());
            }
            if task.image.as_ref().is_none_or(|img| img.is_empty()) {
                return Err(DockerWorkerError::TaskImageRequired.into());
            }

            let client = match Docker::connect_with_local_defaults() {
                Ok(c) => c,
                Err(e) => return Err(DockerWorkerError::ConnectionFailed(e.to_string()).into()),
            };
            let logger = Box::new(std::io::sink());
            let mounter = Arc::new(DockerMounterAdapter::new(mounter));

            let tc = match create_task_container(&client, mounter, broker, &task, logger).await {
                Ok(tc) => tc,
                Err(e) => {
                    return Err(DockerWorkerError::ContainerCreateFailed(e.to_string()).into())
                }
            };

            let tc_id = tc.id.clone();
            active_tasks.insert(task_id.clone(), tc_id.clone());

            let start_result = tc.start().await;

            if let Err(e) = start_result {
                active_tasks.remove(&task_id);
                if let Err(e) = tc.remove().await {
                    warn!(error = %e, "failed to remove container after start failure");
                }
                return Err(DockerWorkerError::ContainerStartFailed(e.to_string()).into());
            }

            let wait_result = tc.wait().await;
            active_tasks.remove(&task_id);

            if let Err(e) = wait_result {
                if let Err(re) = tc.remove().await {
                    warn!(error = %re, "failed to remove container after wait error");
                }
                return Err(DockerWorkerError::ContainerWaitError(e.to_string()).into());
            }

            if let Err(e) = tc.remove().await {
                warn!(error = %e, "failed to remove container after completion");
            }
            Ok(())
        })
    }
}

impl RuntimeTrait for DockerRuntimeAdapter {
    fn run(&self, task: &Task) -> BoxedFuture<()> {
        self.clone().execute_task(task.clone())
    }

    fn stop(&self, task: &Task) -> BoxedFuture<ShutdownResult<ExitCode>> {
        let tid = task.id.clone();
        let active = self.active_tasks.clone();
        Box::pin(async move {
            let tid = tid.ok_or(DockerWorkerError::MissingTaskIdForStop)?;
            if let Some((_, cid)) = active.remove(&tid) {
                let d = Docker::connect_with_local_defaults()?;
                if let Err(e) = d.stop_container(&cid, None).await {
                    warn!(error = %e, container_id = %cid, "failed to stop container during cleanup");
                }
                if let Err(e) = d
                    .remove_container(
                        &cid,
                        Some(RemoveContainerOptions {
                            force: true,
                            ..Default::default()
                        }),
                    )
                    .await
                {
                    warn!(error = %e, container_id = %cid, "failed to remove container during cleanup");
                }
            }
            Ok(Ok(ExitCode::SUCCESS))
        })
    }

    fn health_check(&self) -> BoxedFuture<()> {
        Box::pin(async {
            Docker::connect_with_local_defaults()?
                .ping()
                .await
                .map(|_| ())
                .map_err(|e| anyhow::anyhow!("{e}"))
        })
    }
}
