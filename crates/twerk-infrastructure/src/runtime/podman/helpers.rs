//! Additional helper functions for Podman runtime

use std::path::PathBuf;
use std::time::Instant;

use tokio::process::Command;
use tracing::{debug, error};

use super::errors::PodmanError;
use super::types::{Broker, PodmanRuntime, PullRequest, RegistryCredentials};

#[allow(dead_code)]
impl PodmanRuntime {
    #[allow(dead_code)]
    pub(crate) async fn do_pull_request(
        image: &str,
        registry: Option<RegistryCredentials>,
        _broker: Option<&(dyn Broker + Send + Sync)>,
    ) -> Result<(), PodmanError> {
        if Self::image_exists_locally(image).await {
            debug!("image {} already exists locally, skipping pull", image);
            return Ok(());
        }

        if let Some(ref creds) = registry {
            if !creds.username.is_empty() {
                Self::registry_login(image, &creds.username, &creds.password).await?;
            }
        }

        debug!("Pulling image {}", image);
        let mut cmd = Command::new("podman");
        cmd.arg("pull").arg(image);
        cmd.stdout(std::process::Stdio::piped());
        cmd.stderr(std::process::Stdio::piped());

        let output = cmd
            .output()
            .await
            .map_err(|e| PodmanError::ImagePull(e.to_string()))?;

        if !output.status.success() {
            return Err(PodmanError::ImagePull(format!(
                "podman pull failed for {}: {}",
                image,
                String::from_utf8_lossy(&output.stderr)
            )));
        }

        Ok(())
    }

    /// Login to a registry with credentials
    async fn registry_login(
        image: &str,
        username: &str,
        password: &str,
    ) -> Result<(), PodmanError> {
        let registry_host = Self::extract_registry_host(image);
        debug!(
            "Logging into registry {} for user {}",
            registry_host, username
        );
        let mut cmd = Command::new("podman");
        cmd.arg("login");
        cmd.arg("--username").arg(username);
        cmd.arg("--password-stdin");
        cmd.arg(&registry_host);
        cmd.stdout(std::process::Stdio::piped());
        cmd.stderr(std::process::Stdio::piped());
        cmd.stdin(std::process::Stdio::piped());

        let mut child = cmd
            .spawn()
            .map_err(|e| PodmanError::RegistryLogin(e.to_string()))?;

        if let Some(ref mut stdin) = child.stdin {
            use tokio::io::AsyncWriteExt;
            if let Err(_e) = stdin.write_all(password.as_bytes()).await {
                return Err(PodmanError::RegistryLogin(
                    "failed to write password to stdin".to_string(),
                ));
            }
            if let Err(_e) = stdin.shutdown().await {
                return Err(PodmanError::RegistryLogin(
                    "failed to close stdin".to_string(),
                ));
            }
        }

        let output = child
            .wait_with_output()
            .await
            .map_err(|e| PodmanError::RegistryLogin(e.to_string()))?;

        if !output.status.success() {
            return Err(PodmanError::RegistryLogin(format!(
                "podman login to {} failed: {}",
                registry_host,
                String::from_utf8_lossy(&output.stderr)
            )));
        }

        Ok(())
    }

    /// Pull image via queue
    pub(crate) async fn image_pull(
        &self,
        image: &str,
        registry: Option<RegistryCredentials>,
    ) -> Result<(), PodmanError> {
        let images = self.images.read().await;
        if images.contains_key(image) {
            drop(images);
            self.images
                .write()
                .await
                .insert(image.to_string(), Instant::now());
            return Ok(());
        }
        drop(images);

        let (tx, rx) = tokio::sync::oneshot::channel();
        self.pullq
            .send(PullRequest {
                respond_to: tx,
                image: image.to_string(),
                registry,
            })
            .await
            .map_err(|_| PodmanError::ImagePull("channel closed".to_string()))?;

        rx.await
            .map_err(|_| PodmanError::ImagePull("cancelled".to_string()))??;

        self.images
            .write()
            .await
            .insert(image.to_string(), Instant::now());

        Ok(())
    }

    /// Report task progress to broker
    #[allow(dead_code)]
    pub(crate) async fn report_progress(
        task_id: &str,
        progress_file: PathBuf,
        broker: Option<&(dyn Broker + Send + Sync)>,
    ) {
        loop {
            tokio::time::sleep(super::types::PROGRESS_POLL_INTERVAL).await;

            let progress = match tokio::fs::read_to_string(&progress_file).await {
                Ok(content) => {
                    let trimmed = content.trim();
                    if trimmed.is_empty() {
                        0.0
                    } else {
                        trimmed.parse().unwrap_or(0.0)
                    }
                }
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => return,
                Err(e) => {
                    error!("error reading progress file: {}", e);
                    continue;
                }
            };

            if let Some(b) = broker {
                b.publish_task_progress(task_id, progress);
            }
        }
    }

    /// Stop and remove a container
    pub(crate) async fn stop_container_static(container_id: &str) -> Result<(), PodmanError> {
        debug!("Attempting to stop and remove container {}", container_id);
        let mut cmd = Command::new("podman");
        cmd.arg("rm").arg("-f").arg("-t").arg("0").arg(container_id);
        cmd.stdout(std::process::Stdio::piped());
        cmd.stderr(std::process::Stdio::piped());

        let output = cmd.output().await.map_err(|e| {
            PodmanError::ContainerCreation(format!(
                "failed to remove container {}: {}",
                container_id, e
            ))
        })?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(PodmanError::ContainerCreation(format!(
                "failed to stop container {}: {}",
                container_id, stderr
            )));
        }
        Ok(())
    }

    /// Check Podman is running
    pub async fn health_check(&self) -> Result<(), PodmanError> {
        let mut cmd = Command::new("podman");
        cmd.arg("version");
        let output = cmd
            .output()
            .await
            .map_err(|_| PodmanError::PodmanNotRunning)?;

        if !output.status.success() {
            return Err(PodmanError::PodmanNotRunning);
        }

        Ok(())
    }
}
