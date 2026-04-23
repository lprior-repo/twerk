//! `PodmanRuntime` struct and core types.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use tokio::sync::{mpsc, RwLock};

use super::super::types::{Broker, Mounter, PullRequest};
use twerk_common::constants::CHANNEL_BUFFER_SIZE;

/// Podman runtime for executing tasks via podman CLI.
pub struct PodmanRuntime {
    pub(crate) broker: Option<Box<dyn Broker + Send + Sync>>,
    pub(crate) pullq: mpsc::Sender<PullRequest>,
    pub(crate) images: Arc<RwLock<HashMap<String, std::time::Instant>>>,
    pub(crate) tasks: Arc<RwLock<HashMap<String, String>>>,
    pub(crate) active_tasks: Arc<std::sync::atomic::AtomicU64>,
    pub(crate) mounter: Arc<dyn Mounter + Send + Sync>,
    pub(crate) privileged: bool,
    pub(crate) host_network: bool,
    pub(crate) image_verify: bool,
    pub(crate) image_ttl: Duration,
}

#[allow(clippy::missing_fields_in_debug)]
impl std::fmt::Debug for PodmanRuntime {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PodmanRuntime")
            .field("broker", &"<broker>")
            .field("privileged", &self.privileged)
            .field("host_network", &self.host_network)
            .field("mounter", &"<mounter>")
            .field("image_verify", &self.image_verify)
            .field("image_ttl", &self.image_ttl)
            .finish_non_exhaustive()
    }
}

impl Clone for PodmanRuntime {
    fn clone(&self) -> Self {
        Self {
            broker: self.broker.clone(),
            pullq: self.pullq.clone(),
            images: Arc::clone(&self.images),
            tasks: Arc::clone(&self.tasks),
            active_tasks: Arc::clone(&self.active_tasks),
            mounter: self.mounter.clone(),
            privileged: self.privileged,
            host_network: self.host_network,
            image_verify: self.image_verify,
            image_ttl: self.image_ttl,
        }
    }
}

impl PodmanRuntime {
    /// Creates a new `PodmanRuntime` from configuration.
    #[must_use]
    pub fn new(config: super::PodmanConfig) -> Self {
        let (tx, rx) = mpsc::channel::<PullRequest>(CHANNEL_BUFFER_SIZE);
        let mounter: Arc<dyn Mounter + Send + Sync> = if let Some(m) = config.mounter {
            // Convert Box<dyn Mounter> to Arc<dyn Mounter> via Arc::from(Box).
            Arc::from(m)
        } else {
            Arc::new(super::super::volume::VolumeMounter::new())
        };
        let image_ttl = config
            .image_ttl
            .unwrap_or(super::super::types::DEFAULT_IMAGE_TTL);

        let images = Arc::new(RwLock::new(HashMap::new()));
        let tasks = Arc::new(RwLock::new(HashMap::new()));
        let active_tasks = Arc::new(std::sync::atomic::AtomicU64::new(0));

        Self::start_puller(rx, config.broker.clone());
        Self::start_pruner(images.clone(), active_tasks.clone(), image_ttl);

        Self {
            broker: config.broker,
            pullq: tx,
            images,
            tasks,
            active_tasks,
            mounter,
            privileged: config.privileged,
            host_network: config.host_network,
            image_verify: config.image_verify,
            image_ttl,
        }
    }

    fn start_puller(
        mut rx: mpsc::Receiver<PullRequest>,
        broker: Option<Box<dyn Broker + Send + Sync>>,
    ) {
        tokio::spawn(async move {
            while let Some(pr) = rx.recv().await {
                let image = pr.image.clone();
                let registry = pr.registry.clone();
                let result = Self::do_pull_request(&image, registry, broker.as_deref()).await;
                let _ = pr.respond_to.send(result);
            }
        });
    }

    fn start_pruner(
        images: Arc<RwLock<HashMap<String, std::time::Instant>>>,
        active_tasks: Arc<std::sync::atomic::AtomicU64>,
        ttl: Duration,
    ) {
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(super::super::types::PRUNE_INTERVAL);
            loop {
                interval.tick().await;
                if let Err(e) = Self::prune_images(&images, &active_tasks, ttl).await {
                    tracing::error!("error pruning images: {}", e);
                }
            }
        });
    }
}
