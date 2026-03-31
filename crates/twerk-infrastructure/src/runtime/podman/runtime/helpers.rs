//! Helper functions for `PodmanRuntime`.

use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;
use std::time::Duration;

use tokio::process::Command;
use tokio::sync::RwLock;
use tracing::{debug, error};

use super::super::errors::PodmanError;
use super::super::types::{Broker, PROGRESS_POLL_INTERVAL};
use super::types::PodmanRuntime;

impl PodmanRuntime {
    /// Stop and remove container
    pub(crate) async fn stop_container_static(container_id: &str) -> Result<(), PodmanError> {
        debug!("Attempting to stop and remove container {}", container_id);
        let mut cmd = Command::new("podman");
        cmd.arg("rm").arg("-f").arg("-t").arg("0").arg(container_id);
        cmd.stdout(std::process::Stdio::piped());
        cmd.stderr(std::process::Stdio::piped());

        let output = cmd.output().await.map_err(|e| {
            PodmanError::ContainerCreation(format!(
                "failed to remove container {container_id}: {e}"
            ))
        })?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(PodmanError::ContainerCreation(format!(
                "failed to stop container {container_id}: {stderr}"
            )));
        }
        Ok(())
    }

    /// Report progress to broker
    pub(crate) async fn report_progress(
        task_id: &str,
        progress_file: &Path,
        broker: Option<&(dyn Broker + Send + Sync)>,
    ) {
        loop {
            tokio::time::sleep(PROGRESS_POLL_INTERVAL).await;

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

    /// Prune stale images
    pub(crate) async fn prune_images(
        images: &Arc<RwLock<HashMap<String, std::time::Instant>>>,
        active_tasks: &Arc<std::sync::atomic::AtomicU64>,
        ttl: Duration,
    ) -> Result<(), PodmanError> {
        if active_tasks.load(std::sync::atomic::Ordering::SeqCst) > 0 {
            return Ok(());
        }

        let images_guard = images.read().await;
        let stale: Vec<String> = images_guard
            .iter()
            .filter(|(_img, last_used)| last_used.elapsed() > ttl)
            .map(|(img, _)| img.clone())
            .collect();
        drop(images_guard);

        for image in &stale {
            let mut cmd = Command::new("podman");
            cmd.arg("image").arg("rm").arg(image);
            cmd.stdout(std::process::Stdio::null());
            cmd.stderr(std::process::Stdio::null());

            if let Ok(output) = cmd.output().await {
                if output.status.success() {
                    debug!("pruned image {}", image);
                    images.write().await.remove(image);
                }
            }
        }

        Ok(())
    }

    /// Health check - verify podman is running
    pub(crate) async fn health_check_inner(&self) -> Result<(), PodmanError> {
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
