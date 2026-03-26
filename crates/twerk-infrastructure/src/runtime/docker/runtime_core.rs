//! Core DockerRuntime implementation.

use std::sync::Arc;
use std::time::Duration;
use bollard::Docker;
use tokio::sync::{mpsc, RwLock};

use super::config::DockerConfig;
use super::error::DockerError;
use super::mounters::{CompositeMounter, Mounter};
use super::runtime_types::PullRequest;
use twerk_core::uuid::new_uuid;

/// Docker runtime for executing tasks in containers.
#[derive(Clone)]
pub struct DockerRuntime {
    pub client: Docker,
    config: DockerConfig,
    images: Arc<RwLock<std::collections::HashMap<String, std::time::Instant>>>,
    pull_tx: mpsc::Sender<PullRequest>,
    tasks: Arc<RwLock<usize>>,
    pruner_cancel: tokio::sync::oneshot::Sender<()>,
    mounter: Arc<dyn Mounter>,
}

impl super::DockerRuntimeTrait for DockerRuntime {
    fn run(&self, task: &twerk_core::task::Task) -> crate::runtime::BoxedFuture<()> {
        let mut task_clone = task.clone();
        let this = self.clone();
        Box::pin(async move {
            this.run(&mut task_clone).await.map_err(|e| anyhow::anyhow!(e))
        })
    }

    fn stop(&self, _task: &twerk_core::task::Task) -> crate::runtime::BoxedFuture<crate::runtime::ShutdownResult<std::process::ExitCode>> {
        Box::pin(async move {
            Ok(Ok(std::process::ExitCode::SUCCESS))
        })
    }

    fn health_check(&self) -> crate::runtime::BoxedFuture<()> {
        let this = self.clone();
        Box::pin(async move {
            this.client.ping().await.map(|_| ()).map_err(|e| anyhow::anyhow!(e))
        })
    }
}

impl DockerRuntime {
    /// Creates a new Docker runtime.
    ///
    /// # Errors
    ///
    /// Returns `DockerError` if the Docker client cannot be created.
    pub async fn new(config: DockerConfig) -> Result<Self, DockerError> {
        let client = Docker::connect_with_local_defaults()
            .map_err(|e| DockerError::ClientCreate(e.to_string()))?;

        let (pull_tx, mut pull_rx) = mpsc::channel::<PullRequest>(100);
        let (pruner_cancel_tx, mut pruner_cancel_rx) = tokio::sync::oneshot::channel::<()>();

        let images = Arc::new(RwLock::new(std::collections::HashMap::new()));
        let tasks = Arc::new(RwLock::new(0));

        // Spawn the pull worker — serializes all image pulls
        let images_clone = images.clone();
        let config_clone = config.clone();
        let pull_client = client.clone();
        tokio::spawn(async move {
            while let Some(req) = pull_rx.recv().await {
                let result = Self::do_pull_request(
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
            let mut ticker = tokio::time::interval(Duration::from_secs(60 * 60));
            loop {
                tokio::select! {
                    _ = &mut pruner_cancel_rx => break,
                    _ = ticker.tick() => {
                        Self::prune_images(
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
    pub async fn default_runtime() -> Result<Self, DockerError> {
        Self::new(DockerConfig::default()).await
    }
}
