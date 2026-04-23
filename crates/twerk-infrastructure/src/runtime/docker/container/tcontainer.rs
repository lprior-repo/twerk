//! Tcontainer - Docker container wrapper for task execution.
//!
//! Ported from Go tcontainer.go. Provides container lifecycle management
//! for task execution with tork directory support.

use std::collections::HashMap;
use std::sync::Arc;

use bollard::query_parameters::{RemoveContainerOptions, RemoveVolumeOptions};
use bollard::Docker;

use super::archive::{init_runtime_dir, upload_files_to_container};
use super::monitoring::{read_logs_tail, read_output_file};
use super::probe::probe_if_configured;
use crate::broker::Broker;
use crate::runtime::docker::config::{
    COPY_FROM_CONTAINER_EMPTY, ERROR_PUBLISHING_TASK_PROGRESS, INVALID_PROGRESS,
    PROGRESS_POLL_INTERVAL,
};
use crate::runtime::docker::error::DockerError;
use crate::runtime::docker::mounters::Mounter;
use twerk_core::id::TaskId;
use twerk_core::task::{Probe, Task};

/// Tcontainer is the Docker container wrapper for task execution.
/// Ported from Go tcontainer struct.
pub struct Tcontainer {
    /// Container ID.
    pub id: String,
    /// Docker client.
    pub client: Docker,
    /// Mount manager for bind/tmpfs mounts.
    pub mounter: Arc<dyn Mounter>,
    /// Broker for log shipping and progress. May be None if not configured.
    pub broker: Option<Arc<dyn Broker>>,
    /// Task being executed in this container.
    pub task: Task,
    /// Logger for task output.
    pub logger: Box<dyn std::io::Write + Send + Sync>,
    /// Twerk directory mount info.
    pub torkdir: twerk_core::mount::Mount,
    /// Source volume name for twerk directory. Used for cleanup.
    pub twerkdir_source: Option<String>,
    /// Task ID extracted from task for monitoring.
    pub task_id: TaskId,
    /// Probe configuration for health checking.
    pub probe: Option<Probe>,
}

impl Tcontainer {
    /// Creates a new Tcontainer instance.
    #[must_use]
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        id: String,
        client: Docker,
        mounter: Arc<dyn Mounter>,
        broker: Option<Arc<dyn Broker>>,
        task: Task,
        logger: Box<dyn std::io::Write + Send + Sync>,
        torkdir: twerk_core::mount::Mount,
        twerkdir_source: Option<String>,
        task_id: TaskId,
        probe: Option<Probe>,
    ) -> Self {
        Self {
            id,
            client,
            mounter,
            broker,
            task,
            logger,
            torkdir,
            twerkdir_source,
            task_id,
            probe,
        }
    }

    /// Starts monitoring tasks (log streaming and progress reporting).
    pub fn start_monitoring(&self) {
        let progress_client = self.client.clone();
        let progress_id = self.id.clone();
        let progress_task_id = self.task_id.clone();
        let progress_broker = self.broker.clone();
        tokio::spawn(async move {
            Self::report_progress(
                progress_client,
                progress_id,
                progress_task_id,
                progress_broker,
            )
            .await;
        });

        let log_client = self.client.clone();
        let log_id = self.id.clone();
        let log_task_id = self.task_id.clone();
        let log_broker = self.broker.clone();
        tokio::spawn(async move {
            Self::stream_logs(log_client, log_id, log_task_id, log_broker).await;
        });
    }

    /// Reports task progress periodically to the broker.
    async fn report_progress(
        client: Docker,
        container_id: String,
        task_id: TaskId,
        broker: Option<Arc<dyn Broker>>,
    ) {
        let Some(broker) = broker else {
            return;
        };
        let mut tick = tokio::time::interval(PROGRESS_POLL_INTERVAL);
        let mut prev: Option<f64> = None;
        loop {
            tokio::select! {
                _ = tick.tick() => {
                    match Self::read_progress_value(&client, &container_id).await {
                        Ok(p) if prev.is_none_or(|old| (old - p).abs() > 0.001) => {
                            prev = Some(p);
                            let twerk_task = Task {
                                id: Some(task_id.clone()),
                                progress: p,
                                ..Default::default()
                            };
                            if let Err(e) = broker.publish_task_progress(&twerk_task).await {
                                tracing::warn!(task_id = %task_id, error = %e, ERROR_PUBLISHING_TASK_PROGRESS);
                            }
                        }
                        Err(_) => break,
                        _ => {}
                    }
                }
            }
        }
    }

    /// Streams container logs to the broker.
    async fn stream_logs(
        client: Docker,
        container_id: String,
        task_id: TaskId,
        broker: Option<Arc<dyn Broker>>,
    ) {
        use bollard::query_parameters::LogsOptions;
        use futures_util::StreamExt;

        let Some(broker) = broker else {
            return;
        };

        let options = LogsOptions {
            stdout: true,
            stderr: true,
            follow: true,
            tail: "all".to_string(),
            ..Default::default()
        };

        let mut stream = client.logs(&container_id, Some(options));
        let mut part_num = 0i64;

        while let Some(result) = stream.next().await {
            if let Ok(
                bollard::container::LogOutput::StdOut { message }
                | bollard::container::LogOutput::StdErr { message },
            ) = result
            {
                let msg = String::from_utf8_lossy(message.as_ref()).to_string();
                if !msg.is_empty() {
                    part_num += 1;
                    let _ = broker
                        .publish_task_log_part(&twerk_core::task::TaskLogPart {
                            id: None,
                            number: part_num,
                            task_id: Some(task_id.clone()),
                            contents: Some(msg),
                            created_at: None,
                        })
                        .await;
                }
            }
        }
    }

    /// Reads the progress value from the container's /twerk/progress file.
    async fn read_progress_value(client: &Docker, cid: &str) -> Result<f64, DockerError> {
        use bollard::query_parameters::DownloadFromContainerOptions;
        use futures_util::StreamExt;

        let options = DownloadFromContainerOptions {
            path: "/twerk/progress".to_string(),
        };

        let mut stream = client.download_from_container(cid, Some(options));

        let bytes = stream
            .next()
            .await
            .ok_or_else(|| DockerError::CopyFromContainer(COPY_FROM_CONTAINER_EMPTY.to_string()))?
            .map_err(|e| DockerError::CopyFromContainer(e.to_string()))?;

        let contents = crate::runtime::docker::helpers::parse_tar_contents(&bytes);
        let s = contents.trim();

        if s.is_empty() {
            return Ok(0.0);
        }

        s.parse::<f64>()
            .map_err(|_| DockerError::CopyFromContainer(INVALID_PROGRESS.to_string()))
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
            for attempt in 0..3 {
                if attempt > 0 {
                    tokio::time::sleep(std::time::Duration::from_millis(
                        100 * 2u64.pow(attempt - 1),
                    ))
                    .await;
                }
                match self
                    .client
                    .remove_volume(source, Some(RemoveVolumeOptions { force: true }))
                    .await
                {
                    Ok(()) => break,
                    Err(e) if attempt < 2 => {
                        tracing::debug!(error = %e, volume = %source, attempt, "retrying volume removal");
                    }
                    Err(e) => {
                        return Err(DockerError::VolumeRemove(e.to_string()));
                    }
                }
            }
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
        let status_code = self.wait_for_container_stopped().await?;

        if status_code != 0 {
            return Err(DockerError::NonZeroExit(
                status_code,
                self.read_logs_tail(10)
                    .await
                    .map_or_else(|_| String::new(), std::convert::identity),
            ));
        }

        let stdout = self.read_output().await?;
        tracing::debug!(status_code, task_id = ?self.task.id, "task completed");

        Ok(stdout)
    }

    /// Waits for the container to reach the "not-running" state and returns the exit code.
    async fn wait_for_container_stopped(&self) -> Result<i64, DockerError> {
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

        Ok(result.status_code)
    }

    /// Reads the last N lines of container logs.
    async fn read_logs_tail(&self, lines: usize) -> Result<String, DockerError> {
        read_logs_tail(&self.client, &self.id, lines).await
    }

    /// Reads the output file from the container.
    async fn read_output(&self) -> Result<String, DockerError> {
        read_output_file(&self.client, &self.id, "/twerk/stdout").await
    }

    /// Initializes the twerk directory in the container.
    ///
    /// # Errors
    /// - `DockerError::CopyToContainer` if upload fails
    pub async fn init_twerkdir(&self, run_script: Option<&str>) -> Result<(), DockerError> {
        init_runtime_dir(&self.client, &self.id, run_script, "/twerk/").await
    }

    /// Initializes the work directory in the container.
    ///
    /// # Errors
    /// - `DockerError::CopyToContainer` if upload fails
    pub async fn init_workdir(
        &self,
        files: &HashMap<String, String>,
        workdir: &str,
    ) -> Result<(), DockerError> {
        if files.is_empty() {
            return Ok(());
        }

        upload_files_to_container(&self.client, &self.id, files, workdir).await
    }
}
