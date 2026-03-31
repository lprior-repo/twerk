//! Image management logic for `PodmanRuntime`.

use std::time::Instant;

use tokio::io::AsyncWriteExt;
use tokio::process::Command;

use super::super::errors::PodmanError;
use super::super::types::{Broker, PullRequest, RegistryCredentials};
use super::types::PodmanRuntime;

impl PodmanRuntime {
    /// Pull image via queue
    pub(crate) async fn image_pull(
        &self,
        image: &str,
        registry: Option<RegistryCredentials>,
    ) -> Result<(), PodmanError> {
        // Check cache
        {
            let images = self.images.read().await;
            if images.contains_key(image) {
                drop(images);
                self.images
                    .write()
                    .await
                    .insert(image.to_string(), Instant::now());
                return Ok(());
            }
        }

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

    /// Verify image can be used
    pub(crate) async fn verify_image(image: &str) -> Result<(), PodmanError> {
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

        Ok(())
    }

    /// Internal pull implementation
    pub(crate) async fn do_pull_request(
        image: &str,
        registry: Option<RegistryCredentials>,
        _broker: Option<&(dyn Broker + Send + Sync)>,
    ) -> Result<(), PodmanError> {
        // Check if image exists locally
        if Self::image_exists_locally(image).await {
            tracing::debug!("image {} already exists locally, skipping pull", image);
            return Ok(());
        }

        // Login to registry if credentials provided
        if let Some(ref creds) = registry {
            if !creds.username.is_empty() {
                Self::registry_login(image, &creds.username, &creds.password).await?;
            }
        }

        // Pull image
        tracing::debug!("Pulling image {}", image);
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

    /// Check if image exists locally
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

    /// Login to registry
    pub(crate) async fn registry_login(
        image: &str,
        username: &str,
        password: &str,
    ) -> Result<(), PodmanError> {
        let registry_host = Self::extract_registry_host(image);
        tracing::debug!(
            "Logging into registry {} for user {}",
            registry_host,
            username
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

    /// Extract registry host from image name
    pub(crate) fn extract_registry_host(image: &str) -> String {
        match image.split_once('/') {
            Some((host, _rest)) if host.contains('.') || host.contains(':') => host.to_string(),
            _ => "docker.io".to_string(),
        }
    }
}
