use bollard::query_parameters::RemoveContainerOptions;
use bollard::Docker;
use dashmap::DashMap;
use std::pin::Pin;
use std::process::ExitCode;
use std::sync::Arc;
use tracing::warn;
use twerk_core::id::TaskId;
use twerk_core::mount::Mount;
use twerk_core::task::{Task, TaskLogPart};
use twerk_infrastructure::broker::Broker;
use twerk_infrastructure::runtime::docker::create_task_container;
use twerk_infrastructure::runtime::docker::mounters::Mounter as DockerMounter;
use twerk_infrastructure::runtime::docker::DockerError;
use twerk_infrastructure::runtime::Mounter;
use twerk_infrastructure::runtime::{BoxedFuture, Runtime as RuntimeTrait, ShutdownResult};

// ── Typed errors for Docker runtime ────────────────────────────────

#[derive(Debug, thiserror::Error)]
pub(crate) enum DockerWorkerError {
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
    #[error("container exited with code {0}: {1}")]
    ContainerNonZeroExit(i32, String),
    #[error("task has no ID for stop operation")]
    MissingTaskIdForStop,
}

fn non_empty_output(output: String) -> Option<String> {
    (!output.is_empty()).then_some(output)
}

fn docker_status_to_exit_code(status_code: i64) -> i32 {
    i32::try_from(status_code).map_or(i32::MAX, std::convert::identity)
}

async fn publish_docker_log_part(
    broker: &Arc<dyn Broker>,
    task_id: &TaskId,
    contents: String,
) -> anyhow::Result<()> {
    if contents.trim().is_empty() {
        return Ok(());
    }

    broker
        .publish_task_log_part(&TaskLogPart {
            id: None,
            number: 1,
            task_id: Some(task_id.clone()),
            contents: Some(contents),
            created_at: None,
        })
        .await
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
    privilege: DockerPrivilege,
    #[allow(dead_code)]
    image_ttl_secs: u64,
    active_tasks: Arc<DashMap<TaskId, String>>,
    mounter: Arc<dyn Mounter + Send + Sync>,
    broker: Arc<dyn Broker>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DockerPrivilege {
    Restricted,
    Privileged,
}

impl From<bool> for DockerPrivilege {
    fn from(privileged: bool) -> Self {
        if privileged {
            Self::Privileged
        } else {
            Self::Restricted
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DockerRuntimePolicy {
    pub privilege: DockerPrivilege,
    pub image_ttl_secs: u64,
}

impl DockerRuntimeAdapter {
    #[must_use]
    pub fn new(
        policy: DockerRuntimePolicy,
        mounter: Arc<dyn Mounter + Send + Sync>,
        broker: Arc<dyn Broker>,
    ) -> Self {
        Self {
            privilege: policy.privilege,
            image_ttl_secs: policy.image_ttl_secs,
            active_tasks: Arc::new(DashMap::new()),
            mounter,
            broker,
        }
    }
}

impl DockerRuntimeAdapter {
    pub fn execute_task(self, task: Task) -> BoxedFuture<Option<String>> {
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

            let tc = match create_task_container(&client, mounter, broker.clone(), &task, logger)
                .await
            {
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

            let output = match wait_result {
                Ok(stdout) => {
                    match tc.read_logs_tail(100).await {
                        Ok(logs) => publish_docker_log_part(&broker, &task_id, logs).await?,
                        Err(e) => warn!(error = %e, "failed to read docker logs after completion"),
                    }
                    non_empty_output(stdout)
                }
                Err(DockerError::NonZeroExit(status_code, tail)) => {
                    publish_docker_log_part(&broker, &task_id, tail.clone()).await?;
                    if let Err(re) = tc.remove().await {
                        warn!(error = %re, "failed to remove container after non-zero exit");
                    }
                    return Err(DockerWorkerError::ContainerNonZeroExit(
                        docker_status_to_exit_code(status_code),
                        tail,
                    )
                    .into());
                }
                Err(e) => {
                    match tc.read_logs_tail(100).await {
                        Ok(logs) => publish_docker_log_part(&broker, &task_id, logs).await?,
                        Err(log_error) => {
                            warn!(error = %log_error, "failed to read docker logs after wait error")
                        }
                    }
                    if let Err(re) = tc.remove().await {
                        warn!(error = %re, "failed to remove container after wait error");
                    }
                    return Err(DockerWorkerError::ContainerWaitError(e.to_string()).into());
                }
            };

            if let Err(e) = tc.remove().await {
                warn!(error = %e, "failed to remove container after completion");
            }
            Ok(output)
        })
    }
}

impl RuntimeTrait for DockerRuntimeAdapter {
    fn run(&self, task: &Task) -> BoxedFuture<Option<String>> {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn non_empty_output_returns_none_for_empty_container_output() {
        assert_eq!(non_empty_output(String::new()), None);
    }

    #[test]
    fn non_empty_output_preserves_container_output() {
        assert_eq!(
            non_empty_output("docker-result".to_string()),
            Some("docker-result".to_string())
        );
    }

    #[test]
    fn docker_status_to_exit_code_preserves_standard_exit_codes() {
        assert_eq!(docker_status_to_exit_code(42), 42);
    }

    #[test]
    fn docker_status_to_exit_code_saturates_large_values() {
        assert_eq!(docker_status_to_exit_code(i64::MAX), i32::MAX);
    }
}
