use anyhow::anyhow;
use dashmap::DashMap;
use std::process::ExitCode;
use std::sync::Arc;
use std::time::Duration;
use tokio::process::Command;
use twerk_core::task::Task;
use twerk_core::task::TASK_STATE_ACTIVE;
use twerk_infrastructure::runtime::{
    BoxedFuture, Runtime as RuntimeTrait, ShutdownError, ShutdownResult, DEFAULT_FORCE_TIMEOUT,
    DEFAULT_GRACEFUL_TIMEOUT, ENV_TASK_STOP_ENABLE_CLEANUP, ENV_TASK_STOP_FORCE_TIMEOUT,
    ENV_TASK_STOP_GRACEFUL_TIMEOUT,
};

// Container handle to track running containers
#[derive(Debug, Clone)]
pub struct ContainerHandle {
    pub id: String,
    pub volumes: Vec<String>, // Volume IDs associated with this container
}

// Config for podman runtime adapter
#[derive(Debug, Clone)]
pub struct PodmanRuntimeConfig {
    pub privileged: bool,
    pub host_network: bool,
    pub graceful_timeout: u64,
    pub force_timeout: u64,
    pub enable_cleanup: bool,
}

impl Default for PodmanRuntimeConfig {
    fn default() -> Self {
        Self {
            privileged: false,
            host_network: false,
            graceful_timeout: 30,
            force_timeout: 5,
            enable_cleanup: true,
        }
    }
}

impl From<&PodmanRuntimeConfig> for PodmanRuntimeAdapter {
    fn from(config: &PodmanRuntimeConfig) -> Self {
        Self {
            config: config.clone(),
            active_containers: Arc::new(DashMap::new()),
        }
    }
}

#[derive(Debug)]
pub struct PodmanRuntimeAdapter {
    config: PodmanRuntimeConfig,
    active_containers: Arc<DashMap<String, ContainerHandle>>,
}

impl PodmanRuntimeAdapter {
    #[must_use]
    pub fn new(privileged: bool, host_network: bool) -> Self {
        Self {
            config: PodmanRuntimeConfig {
                privileged,
                host_network,
                graceful_timeout: Self::read_timeout_env(
                    ENV_TASK_STOP_GRACEFUL_TIMEOUT,
                    DEFAULT_GRACEFUL_TIMEOUT,
                ),
                force_timeout: Self::read_timeout_env(
                    ENV_TASK_STOP_FORCE_TIMEOUT,
                    DEFAULT_FORCE_TIMEOUT,
                ),
                enable_cleanup: Self::read_cleanup_env(ENV_TASK_STOP_ENABLE_CLEANUP, true),
            },
            active_containers: Arc::new(DashMap::new()),
        }
    }

    fn read_timeout_env(key: &str, default: u64) -> u64 {
        match std::env::var(key).ok().and_then(|v| v.parse().ok()) {
            Some(val) => val,
            None => default,
        }
    }

    fn read_cleanup_env(key: &str, default: bool) -> bool {
        match std::env::var(key)
            .ok()
            .map(|v| v.to_lowercase() == "true" || v == "1")
        {
            Some(val) => val,
            None => default,
        }
    }

    #[allow(dead_code)]
    fn validate_task(task: &Task) -> ShutdownResult<()> {
        // Check for empty task ID
        if task.id.as_ref().is_none_or(|id| id.is_empty()) {
            return Err(ShutdownError::InvalidTaskId(
                task.id.clone().unwrap_or_default().to_string(),
            ));
        }

        // Check if task is in active state
        if !TASK_STATE_ACTIVE.contains(&task.state.as_str()) {
            return Err(ShutdownError::TaskNotRunning(task.state.clone()));
        }

        Ok(())
    }
}

impl RuntimeTrait for PodmanRuntimeAdapter {
    fn run(&self, task: &Task) -> BoxedFuture<()> {
        let (p, h, tid, img, cmd, wd, env) = (
            self.config.privileged,
            self.config.host_network,
            task.id.clone().unwrap_or_default(),
            task.image.clone().unwrap_or_default(),
            task.cmd.clone(),
            task.workdir.clone(),
            task.env.clone(),
        );

        Box::pin(async move {
            if tid.is_empty() || img.is_empty() {
                return Err(anyhow!("id and image required"));
            }

            let mut c = Command::new("podman");
            c.arg("run");
            if p {
                c.arg("--privileged");
            }
            if h {
                c.arg("--network").arg("host");
            }
            c.arg(&img);
            if let Some(ref a) = cmd {
                for arg in a {
                    c.arg(arg);
                }
            }
            if let Some(ref w) = wd {
                c.arg("--workdir").arg(w);
            }
            if let Some(ref e) = env {
                for (k, v) in e {
                    c.env(k, v);
                }
            }

            let out = c.output().await?;
            if !out.status.success() {
                return Err(anyhow!(
                    "podman failed: {}",
                    String::from_utf8_lossy(&out.stderr)
                ));
            }

            Ok(())
        })
    }

    fn stop(&self, task: &Task) -> BoxedFuture<ShutdownResult<ExitCode>> {
        let (task_id_str, task_state, config, active_containers) = (
            task.id.clone().unwrap_or_default().to_string(),
            task.state.clone(),
            self.config.clone(),
            self.active_containers.clone(),
        );

        Box::pin(async move {
            // Validate task first (precondition check)
            if task_id_str.is_empty() {
                return Ok(Err(ShutdownError::InvalidTaskId(task_id_str.clone())));
            }
            if !TASK_STATE_ACTIVE.contains(&task_state.as_str()) {
                return Ok(Err(ShutdownError::TaskNotRunning(task_state.clone())));
            }

            // Check if container is tracked (precondition check)
            let handle = match active_containers.get(task_id_str.as_str()) {
                Some(h) => h.clone(),
                None => {
                    // Container not tracked - could be already stopped or never started
                    // Return Ok with exit code 0 for idempotency
                    return Ok(Ok(ExitCode::SUCCESS));
                }
            };

            // Stop the container
            let exit_code =
                Self::stop_container(&handle, config.graceful_timeout, config.force_timeout)
                    .await?;

            // Remove from active containers map
            active_containers.remove(task_id_str.as_str());

            // Cleanup volumes (postcondition)
            if config.enable_cleanup {
                Self::cleanup_volumes(&handle.volumes).await?;
            }

            Ok(Ok(exit_code))
        })
    }

    fn health_check(&self) -> BoxedFuture<()> {
        Box::pin(async { Ok(()) })
    }
}

impl PodmanRuntimeAdapter {
    async fn stop_container(
        handle: &ContainerHandle,
        graceful_timeout: u64,
        force_timeout: u64,
    ) -> ShutdownResult<ExitCode> {
        let container_id = &handle.id;

        // Stop container gracefully with timeout
        let stop_timeout = Duration::from_secs(graceful_timeout);

        let mut stop_cmd = Command::new("podman");
        stop_cmd
            .arg("stop")
            .arg("--time")
            .arg(graceful_timeout.to_string())
            .arg(container_id);

        let stop_result = tokio::time::timeout(stop_timeout, stop_cmd.output()).await;

        match stop_result {
            Ok(Ok(out)) => {
                if out.status.success() {
                    // Container stopped gracefully
                    Ok(ExitCode::SUCCESS)
                } else {
                    // Container stop failed, try force remove
                    let exit_code =
                        Self::force_remove_container(container_id, force_timeout).await?;
                    Ok(exit_code)
                }
            }
            Ok(Err(_)) => {
                // Stop command failed, try force remove
                let exit_code = Self::force_remove_container(container_id, force_timeout).await?;
                Ok(exit_code)
            }
            Err(_) => {
                // Timeout on stop, try force remove
                let exit_code = Self::force_remove_container(container_id, force_timeout).await?;
                Ok(exit_code)
            }
        }
    }

    async fn force_remove_container(
        container_id: &str,
        force_timeout: u64,
    ) -> ShutdownResult<ExitCode> {
        let remove_timeout = Duration::from_secs(force_timeout);

        let mut remove_cmd = Command::new("podman");
        remove_cmd.arg("rm").arg("--force").arg(container_id);

        let remove_result = tokio::time::timeout(remove_timeout, remove_cmd.output()).await;

        match remove_result {
            Ok(Ok(out)) => {
                if out.status.success() {
                    Ok(ExitCode::SUCCESS)
                } else {
                    Err(ShutdownError::TerminationFailed(format!(
                        "failed to remove container: {}",
                        String::from_utf8_lossy(&out.stderr)
                    )))
                }
            }
            Ok(Err(e)) => Err(ShutdownError::TerminationFailed(format!(
                "failed to execute podman rm: {}",
                e
            ))),
            Err(_) => Err(ShutdownError::ShutdownTimeout(force_timeout)),
        }
    }

    async fn cleanup_volumes(volumes: &[String]) -> ShutdownResult<()> {
        for volume in volumes {
            if volume.is_empty() {
                continue;
            }

            let mut rm_cmd = Command::new("podman");
            rm_cmd.arg("volume").arg("rm").arg(volume);

            if let Err(e) = rm_cmd.output().await {
                // Log error but continue with other volumes
                tracing::warn!("Failed to remove volume {}: {}", volume, e);
                // Don't fail the entire stop operation for volume cleanup errors
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use twerk_core::task::Task;

    fn create_test_task(id: &str, state: &str) -> Task {
        Task {
            id: Some(id.to_string().into()),
            state: state.to_string(),
            ..Default::default()
        }
    }

    #[test]
    fn test_validate_task_empty_id() {
        let _adapter = PodmanRuntimeAdapter::new(false, false);
        let task = create_test_task("", "RUNNING");

        let result = PodmanRuntimeAdapter::validate_task(&task);
        assert!(matches!(result, Err(ShutdownError::InvalidTaskId(_))));
    }

    #[test]
    fn test_validate_task_completed_state() {
        let _adapter = PodmanRuntimeAdapter::new(false, false);
        let task = create_test_task("task-1", "COMPLETED");

        let result = PodmanRuntimeAdapter::validate_task(&task);
        assert!(matches!(result, Err(ShutdownError::TaskNotRunning(_))));
    }

    #[test]
    fn test_validate_task_running_state() {
        let _adapter = PodmanRuntimeAdapter::new(false, false);
        let task = create_test_task("task-1", "RUNNING");

        let result = PodmanRuntimeAdapter::validate_task(&task);
        assert!(result.is_ok());
    }
}
