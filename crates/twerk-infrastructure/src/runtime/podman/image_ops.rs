//! Image operations for Podman runtime

use std::sync::Arc;
use std::time::{Duration, Instant};

use tokio::process::Command;
use tokio::sync::RwLock;
use tracing::{debug, info};

use super::errors::PodmanError;
use super::types::{PodmanRuntime};

#[allow(dead_code)]
impl PodmanRuntime {
    /// Verify an image can be used to create a container
    pub(crate) async fn verify_image(image: &str) -> Result<(), PodmanError> {
        info!("verifying image {}", image);

        let mut create_cmd = Command::new("podman");
        create_cmd.arg("create").arg(image).arg("true");
        create_cmd.stdout(std::process::Stdio::piped());
        create_cmd.stderr(std::process::Stdio::piped());

        let create_output = create_cmd
            .output()
            .await
            .map_err(|e| PodmanError::ImageVerification(e.to_string()))?;

        if !create_output.status.success() {
            return Err(PodmanError::ImageVerification(format!(
                "image {} failed verification: {}",
                image,
                String::from_utf8_lossy(&create_output.stderr)
            )));
        }

        let container_id = String::from_utf8_lossy(&create_output.stdout)
            .trim()
            .to_string();

        if container_id.is_empty() {
            return Err(PodmanError::ImageVerification(
                "empty container ID during verification".to_string(),
            ));
        }

        let mut rm_cmd = Command::new("podman");
        rm_cmd.arg("rm").arg("-f").arg(&container_id);
        let _ = rm_cmd.output().await;

        info!("image {} verified successfully", image);
        Ok(())
    }

    /// Check if an image exists locally
    pub(crate) async fn image_exists_locally(image: &str) -> bool {
        let output = Command::new("podman")
            .arg("inspect")
            .arg(image)
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .output()
            .await;
        output.is_ok_and(|out| out.status.success())
    }

    /// Extract registry host from image name
    pub(crate) fn extract_registry_host(image: &str) -> String {
        match image.split_once('/') {
            Some((host, _rest)) if host.contains('.') || host.contains(':') => {
                host.to_string()
            }
            _ => "docker.io".to_string(),
        }
    }

    /// Prune stale images based on TTL
    pub(crate) async fn prune_images(
        images: &Arc<RwLock<std::collections::HashMap<String, Instant>>>,
        active_tasks: &Arc<std::sync::atomic::AtomicU64>,
        ttl: Duration,
    ) -> Result<(), anyhow::Error> {
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
}
