//! Container execution and lifecycle for Podman runtime

use std::path::Path;

use tokio::io::AsyncBufReadExt;
use tokio::process::Command;
use tracing::{debug, error, warn};

use super::errors::PodmanError;
use super::runtime::ContainerGuard;
use super::types::{PodmanRuntime, RegistryCredentials, Task};

#[allow(dead_code)]
impl PodmanRuntime {
    /// Execute container and handle logs
    pub(crate) async fn execute_container(
        &self,
        task: &mut Task,
        workdir: &Path,
        output_file: &Path,
        progress_file: &Path,
    ) -> Result<(), PodmanError> {
        // Pull image
        let registry = task.registry.as_ref().and_then(|r| {
            if r.username.is_empty() {
                None
            } else {
                Some(RegistryCredentials {
                    username: r.username.clone(),
                    password: r.password.clone(),
                })
            }
        });

        self.image_pull(&task.image, registry).await?;

        // Optional image verification
        if self.image_verify {
            if let Err(e) = PodmanRuntime::verify_image(&task.image).await {
                error!("image {} is invalid or corrupted: {}", task.image, e);
                let mut rm_cmd = Command::new("podman");
                rm_cmd.arg("image").arg("rm").arg("-f").arg(&task.image);
                let _ = rm_cmd.output().await;
                return Err(e);
            }
        }

        // Build entrypoint
        let entrypoint = if task.entrypoint.is_empty() {
            vec!["sh".to_string()]
        } else {
            task.entrypoint.clone()
        };

        // Build podman create command
        let mut create_cmd = PodmanRuntime::build_create_command(workdir, task, entrypoint.clone());
        self.add_privileged_flag(&mut create_cmd);

        let create_output = tokio::time::timeout(
            super::types::CREATE_TIMEOUT,
            create_cmd.output(),
        )
        .await
        .map_err(|_| {
            PodmanError::ContainerCreation("create timed out after 30 seconds".to_string())
        })?
        .map_err(|e| PodmanError::ContainerCreation(e.to_string()))?;

        if !create_output.status.success() {
            return Err(PodmanError::ContainerCreation(
                String::from_utf8_lossy(&create_output.stderr).to_string(),
            ));
        }

        let container_id = String::from_utf8_lossy(&create_output.stdout)
            .trim()
            .to_string();

        if container_id.is_empty() {
            return Err(PodmanError::ContainerCreation(
                "empty container ID".to_string(),
            ));
        }

        debug!("created container {}", container_id);

        self.tasks
            .write()
            .await
            .insert(task.id.clone(), container_id.clone());

        let mut guard = ContainerGuard::new(
            container_id.clone(),
            self.tasks.clone(),
        );

        // Start container
        let mut start_cmd = Command::new("podman");
        start_cmd.arg("start").arg(&container_id);
        start_cmd.stdout(std::process::Stdio::piped());
        start_cmd.stderr(std::process::Stdio::piped());

        let start_output = start_cmd
            .output()
            .await
            .map_err(|e| PodmanError::ContainerStart(e.to_string()))?;

        if !start_output.status.success() {
            return Err(PodmanError::ContainerStart(
                String::from_utf8_lossy(&start_output.stderr).to_string(),
            ));
        }

        // Progress reporting - start before probe so logs are visible during health checks
        let progress_task_id = task.id.clone();
        let progress_file_path = progress_file.to_path_buf();
        let broker = self.broker.clone();
        let progress_handle = tokio::spawn(async move {
            PodmanRuntime::report_progress(&progress_task_id, progress_file_path, broker.as_deref()).await;
        });

        // Read logs - start before probe to align with Go Tork behavior
        let logs_task_id = task.id.clone();
        let logs_broker = self.broker.clone();
        let mut logs_cmd = Command::new("podman");
        logs_cmd.arg("logs").arg("--follow").arg(&container_id);
        logs_cmd.stdout(std::process::Stdio::piped());
        logs_cmd.stderr(std::process::Stdio::piped());

        let mut child = logs_cmd
            .spawn()
            .map_err(|e| PodmanError::LogsRead(e.to_string()))?;

        // Probe container (after logs are streaming)
        if let Some(ref probe) = task.probe {
            let host_port = PodmanRuntime::get_host_port(&container_id, probe.port).await?;
            self.probe_container(&host_port, probe).await?;
        }

        if let Some(stdout) = child.stdout.take() {
            let mut reader = tokio::io::BufReader::new(stdout).lines();
            let broker_clone = logs_broker.clone();
            let tid = logs_task_id.clone();
            tokio::spawn(async move {
                while let Ok(Some(line)) = reader.next_line().await {
                    debug!("[podman:stdout] {}", line);
                    if let Some(ref b) = broker_clone {
                        b.ship_log(&tid, &line);
                    }
                }
            });
        }

        if let Some(stderr) = child.stderr.take() {
            let mut reader = tokio::io::BufReader::new(stderr).lines();
            let broker_clone = logs_broker.clone();
            let tid = logs_task_id.clone();
            tokio::spawn(async move {
                while let Ok(Some(line)) = reader.next_line().await {
                    debug!("[podman:stderr] {}", line);
                    if let Some(ref b) = broker_clone {
                        b.ship_log(&tid, &line);
                    }
                }
            });
        }

        child
            .wait()
            .await
            .map_err(|e| PodmanError::LogsRead(e.to_string()))?;

        progress_handle.abort();

        // Check exit code
        let mut inspect_cmd = Command::new("podman");
        inspect_cmd
            .arg("inspect")
            .arg("--format")
            .arg("{{.State.ExitCode}}")
            .arg(&container_id);
        let inspect_output = inspect_cmd
            .output()
            .await
            .map_err(|e| PodmanError::ContainerInspect(e.to_string()))?;

        let exit_code = String::from_utf8_lossy(&inspect_output.stdout)
            .trim()
            .to_string();
        if exit_code != "0" {
            return Err(PodmanError::ContainerExitCode(exit_code));
        }

        // Read output
        let output = tokio::fs::read_to_string(output_file)
            .await
            .map_err(|e| PodmanError::OutputRead(e.to_string()))?;
        task.result = output;

        // Cleanup
        if let Err(e) = PodmanRuntime::stop_container_static(&container_id).await {
            warn!("error stopping container {}: {}", container_id, e);
        }
        self.tasks.write().await.remove(&container_id);
        guard.disarm();

        if let Err(e) = tokio::fs::remove_dir_all(workdir).await {
            warn!("error removing workdir {:?}: {}", workdir, e);
        }

        Ok(())
    }
}
