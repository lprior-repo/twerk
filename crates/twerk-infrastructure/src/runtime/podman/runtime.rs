//! Podman runtime core

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use tokio::sync::mpsc;
use tokio::sync::RwLock;
use tracing::{error, warn};

use super::errors::PodmanError;
use super::types::{Broker, Mount, PodmanConfig, PodmanRuntime, Task};
use super::types::{PullRequest};
use crate::runtime::podman::volume::VolumeMounter;

// ── Container Guard ────────────────────────────────────────────

/// Guard that ensures container cleanup on function exit (success or error).
/// This mimics Go's defer pattern for guaranteed cleanup.
/// The guard is disarmed via `disarm()` once cleanup is done normally.
pub(crate) struct ContainerGuard {
    container_id: String,
    tasks: Arc<RwLock<HashMap<String, String>>>,
    disarmed: bool,
}

impl ContainerGuard {
    pub(crate) fn new(container_id: String, tasks: Arc<RwLock<HashMap<String, String>>>) -> Self {
        Self {
            container_id,
            tasks,
            disarmed: false,
        }
    }

    pub(crate) fn disarm(&mut self) {
        self.disarmed = true;
    }
}

impl Drop for ContainerGuard {
    fn drop(&mut self) {
        if self.disarmed {
            return;
        }
        let cid = self.container_id.clone();
        let tasks = self.tasks.clone();
        tokio::spawn(async move {
            if let Err(e) = PodmanRuntime::stop_container_static(&cid).await {
                warn!("error stopping container {} in guard drop: {}", cid, e);
            }
            let _ = tasks.write().await.remove(&cid);
        });
    }
}

impl PodmanRuntime {
    pub fn new(config: PodmanConfig) -> Self {
        let (tx, rx) = mpsc::channel::<PullRequest>(100);
        let mounter = config
            .mounter
            .unwrap_or_else(|| Box::new(VolumeMounter::new()));
        let image_ttl = config.image_ttl.unwrap_or(super::types::DEFAULT_IMAGE_TTL);

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
                let result = Self::do_pull_request(&image, registry, broker.as_ref()).await;
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
            let mut interval = tokio::time::interval(super::types::PRUNE_INTERVAL);
            loop {
                interval.tick().await;
                if let Err(e) = Self::prune_images(&images, &active_tasks, ttl).await {
                    error!("error pruning images: {}", e);
                }
            }
        });
    }

    pub async fn run(&self, task: &mut Task) -> Result<(), PodmanError> {
        if task.id.is_empty() {
            return Err(PodmanError::TaskIdRequired);
        }
        if task.image.is_empty() {
            return Err(PodmanError::ImageRequired);
        }
        if task.name.as_ref().is_none_or(|n| n.is_empty()) {
            if !task.networks.is_empty() {
                return Err(PodmanError::NameRequiredForNetwork);
            }
            return Err(PodmanError::NameRequired);
        }
        if !task.sidecars.is_empty() {
            return Err(PodmanError::SidecarsNotSupported);
        }

        let mut mounted_mounts: Vec<Mount> = Vec::new();
        for mut mount in task.mounts.clone() {
            if let Err(e) = self.mounter.mount(&mut mount) {
                error!("error mounting: {}", e);
                return Err(PodmanError::WorkdirCreation(e.to_string()));
            }
            mounted_mounts.push(mount);
        }

        let mounter = &self.mounter;
        let result = self.run_inner(task, &mounted_mounts).await;

        for mount in &mounted_mounts {
            if let Err(e) = mounter.unmount(mount) {
                error!("error unmounting volume {}: {}", mount.target, e);
            }
        }

        result
    }

    async fn run_inner(&self, task: &mut Task, mounts: &[Mount]) -> Result<(), PodmanError> {
        let task_mounts = mounts.to_vec();
        task.mounts = task_mounts.clone();

        for pre in task.pre.iter_mut() {
            pre.id = uuid::Uuid::new_v4().to_string();
            pre.mounts = task_mounts.clone();
            pre.networks = task.networks.clone();
            pre.limits = task.limits.clone();
            self.do_run(pre).await?;
        }

        self.do_run(task).await?;

        for post in task.post.iter_mut() {
            post.id = uuid::Uuid::new_v4().to_string();
            post.mounts = task_mounts.clone();
            post.networks = task.networks.clone();
            post.limits = task.limits.clone();
            self.do_run(post).await?;
        }

        Ok(())
    }

    async fn do_run(&self, task: &mut Task) -> Result<(), PodmanError> {
        self.active_tasks
            .fetch_add(1, std::sync::atomic::Ordering::SeqCst);

        let result = self.do_run_inner(task).await;

        self.active_tasks
            .fetch_sub(1, std::sync::atomic::Ordering::SeqCst);

        result
    }
}
