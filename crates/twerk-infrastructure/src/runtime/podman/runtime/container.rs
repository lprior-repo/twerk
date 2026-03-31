//! Container execution logic for PodmanRuntime.

use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;
use std::time::Duration;

use tokio::io::AsyncBufReadExt;
use tokio::process::Command;
use tokio::sync::RwLock;
use tracing::{debug, warn};

use super::super::errors::PodmanError;
use super::super::types::{Broker, CoreTask, RegistryCredentials};
use super::types::PodmanRuntime;

impl PodmanRuntime {
    /// Execute container and handle logs.
    pub(crate) async fn execute_container(
        &self,
        task: &mut CoreTask,
        workdir: &Path,
        output_file: &Path,
        progress_file: &Path,
    ) -> Result<(), PodmanError> {
        // Convert to owned PathBuf for async tasks
        let progress_file_buf = progress_file.to_path_buf();
        let task_id_str = task.id.as_ref().map_or("unknown", |id| id.as_str());
        let image = task.image.as_ref().ok_or(PodmanError::ImageRequired)?;

        // Pull image
        let registry = task.registry.as_ref().and_then(|r| {
            let username = r.username.as_ref()?;
            let password = r.password.as_ref()?;
            if username.is_empty() {
                None
            } else {
                Some(RegistryCredentials {
                    username: username.clone(),
                    password: password.clone(),
                })
            }
        });

        self.image_pull(image, registry).await?;

        // Optional image verification
        if self.image_verify {
            if let Err(e) = Self::verify_image(image).await {
                tracing::error!("image {} is invalid or corrupted: {}", image, e);
                let mut rm_cmd = Command::new("podman");
                rm_cmd.arg("image").arg("rm").arg("-f").arg(image);
                let _ = rm_cmd.output().await;
                return Err(e);
            }
        }

        // Build entrypoint
        let entrypoint = if task.entrypoint.as_ref().is_some_and(|e| !e.is_empty()) {
            task.entrypoint.clone().unwrap()
        } else {
            vec!["sh".to_string()]
        };

        // Build podman create command
        let mut create_cmd = self.build_create_command(workdir, task, entrypoint.clone());

        if self.privileged {
            create_cmd.arg("--privileged");
        }

        // Create container
        let create_output = tokio::time::timeout(Duration::from_secs(30), create_cmd.output())
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
            .insert(task_id_str.to_string(), container_id.clone());

        // Ensure container is stopped on exit
        struct ContainerGuard {
            container_id: String,
            tasks: Arc<RwLock<HashMap<String, String>>>,
        }
        impl Drop for ContainerGuard {
            fn drop(&mut self) {
                let cid = self.container_id.clone();
                let tasks = self.tasks.clone();
                tokio::spawn(async move {
                    if let Err(e) = PodmanRuntime::stop_container_static(&cid).await {
                        warn!("error stopping container {}: {}", cid, e);
                    }
                    tasks.write().await.remove(&cid);
                });
            }
        }
        let _guard = ContainerGuard {
            container_id: container_id.clone(),
            tasks: Arc::clone(&self.tasks),
        };

        // Start progress reporting
        let progress_task_id = task_id_str.to_string();
        let broker = self.broker.clone();
        let progress_handle = tokio::spawn(async move {
            PodmanRuntime::report_progress(
                &progress_task_id,
                &progress_file_buf,
                broker.as_deref(),
            )
            .await;
        });

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

        // Read logs
        let logs_broker = self.broker.clone();
        let logs_task_id = task_id_str.to_string();
        let mut logs_cmd = Command::new("podman");
        logs_cmd.arg("logs").arg("--follow").arg(&container_id);
        logs_cmd.stdout(std::process::Stdio::piped());
        logs_cmd.stderr(std::process::Stdio::piped());

        let mut child = logs_cmd
            .spawn()
            .map_err(|e| PodmanError::LogsRead(e.to_string()))?;

        // Ship logs to broker
        if let Some(stdout) = child.stdout.take() {
            let broker_clone = logs_broker.clone();
            let tid = logs_task_id.clone();
            tokio::spawn(async move {
                let mut reader = tokio::io::BufReader::new(stdout).lines();
                while let Ok(Some(line)) = reader.next_line().await {
                    debug!("[podman:stdout] {}", line);
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
        let output = tokio::fs::read_to_string(&output_file)
            .await
            .map_err(|e| PodmanError::OutputRead(e.to_string()))?;
        task.result = Some(output);

        Ok(())
    }
}
