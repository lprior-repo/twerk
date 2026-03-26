//! Docker runtime implementation following functional-rust conventions.
//!
//! # Architecture
//!
//! - **Data**: `DockerRuntime` holds Docker client and configuration state
//! - **Calc**: Pure parsing and validation logic
//! - **Actions**: Docker API calls, I/O pushed to boundary

use std::collections::HashMap;
use std::pin::Pin;
use std::sync::Arc;
use std::time::Duration;

use bollard::models::{
    ContainerCreateBody as BollardConfig, HostConfig, Mount as BollardMount, MountTypeEnum,
    NetworkCreateRequest,
};
use bollard::query_parameters::{
    CreateImageOptions, RemoveContainerOptions, WaitContainerOptions,
};
use bollard::Docker;
use futures_util::StreamExt;
use tokio::sync::{mpsc, RwLock};

use crate::runtime::docker::bind::{BindConfig, BindMounter};
use crate::runtime::docker::tmpfs::TmpfsMounter;
use crate::runtime::docker::volume::VolumeMounter;
use thiserror::Error;
use twerk_core::id::TaskId;
use twerk_core::mount::Mount;
use twerk_core::mount::{MOUNT_TYPE_BIND, MOUNT_TYPE_TMPFS, MOUNT_TYPE_VOLUME};
use twerk_core::task::{Probe, Registry, Task as TwerkTask, TaskLimits};
use twerk_core::uuid::new_uuid;

// ----------------------------------------------------------------------------
// Constants
// ----------------------------------------------------------------------------

/// Default workdir for task files.
const DEFAULT_WORKDIR: &str = "/twerk/workdir";

/// Default image TTL (3 days).
const DEFAULT_IMAGE_TTL: Duration = Duration::from_secs(72 * 60 * 60);

/// Default probe path.
const DEFAULT_PROBE_PATH: &str = "/";

/// Default probe timeout.
const DEFAULT_PROBE_TIMEOUT: &str = "1m";

/// Default command when none specified (uses /twerk/entrypoint script).
const DEFAULT_CMD: &[&str] = &["/twerk/entrypoint"];

/// Default entrypoint for `run` scripts.
const RUN_ENTRYPOINT: &[&str] = &["sh", "-c"];

// ----------------------------------------------------------------------------
// Configuration
// ----------------------------------------------------------------------------

/// Docker runtime configuration.
#[derive(Clone)]
pub struct DockerConfig {
    /// Docker config file path for registry credentials.
    pub config_file: Option<String>,
    /// Docker config path for registry credentials (alternative to config_file).
    pub config_path: Option<String>,
    /// Whether to run containers in privileged mode.
    pub privileged: bool,
    /// Image TTL for pruning.
    pub image_ttl: Duration,
    /// Whether to verify image integrity.
    pub image_verify: bool,
    /// Broker for log shipping and progress.
    pub broker: Option<Arc<dyn crate::broker::Broker>>,
    /// Whether to allow host network mode for containers.
    pub host_network: bool,
}

impl Default for DockerConfig {
    fn default() -> Self {
        Self {
            config_file: None,
            config_path: None,
            privileged: false,
            image_ttl: DEFAULT_IMAGE_TTL,
            image_verify: false,
            broker: None,
            host_network: false,
        }
    }
}

/// Builder for Docker runtime configuration.
#[derive(Default)]
pub struct DockerConfigBuilder {
    config_file: Option<String>,
    config_path: Option<String>,
    privileged: bool,
    image_ttl: Duration,
    image_verify: bool,
    broker: Option<Arc<dyn crate::broker::Broker>>,
    host_network: bool,
}

impl DockerConfigBuilder {
    #[must_use]
    pub fn with_image_ttl(mut self, ttl: Duration) -> Self {
        self.image_ttl = ttl;
        self
    }

    #[must_use]
    pub fn with_privileged(mut self, privileged: bool) -> Self {
        self.privileged = privileged;
        self
    }

    #[must_use]
    pub fn with_image_verify(mut self, verify: bool) -> Self {
        self.image_verify = verify;
        self
    }

    #[must_use]
    pub fn with_config_file(mut self, path: &str) -> Self {
        self.config_file = Some(path.to_string());
        self
    }

    #[must_use]
    pub fn build(self) -> DockerConfig {
        DockerConfig {
            config_file: self.config_file,
            config_path: self.config_path,
            privileged: self.privileged,
            image_ttl: self.image_ttl,
            image_verify: self.image_verify,
            broker: self.broker,
            host_network: self.host_network,
        }
    }
}

// ----------------------------------------------------------------------------
// Domain Errors
// ----------------------------------------------------------------------------

/// Errors from Docker runtime operations.
#[derive(Debug, Error)]
pub enum DockerError {
    #[error("failed to create Docker client: {0}")]
    ClientCreate(String),

    #[error("task ID is required")]
    TaskIdRequired,

    #[error("volume target is required")]
    VolumeTargetRequired,

    #[error("bind target is required")]
    BindTargetRequired,

    #[error("bind source is required")]
    BindSourceRequired,

    #[error("unknown mount type: {0}")]
    UnknownMountType(String),

    #[error("error pulling image: {0}")]
    ImagePull(String),

    #[error("error creating container: {0}")]
    ContainerCreate(String),

    #[error("error starting container: {0}")]
    ContainerStart(String),

    #[error("error waiting for container: {0}")]
    ContainerWait(String),

    #[error("error getting logs: {0}")]
    ContainerLogs(String),

    #[error("error removing container: {0}")]
    ContainerRemove(String),

    #[error("error mounting: {0}")]
    Mount(String),

    #[error("error unmounting: {0}")]
    Unmount(String),

    #[error("error creating network: {0}")]
    NetworkCreate(String),

    #[error("error removing network: {0}")]
    NetworkRemove(String),

    #[error("error creating volume: {0}")]
    VolumeCreate(String),

    #[error("error removing volume: {0}")]
    VolumeRemove(String),

    #[error("error copying files to container: {0}")]
    CopyToContainer(String),

    #[error("error copying files from container: {0}")]
    CopyFromContainer(String),

    #[error("error inspecting container: {0}")]
    ContainerInspect(String),

    #[error("invalid CPUs value: {0}")]
    InvalidCpus(String),

    #[error("invalid memory value: {0}")]
    InvalidMemory(String),

    #[error("image verification failed: {0}")]
    ImageVerifyFailed(String),

    #[error("image {0} is invalid or corrupted")]
    CorruptedImage(String),

    #[error("image {0} not found")]
    ImageNotFound(String),

    #[error("exit code {0}: {1}")]
    NonZeroExit(i64, String),

    #[error("probe timed out after {0}")]
    ProbeTimeout(String),

    #[error("probe error: {0}")]
    ProbeError(String),

    #[error("error parsing GPU options: {0}")]
    InvalidGpuOptions(String),

    #[error("host networking is not enabled")]
    HostNetworkDisabled,

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Docker API error: {0}")]
    Api(#[from] bollard::errors::Error),
}

// ----------------------------------------------------------------------------
// Mounter trait
// ----------------------------------------------------------------------------

/// Mounter trait for volume mounts. Must be dyn-compatible.
pub trait Mounter: Send + Sync {
    fn mount(
        &self,
        mnt: &Mount,
    ) -> Pin<Box<dyn std::future::Future<Output = std::result::Result<(), String>> + Send + '_>>;
    fn unmount(
        &self,
        mnt: &Mount,
    ) -> Pin<Box<dyn std::future::Future<Output = std::result::Result<(), String>> + Send + '_>>;
}

impl Mounter for VolumeMounter {
    fn mount(
        &self,
        mnt: &Mount,
    ) -> Pin<Box<dyn std::future::Future<Output = std::result::Result<(), String>> + Send + '_>> {
        let mnt = mnt.clone();
        Box::pin(async move {
            match self.mount(&mnt).await {
                Ok(_) => {
                    Ok(())
                }
                Err(e) => Err(e.to_string()),
            }
        })
    }

    fn unmount(
        &self,
        mnt: &Mount,
    ) -> Pin<Box<dyn std::future::Future<Output = std::result::Result<(), String>> + Send + '_>> {
        let mnt = mnt.clone();
        Box::pin(async move { self.unmount(&mnt).await.map_err(|e| e.to_string()) })
    }
}

impl Mounter for BindMounter {
    fn mount(
        &self,
        mnt: &Mount,
    ) -> Pin<Box<dyn std::future::Future<Output = std::result::Result<(), String>> + Send + '_>> {
        let result = BindMounter::mount(self, mnt);
        Box::pin(async move { result.map_err(|e| e.to_string()) })
    }

    fn unmount(
        &self,
        mnt: &Mount,
    ) -> Pin<Box<dyn std::future::Future<Output = std::result::Result<(), String>> + Send + '_>> {
        let result = BindMounter::unmount(self, mnt);
        Box::pin(async move { result.map_err(|e| e.to_string()) })
    }
}

impl Mounter for TmpfsMounter {
    fn mount(
        &self,
        mnt: &Mount,
    ) -> Pin<Box<dyn std::future::Future<Output = std::result::Result<(), String>> + Send + '_>> {
        let result = TmpfsMounter::mount(self, mnt);
        Box::pin(async move { result.map_err(|e| e.to_string()) })
    }

    fn unmount(
        &self,
        mnt: &Mount,
    ) -> Pin<Box<dyn std::future::Future<Output = std::result::Result<(), String>> + Send + '_>> {
        let result = TmpfsMounter::unmount(self, mnt);
        Box::pin(async move { result.map_err(|e| e.to_string()) })
    }
}

/// Composite mounter that dispatches to the appropriate mounter based on mount type.
pub struct CompositeMounter {
    volume_mounter: Arc<VolumeMounter>,
    bind_mounter: Arc<BindMounter>,
    tmpfs_mounter: Arc<TmpfsMounter>,
}

impl CompositeMounter {
    /// Creates a new composite mounter with all mounters initialized.
    pub fn new(client: bollard::Docker) -> Self {
        Self {
            volume_mounter: Arc::new(VolumeMounter::with_client(client)),
            bind_mounter: Arc::new(BindMounter::new(BindConfig {
                allowed: true,
                sources: Vec::new(),
            })),
            tmpfs_mounter: Arc::new(TmpfsMounter::new()),
        }
    }

    fn mounter_for(&self, mount_type: &str) -> Arc<dyn Mounter> {
        match mount_type {
            MOUNT_TYPE_BIND => self.bind_mounter.clone(),
            MOUNT_TYPE_TMPFS => self.tmpfs_mounter.clone(),
            _ => self.volume_mounter.clone(),
        }
    }
}

impl Mounter for CompositeMounter {
    fn mount(
        &self,
        mnt: &Mount,
    ) -> Pin<Box<dyn std::future::Future<Output = std::result::Result<(), String>> + Send + '_>> {
        let mnt = mnt.clone();
        let mounter = self.mounter_for(mnt.mount_type.as_deref().map_or("", |t| t));
        Box::pin(async move { mounter.mount(&mnt).await })
    }

    fn unmount(
        &self,
        mnt: &Mount,
    ) -> Pin<Box<dyn std::future::Future<Output = std::result::Result<(), String>> + Send + '_>> {
        let mnt = mnt.clone();
        let mounter = self.mounter_for(mnt.mount_type.as_deref().map_or("", |t| t));
        Box::pin(async move { mounter.unmount(&mnt).await })
    }
}

/// Docker runtime for executing tasks in containers.
#[derive(Clone)]
pub struct DockerRuntime {
    pub client: Docker,
    config: DockerConfig,
    images: Arc<RwLock<HashMap<String, std::time::Instant>>>,
    pull_tx: mpsc::Sender<PullRequest>,
    tasks: Arc<RwLock<usize>>,
    mounter: Arc<dyn Mounter>,
}

struct PullRequest {
    image: String,
    registry: Option<Registry>,
    result_tx: tokio::sync::oneshot::Sender<std::result::Result<(), DockerError>>,
}

impl crate::runtime::Runtime for DockerRuntime {
    fn run(&self, task: &twerk_core::task::Task) -> crate::runtime::BoxedFuture<()> {
        let mut task_clone = task.clone();
        let this = self.clone();
        Box::pin(async move {
            this.run(&mut task_clone).await.map_err(|e| anyhow::anyhow!(e))
        })
    }

    fn stop(&self, _task: &twerk_core::task::Task) -> crate::runtime::BoxedFuture<()> {
        Box::pin(async move {
            Ok(())
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
    pub async fn new(config: DockerConfig) -> std::result::Result<Self, DockerError> {
        let client = Docker::connect_with_local_defaults()
            .map_err(|e| DockerError::ClientCreate(e.to_string()))?;

        let (pull_tx, mut pull_rx) = mpsc::channel::<PullRequest>(100);

        let images = Arc::new(RwLock::new(HashMap::new()));
        let tasks = Arc::new(RwLock::new(0));

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

        let mounter: Arc<dyn Mounter> = Arc::new(CompositeMounter::new(client.clone()));

        Ok(Self {
            client,
            config,
            images,
            pull_tx,
            tasks,
            mounter,
        })
    }

    pub async fn default_runtime() -> std::result::Result<Self, DockerError> {
        Self::new(DockerConfig::default()).await
    }

    pub async fn run(&self, task: &mut TwerkTask) -> std::result::Result<(), DockerError> {
        {
            let mut count = self.tasks.write().await;
            *count += 1;
        }

        let network_id = if let Some(sidecars) = &task.sidecars {
            if !sidecars.is_empty() {
                let id = self.create_network().await?;
                if let Some(nets) = &mut task.networks {
                    nets.push(id.clone());
                } else {
                    task.networks = Some(vec![id.clone()]);
                }
                Some(id)
            } else { None }
        } else {
            None
        };

        let mut mounted_mounts = Vec::new();
        if let Some(task_mounts) = &task.mounts {
            for mnt in task_mounts {
                let mut mnt_clone = mnt.clone();
                mnt_clone.id = Some(new_uuid());
                if let Err(e) = self.mounter.mount(&mnt_clone).await {
                    return Err(DockerError::Mount(e));
                }
                mounted_mounts.push(mnt_clone);
            }
        }
        task.mounts = Some(mounted_mounts.clone());

        if let Some(pre_tasks) = &task.pre {
            for pre_task in pre_tasks {
                let mut pre_task_clone = pre_task.clone();
                pre_task_clone.id = Some(TaskId::new(new_uuid()));
                pre_task_clone.mounts = Some(mounted_mounts.clone());
                pre_task_clone.networks = task.networks.clone();
                pre_task_clone.limits = task.limits.clone();
                self.run_task(&mut pre_task_clone).await?;
            }
        }

        self.run_task(task).await?;

        if let Some(post_tasks) = &task.post {
            for post_task in post_tasks {
                let mut post_task_clone = post_task.clone();
                post_task_clone.id = Some(TaskId::new(new_uuid()));
                post_task_clone.mounts = Some(mounted_mounts.clone());
                post_task_clone.networks = task.networks.clone();
                post_task_clone.limits = task.limits.clone();
                self.run_task(&mut post_task_clone).await?;
            }
        }

        for mnt in mounted_mounts {
            let _ = self.mounter.unmount(&mnt).await;
        }

        if let Some(id) = network_id {
            let _ = self.client.remove_network(&id).await;
        }

        Ok(())
    }

    async fn run_task(&self, task: &mut TwerkTask) -> std::result::Result<(), DockerError> {
        let container = self.create_container(task).await?;
        let container_id = container.id.clone();

        let result = async {
            if let Some(sidecars) = &task.sidecars {
                for sidecar in sidecars {
                    let mut sidecar_task = sidecar.clone();
                    sidecar_task.id = Some(TaskId::new(new_uuid()));
                    sidecar_task.mounts = task.mounts.clone();
                    sidecar_task.networks = task.networks.clone();
                    sidecar_task.limits = task.limits.clone();

                    let sidecar_container = self.create_container(&sidecar_task).await?;
                    sidecar_container.start().await?;
                }
            }

            container.start().await?;
            task.result = Some(container.wait().await?);
            Ok(())
        }.await;

        let _ = self.client.remove_container(&container_id, Some(RemoveContainerOptions { force: true, ..Default::default() })).await;
        result
    }

    pub async fn create_container(&self, task: &TwerkTask) -> std::result::Result<Container, DockerError> {
        let image = task.image.as_ref().ok_or(DockerError::TaskIdRequired)?;
        let id = task.id.as_ref().ok_or(DockerError::TaskIdRequired)?;

        self.pull_image(image, task.registry.as_ref()).await?;
        let env: Vec<String> = task.env.as_ref().map_or_else(Vec::new, |e| {
            e.iter()
                .map(|(k, v)| format!("{k}={v}"))
                .collect()
        });

        let mut mounts: Vec<BollardMount> = Vec::new();
        if let Some(task_mounts) = &task.mounts {
            for mnt in task_mounts {
                let typ = match mnt.mount_type.as_deref().map_or("volume", |t| t) {
                    MOUNT_TYPE_VOLUME => MountTypeEnum::VOLUME,
                    MOUNT_TYPE_BIND => MountTypeEnum::BIND,
                    MOUNT_TYPE_TMPFS => MountTypeEnum::TMPFS,
                    other => return Err(DockerError::UnknownMountType(other.to_string())),
                };
                mounts.push(BollardMount { target: mnt.target.clone(), source: mnt.source.clone(), typ: Some(typ), ..Default::default() });
            }
        }

        let (nano_cpus, memory) = Self::parse_limits(task.limits.as_ref())?;

        let container_config = BollardConfig {
            image: Some(image.clone()),
            env: Some(env),
            cmd: task.cmd.clone(),
            entrypoint: task.entrypoint.clone(),
            host_config: Some(HostConfig { mounts: Some(mounts), nano_cpus, memory, ..Default::default() }),
            ..Default::default()
        };

        let create_response = self.client.create_container(None::<bollard::query_parameters::CreateContainerOptions>, container_config).await?;

        Ok(Container {
            id: create_response.id,
            client: self.client.clone(),
            task_id: id.clone(),
            probe: task.probe.clone(),
            broker: self.config.broker.clone(),
        })
    }

    fn parse_limits(limits: Option<&TaskLimits>) -> std::result::Result<(Option<i64>, Option<i64>), DockerError> {
        let limits = match limits { Some(l) => l, None => return Ok((None, None)) };
        let nano_cpus = limits.cpus.as_ref().and_then(|c| c.parse::<f64>().ok()).map(|c| (c * 1e9) as i64);
        let memory = limits.memory.as_ref().and_then(|m| m.parse::<i64>().ok());
        Ok((nano_cpus, memory))
    }

    async fn pull_image(&self, image: &str, registry: Option<&Registry>) -> std::result::Result<(), DockerError> {
        let (tx, rx) = tokio::sync::oneshot::channel();
        self.pull_tx.send(PullRequest { image: image.to_string(), registry: registry.cloned(), result_tx: tx }).await.map_err(|_| DockerError::ImagePull("closed".to_string()))?;
        rx.await.map_err(|_| DockerError::ImagePull("died".to_string()))?
    }

    async fn do_pull_request(client: &Docker, _images: &Arc<RwLock<HashMap<String, std::time::Instant>>>, _config: &DockerConfig, image: &str, _registry: Option<&Registry>) -> std::result::Result<(), DockerError> {
        let mut stream = client.create_image(Some(CreateImageOptions { from_image: Some(image.to_string()), ..Default::default() }), None, None);
        while let Some(res) = stream.next().await { res?; }
        Ok(())
    }

    async fn create_network(&self) -> std::result::Result<String, DockerError> {
        let name = new_uuid();
        let res = self.client.create_network(NetworkCreateRequest { name: name.clone(), ..Default::default() }).await?;
        Ok(res.id)
    }
}

pub struct Container {
    pub id: String,
    pub client: Docker,
    pub task_id: TaskId,
    pub probe: Option<Probe>,
    pub broker: Option<Arc<dyn crate::broker::Broker>>,
}

impl Container {
    pub async fn start(&self) -> std::result::Result<(), DockerError> {
        self.client.start_container(&self.id, None::<bollard::query_parameters::StartContainerOptions>).await?;
        Ok(())
    }

    pub async fn wait(&self) -> std::result::Result<String, DockerError> {
        let mut stream = self.client.wait_container(&self.id, None::<WaitContainerOptions>);
        while let Some(res) = stream.next().await {
            let res = res?;
            if res.status_code != 0 { return Err(DockerError::NonZeroExit(res.status_code, String::new())); }
        }
        Ok(String::new())
    }
}
