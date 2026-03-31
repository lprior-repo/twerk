//! Tcontainer - Docker container wrapper for task execution.
//!
//! Ported from Go tcontainer.go. Provides container lifecycle management
//! for task execution with tork directory support.

use super::archive::{init_runtime_dir, upload_files_to_container};
use super::monitoring::{read_logs_tail, read_output_file};
use super::probe::probe_if_configured;
use crate::broker::Broker;
use crate::runtime::docker::error::DockerError;
use crate::runtime::docker::mounters::Mounter;
use bollard::query_parameters::{RemoveContainerOptions, RemoveVolumeOptions};
use bollard::Docker;
use std::sync::Arc;
use twerk_core::task::Task;

/// Tcontainer is the Docker container wrapper for task execution.
/// Ported from Go tcontainer struct.
pub struct Tcontainer {
    pub id: String,
    pub client: Docker,
    pub mounter: Arc<dyn Mounter>,
    pub broker: Arc<dyn Broker>,
    pub task: Task,
    pub logger: Box<dyn std::io::Write + Send + Sync>,
    pub torkdir: twerk_core::mount::Mount,
}

impl Tcontainer {
    /// Creates a new Tcontainer instance.
    #[must_use]
    pub fn new(
        id: String,
        client: Docker,
        mounter: Arc<dyn Mounter>,
        broker: Arc<dyn Broker>,
        task: Task,
        logger: Box<dyn std::io::Write + Send + Sync>,
        torkdir: twerk_core::mount::Mount,
    ) -> Self {
        Self {
            id,
            client,
            mounter,
            broker,
            task,
            logger,
            torkdir,
        }
    }

    /// Starts the container and waits for the probe to be ready.
    ///
    /// # Errors
    /// - `DockerError::ContainerStart` if container fails to start
    /// - `DockerError::ProbeError` or `DockerError::ProbeTimeout` if probe fails
    pub async fn start(&self) -> Result<(), DockerError> {
        tracing::debug!(container_id = %self.id, "Starting container");

        self.client
            .start_container(&self.id, None)
            .await
            .map_err(|e| DockerError::ContainerStart(format!("{}: {}", self.id, e)))?;

        probe_if_configured(&self.client, &self.id, self.task.probe.as_ref()).await?;

        Ok(())
    }

    /// Removes the container and cleans up resources.
    ///
    /// # Errors
    /// - `DockerError::ContainerRemove` if container removal fails
    /// - `DockerError::VolumeRemove` if volume removal fails
    /// - `DockerError::Unmount` if unmount fails
    pub async fn remove(&self) -> Result<(), DockerError> {
        tracing::debug!(container_id = %self.id, "Removing container");

        self.client
            .remove_container(
                &self.id,
                Some(RemoveContainerOptions {
                    force: true,
                    ..Default::default()
                }),
            )
            .await
            .map_err(|e| DockerError::ContainerRemove(e.to_string()))?;

        if let Some(ref source) = self.torkdir.source {
            self.client
                .remove_volume(source, Some(RemoveVolumeOptions { force: true }))
                .await
                .map_err(|e| DockerError::VolumeRemove(e.to_string()))?;
        }

        self.mounter
            .unmount(&self.torkdir)
            .await
            .map_err(DockerError::Unmount)?;

        Ok(())
    }

    /// Waits for the container to complete and returns the stdout.
    ///
    /// # Errors
    /// - `DockerError::ContainerWait` if wait fails
    /// - `DockerError::NonZeroExit` if container exits with non-zero status
    /// - `DockerError::CopyFromContainer` if reading output fails
    pub async fn wait(&self) -> Result<String, DockerError> {
        use bollard::query_parameters::WaitContainerOptions;
        use futures_util::StreamExt;

        let options = WaitContainerOptions {
            condition: "not-running".to_string(),
        };

        let result = self
            .client
            .wait_container(&self.id, Some(options))
            .next()
            .await
            .ok_or_else(|| DockerError::ContainerWait("no wait result".to_string()))?
            .map_err(|e| DockerError::ContainerWait(e.to_string()))?;

        let status_code: i64 = result.status_code;

        if status_code != 0 {
            return Err(DockerError::NonZeroExit(
                status_code,
                self.read_logs_tail(10)
                    .await
                    .unwrap_or_else(|_| String::new()),
            ));
        }

        let stdout = self.read_output().await?;
        tracing::debug!(status_code, task_id = ?self.task.id, "task completed");

        Ok(stdout)
    }

    /// Reads the last N lines of container logs.
    async fn read_logs_tail(&self, lines: usize) -> Result<String, DockerError> {
        read_logs_tail(&self.client, &self.id, lines).await
    }

    /// Reads the output file from the container.
    async fn read_output(&self) -> Result<String, DockerError> {
        read_output_file(&self.client, &self.id, "/tork/stdout").await
    }

    /// Initializes the tork directory in the container.
    ///
    /// # Errors
    /// - `DockerError::CopyToContainer` if upload fails
    pub async fn init_torkdir(&self) -> Result<(), DockerError> {
        let run_script = self.task.run.as_deref();
        init_runtime_dir(&self.client, &self.id, run_script, "/twerk/").await
    }

    /// Initializes the work directory in the container.
    ///
    /// # Errors
    /// - `DockerError::CopyToContainer` if upload fails
    pub async fn init_workdir(&self) -> Result<(), DockerError> {
        let files = match &self.task.files {
            Some(f) => f,
            None => return Ok(()),
        };

        if files.is_empty() {
            return Ok(());
        }

        let workdir = self.task.workdir.as_deref().unwrap_or("/workspace");
        upload_files_to_container(&self.client, &self.id, files, workdir).await
    }
}
