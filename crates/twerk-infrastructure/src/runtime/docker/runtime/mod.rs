//! Docker runtime for executing tasks in containers.
//!
//! This module provides the `DockerRuntime` struct which implements the
//! `Runtime` trait for executing tasks in Docker containers.

mod container_config;
mod container_create;
mod execution;
mod image;
mod tests;
mod types;

use std::sync::Arc;

use bollard::Docker;
use dashmap::DashMap;
use tokio::sync::{mpsc, RwLock};

pub use super::config::DockerConfig;
pub use super::error::DockerError;
pub use super::mounters::{CompositeMounter, Mounter};

use super::container::Container;
use execution::run;
use image::{do_pull_request, prune_images};
use types::PullRequest;

/// Docker runtime for executing tasks in containers.
pub struct DockerRuntime {
    pub client: Docker,
    config: DockerConfig,
    images: Arc<DashMap<String, std::time::Instant>>,
    pull_tx: mpsc::Sender<PullRequest>,
    tasks: Arc<RwLock<usize>>,
    #[allow(dead_code)]
    pruner_cancel: tokio::sync::oneshot::Sender<()>,
    mounter: Arc<dyn Mounter>,
}

impl crate::runtime::Runtime for DockerRuntime {
    fn run(&self, task: &twerk_core::task::Task) -> crate::runtime::BoxedFuture<()> {
        let mut task_clone = task.clone();
        let client = self.client.clone();
        let images = Arc::clone(&self.images);
        let pull_tx = self.pull_tx.clone();
        let tasks = Arc::clone(&self.tasks);
        let config = self.config.clone();
        let mounter = Arc::clone(&self.mounter);
        let (pruner_cancel, _) = tokio::sync::oneshot::channel::<()>();

        Box::pin(async move {
            let runtime = DockerRuntime {
                client,
                config,
                images,
                pull_tx,
                tasks,
                pruner_cancel,
                mounter,
            };
            runtime
                .run(&mut task_clone)
                .await
                .map_err(|e| anyhow::anyhow!(e))
        })
    }

    fn stop(
        &self,
        _task: &twerk_core::task::Task,
    ) -> crate::runtime::BoxedFuture<crate::runtime::ShutdownResult<std::process::ExitCode>> {
        Box::pin(async move {
            // Docker runtime stop - returns success as no-op for now
            Ok(Ok(std::process::ExitCode::SUCCESS))
        })
    }

    fn health_check(&self) -> crate::runtime::BoxedFuture<()> {
        let client = self.client.clone();
        Box::pin(async move {
            client
                .ping()
                .await
                .map(|_| ())
                .map_err(|e| anyhow::anyhow!(e))
        })
    }
}

impl DockerRuntime {
    /// Creates a new Docker runtime.
    ///
    /// # Errors
    ///
    /// Returns `DockerError` if the Docker client cannot be created.
    #[allow(clippy::unused_async)]
    pub async fn new(config: DockerConfig) -> Result<Self, DockerError> {
        let client = Docker::connect_with_local_defaults()
            .map_err(|e| DockerError::ClientCreate(e.to_string()))?;

        let (pull_tx, mut pull_rx) = mpsc::channel::<PullRequest>(100);
        let (pruner_cancel_tx, mut pruner_cancel_rx) = tokio::sync::oneshot::channel::<()>();

        let images = Arc::new(DashMap::new());
        let tasks = Arc::new(RwLock::new(0));

        // Spawn the pull worker — serializes all image pulls
        let images_clone = images.clone();
        let config_clone = config.clone();
        let pull_client = client.clone();
        tokio::spawn(async move {
            while let Some(req) = pull_rx.recv().await {
                let result = do_pull_request(
                    &pull_client,
                    &images_clone,
                    &config_clone,
                    &req.image,
                    req.registry.as_ref(),
                )
                .await;
                let _ = req.result_tx.send(result);
            }
        });

        // Spawn the pruner — removes expired images every hour
        let images_prune = images.clone();
        let tasks_prune = tasks.clone();
        let config_prune = config.clone();
        let prune_client = client.clone();
        tokio::spawn(async move {
            let mut ticker = tokio::time::interval(std::time::Duration::from_secs(60 * 60));
            loop {
                tokio::select! {
                    _ = &mut pruner_cancel_rx => break,
                    _ = ticker.tick() => {
                        prune_images(
                            &prune_client,
                            &images_prune,
                            &tasks_prune,
                            config_prune.image_ttl,
                        )
                        .await;
                    }
                }
            }
        });

        // Create composite mounter for bind, tmpfs, and volume mounts
        let mounter: Arc<dyn Mounter> = Arc::new(CompositeMounter::new(client.clone()));

        Ok(Self {
            client,
            config,
            images,
            pull_tx,
            tasks,
            pruner_cancel: pruner_cancel_tx,
            mounter,
        })
    }

    /// Creates a new runtime with default configuration.
    ///
    /// # Errors
    ///
    /// Returns `DockerError` if the runtime cannot be created.
    pub async fn default_runtime() -> Result<Self, DockerError> {
        Self::new(DockerConfig::default()).await
    }

    /// Runs a task in a Docker container.
    ///
    /// # Errors
    ///
    /// Returns `DockerError` if the task cannot be executed.
    pub async fn run(&self, task: &mut twerk_core::task::Task) -> Result<(), DockerError> {
        run(
            self.client.clone(),
            self.config.clone(),
            self.images.clone(),
            self.pull_tx.clone(),
            self.tasks.clone(),
            self.mounter.clone(),
            task,
        )
        .await
    }

    /// Health check on the Docker daemon.
    ///
    /// # Errors
    ///
    /// Returns `DockerError` if the health check fails.
    pub async fn health_check(&self) -> Result<(), DockerError> {
        self.client
            .ping()
            .await
            .map(|_| ())
            .map_err(|e| DockerError::ClientCreate(e.to_string()))
    }

    /// Creates a container for a task.
    ///
    /// # Errors
    ///
    /// Returns `DockerError` if the container cannot be created.
    ///
    /// Go parity: `createTaskContainer` — full lifecycle setup including
    /// image pull, env, mounts, limits, GPU, probe ports, networking aliases,
    /// workdir, and file initialization.
    #[allow(dead_code)] // used in integration tests
    pub async fn create_container(
        &self,
        task: &twerk_core::task::Task,
    ) -> Result<Container, DockerError> {
        container_create::create_container(
            &self.client,
            &self.config,
            &self.pull_tx,
            &self.mounter,
            task,
        )
        .await
    }
}
