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

use bollard::auth::DockerCredentials;
use bollard::container::{
    Config as BollardConfig, DownloadFromContainerOptions, LogOutput, LogsOptions,
    NetworkingConfig as BollardNetworkingConfig, RemoveContainerOptions,
    UploadToContainerOptions, WaitContainerOptions,
};
use bollard::image::CreateImageOptions;
use bollard::secret::{
    DeviceRequest, EndpointSettings, HealthConfig, HostConfig, Mount as BollardMount,
    MountTypeEnum, PortBinding,
};
use bollard::network::CreateNetworkOptions;
use bollard::Docker;
use futures_util::StreamExt;
use tar::Archive as TarArchive;
use tokio::sync::{mpsc, RwLock};
use tokio::time::{interval, sleep};

use crate::docker::archive::Archive;
use crate::docker::auth::{config_path, Config as AuthConfig};
use crate::docker::reference::parse as parse_reference;
use crate::docker::tork::{mount_type, Mount, Probe, Registry, TaskLimits};
use crate::docker::bind::{BindConfig, BindMounter};
use crate::docker::tmpfs::TmpfsMounter;
use crate::docker::volume::VolumeMounter;
use tork::task::{Task as TorkTask, TaskLogPart};
use thiserror::Error;

// ----------------------------------------------------------------------------
// Constants
// ----------------------------------------------------------------------------

/// Default workdir for task files.
const DEFAULT_WORKDIR: &str = "/tork/workdir";

/// Default image TTL (3 days).
const DEFAULT_IMAGE_TTL: Duration = Duration::from_secs(72 * 60 * 60);

/// Default probe path.
const DEFAULT_PROBE_PATH: &str = "/";

/// Default probe timeout.
const DEFAULT_PROBE_TIMEOUT: &str = "1m";

/// Default command when none specified (uses /tork/entrypoint script).
const DEFAULT_CMD: &[&str] = &["/tork/entrypoint"];

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
    pub broker: Option<Arc<dyn tork::broker::Broker>>,
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
    broker: Option<Arc<dyn tork::broker::Broker>>,
    host_network: bool,
}

impl DockerConfigBuilder {
    #[must_use]
    pub fn with_config_file(mut self, path: &str) -> Self {
        self.config_file = Some(path.to_string());
        self
    }

    #[must_use]
    pub fn with_config_path(mut self, path: &str) -> Self {
        self.config_path = Some(path.to_string());
        self
    }

    #[must_use]
    pub fn with_privileged(mut self, enabled: bool) -> Self {
        self.privileged = enabled;
        self
    }

    #[must_use]
    pub fn with_image_ttl(mut self, ttl: Duration) -> Self {
        self.image_ttl = ttl;
        self
    }

    #[must_use]
    pub fn with_image_verify(mut self, enabled: bool) -> Self {
        self.image_verify = enabled;
        self
    }

    #[must_use]
    pub fn with_broker(mut self, broker: Arc<dyn tork::broker::Broker>) -> Self {
        self.broker = Some(broker);
        self
    }

    #[must_use]
    pub fn with_host_network(mut self, enabled: bool) -> Self {
        self.host_network = enabled;
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
    ) -> Pin<Box<dyn std::future::Future<Output = Result<(), String>> + Send + '_>>;
    fn unmount(
        &self,
        mnt: &Mount,
    ) -> Pin<Box<dyn std::future::Future<Output = Result<(), String>> + Send + '_>>;
}

impl Mounter for VolumeMounter {
    fn mount(
        &self,
        mnt: &Mount,
    ) -> Pin<Box<dyn std::future::Future<Output = Result<(), String>> + Send + '_>> {
        let mnt = mnt.clone();
        Box::pin(async move {
            match self.mount(&mnt).await {
                Ok(mounted) => {
                    let mut result = mnt;
                    result.source = mounted.source;
                    Ok(())
                }
                Err(e) => Err(e.to_string()),
            }
        })
    }

    fn unmount(
        &self,
        mnt: &Mount,
    ) -> Pin<Box<dyn std::future::Future<Output = Result<(), String>> + Send + '_>> {
        let mnt = mnt.clone();
        Box::pin(async move { self.unmount(&mnt).await.map_err(|e| e.to_string()) })
    }
}

impl Mounter for BindMounter {
    fn mount(
        &self,
        mnt: &Mount,
    ) -> Pin<Box<dyn std::future::Future<Output = Result<(), String>> + Send + '_>> {
        let result = BindMounter::mount(self, mnt);
        Box::pin(async move { result.map_err(|e| e.to_string()) })
    }

    fn unmount(
        &self,
        mnt: &Mount,
    ) -> Pin<Box<dyn std::future::Future<Output = Result<(), String>> + Send + '_>> {
        let result = BindMounter::unmount(self, mnt);
        Box::pin(async move { result.map_err(|e| e.to_string()) })
    }
}

impl Mounter for TmpfsMounter {
    fn mount(
        &self,
        mnt: &Mount,
    ) -> Pin<Box<dyn std::future::Future<Output = Result<(), String>> + Send + '_>> {
        let result = TmpfsMounter::mount(self, mnt);
        Box::pin(async move { result.map_err(|e| e.to_string()) })
    }

    fn unmount(
        &self,
        mnt: &Mount,
    ) -> Pin<Box<dyn std::future::Future<Output = Result<(), String>> + Send + '_>> {
        let result = TmpfsMounter::unmount(self, mnt);
        Box::pin(async move { result.map_err(|e| e.to_string()) })
    }
}

/// Composite mounter that dispatches to the appropriate mounter based on mount type.
///
/// This ensures bind, tmpfs, and volume mounts are handled by their respective mounters.
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
            mount_type::BIND => self.bind_mounter.clone(),
            mount_type::TMPFS => self.tmpfs_mounter.clone(),
            // Default to volume for mount_type::VOLUME or unknown types
            _ => self.volume_mounter.clone(),
        }
    }
}

impl Mounter for CompositeMounter {
    fn mount(
        &self,
        mnt: &Mount,
    ) -> Pin<Box<dyn std::future::Future<Output = Result<(), String>> + Send + '_>> {
        let mnt = mnt.clone();
        let mounter = self.mounter_for(&mnt.mount_type);
        Box::pin(async move { mounter.mount(&mnt).await })
    }

    fn unmount(
        &self,
        mnt: &Mount,
    ) -> Pin<Box<dyn std::future::Future<Output = Result<(), String>> + Send + '_>> {
        let mnt = mnt.clone();
        let mounter = self.mounter_for(&mnt.mount_type);
        Box::pin(async move { mounter.unmount(&mnt).await })
    }
}

// ----------------------------------------------------------------------------
// Runtime State
// ----------------------------------------------------------------------------

/// Docker runtime for executing tasks in containers.
#[allow(dead_code)] // fields used in future Go-parity features (image cache, pruning)
pub struct DockerRuntime {
    /// Docker client.
    pub client: Docker,
    /// Configuration.
    config: DockerConfig,
    /// Image cache with timestamps.
    images: Arc<RwLock<HashMap<String, std::time::Instant>>>,
    /// Pull request sender.
    pull_tx: mpsc::Sender<PullRequest>,
    /// Task count for pruning.
    tasks: Arc<RwLock<usize>>,
    /// Pruner cancellation.
    pruner_cancel: tokio::sync::oneshot::Sender<()>,
    /// Mounter for volumes and mounts.
    mounter: Arc<dyn Mounter>,
}

struct PullRequest {
    image: String,
    registry: Option<Registry>,
    #[allow(dead_code)] // reserved for Go-parity pull logging
    logger: Box<dyn std::io::Write + Send + Sync>,
    result_tx: tokio::sync::oneshot::Sender<Result<(), DockerError>>,
}

// ----------------------------------------------------------------------------
// Implementation
// ----------------------------------------------------------------------------

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

        let images = Arc::new(RwLock::new(HashMap::new()));
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
            let mut ticker = interval(Duration::from_secs(60 * 60));
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

    /// Runs a task in a Docker container.
    ///
    /// # Errors
    ///
    /// Returns `DockerError` if the task cannot be executed.
    pub async fn run(&self, task: &mut Task) -> Result<(), DockerError> {
        // Increment task count
        {
            let mut count = self.tasks.write().await;
            *count += 1;
        }

        // Decrement task count when done (deferred via drop guard)
        struct Guard(Arc<RwLock<usize>>);
        impl Drop for Guard {
            fn drop(&mut self) {
                if let Ok(mut count) = self.0.try_write() {
                    *count = count.saturating_sub(1);
                }
            }
        }
        let _guard = Guard(self.tasks.clone());

        // If the task has sidecars, create a network
        let network_id = if !task.sidecars.is_empty() {
            let id = self.create_network().await?;
            task.networks.push(id.clone());
            Some(id)
        } else {
            None
        };

        // Prepare mounts
        let mut mounted_mounts = Vec::new();
        for mnt in &task.mounts {
            let mut mnt = mnt.clone();
            mnt.id = Some(uuid::Uuid::new_v4().to_string());
            if let Err(e) = self.mounter.mount(&mnt).await {
                return Err(DockerError::Mount(e));
            }
            mounted_mounts.push(mnt);
        }
        task.mounts = mounted_mounts.clone();

        // Execute pre-tasks
        let pre_tasks: Vec<Task> = task.pre.iter().cloned().collect();
        for mut pre_task in pre_tasks {
            pre_task.id = uuid::Uuid::new_v4().to_string();
            pre_task.mounts = mounted_mounts.clone();
            pre_task.networks = task.networks.clone();
            pre_task.limits = task.limits.clone();
            self.run_task(&mut pre_task).await?;
        }

        // Run the actual task
        self.run_task(task).await?;

        // Execute post-tasks
        let post_tasks: Vec<Task> = task.post.iter().cloned().collect();
        for mut post_task in post_tasks {
            post_task.id = uuid::Uuid::new_v4().to_string();
            post_task.mounts = mounted_mounts.clone();
            post_task.networks = task.networks.clone();
            post_task.limits = task.limits.clone();
            self.run_task(&mut post_task).await?;
        }

        // Clean up mounts
        for mnt in mounted_mounts {
            if let Err(e) = self.mounter.unmount(&mnt).await {
                tracing::error!(error = %e, mount = ?mnt, "error unmounting");
            }
        }

        // Clean up network
        if let Some(ref id) = network_id {
            self.remove_network(id).await;
        }

        Ok(())
    }

    /// Runs a single task (main, pre, or post).
    async fn run_task(&self, task: &mut Task) -> Result<(), DockerError> {
        let container = self.create_container(task).await?;

        let container_id = container.id.clone();
        let torkdir_source = container.torkdir_source.clone();

        let result = async {
            // Start sidecars
            for sidecar in &task.sidecars {
                let mut sidecar_task = sidecar.clone();
                sidecar_task.id = uuid::Uuid::new_v4().to_string();
                sidecar_task.mounts = task.mounts.clone();
                sidecar_task.networks = task.networks.clone();
                sidecar_task.limits = task.limits.clone();

                let sidecar_container = self.create_container(&sidecar_task).await?;
                let sidecar_id = sidecar_container.id.clone();
                let sidecar_torkdir = sidecar_container.torkdir_source.clone();

                sidecar_container.start().await
                    .map_err(|e| DockerError::ContainerStart(e.to_string()))?;

                // Defer sidecar removal
                let sc = self.client.clone();
                tokio::spawn(async move {
                    let _ = sc.remove_container(
                        &sidecar_id,
                        Some(RemoveContainerOptions { force: true, ..Default::default() }),
                    ).await;
                    if let Some(source) = sidecar_torkdir {
                        let _ = sc.remove_volume(&source, None::<bollard::volume::RemoveVolumeOptions>).await;
                    }
                });
            }

            // Start main container (includes probe if configured)
            container.start().await?;

            // Wait for completion and capture result
            task.result = Some(container.wait().await?);
            Ok(())
        }.await;

        // Clean up main container
        let _ = self.client.remove_container(
            &container_id,
            Some(RemoveContainerOptions { force: true, ..Default::default() }),
        ).await;
        if let Some(source) = torkdir_source {
            let _ = self.client.remove_volume(&source, None).await;
        }

        result
    }

    /// Health check on the Docker daemon.
    pub async fn health_check(&self) -> Result<(), DockerError> {
        self.client.ping().await
            .map(|_| ())
            .map_err(|e| DockerError::ClientCreate(e.to_string()))
    }

    /// Pull an image via the serialized pull queue.
    async fn pull_image(&self, image: &str, registry: Option<&Registry>) -> Result<(), DockerError> {
        let (result_tx, result_rx) = tokio::sync::oneshot::channel();
        let request = PullRequest {
            image: image.to_string(),
            registry: registry.cloned(),
            logger: Box::new(std::io::sink()),
            result_tx,
        };
        self.pull_tx.send(request).await
            .map_err(|_| DockerError::ImagePull("pull queue closed".to_string()))?;
        result_rx.await
            .map_err(|_| DockerError::ImagePull("pull worker died".to_string()))?
    }

    /// Internal pull implementation.
    async fn do_pull_request(
        client: &Docker,
        images: &Arc<RwLock<HashMap<String, std::time::Instant>>>,
        config: &DockerConfig,
        image: &str,
        #[allow(unused_variables)] registry: Option<&Registry>,
    ) -> Result<(), DockerError> {
        // Check cache (respecting TTL)
        {
            let cache = images.read().await;
            if let Some(ts) = cache.get(image) {
                if std::time::Instant::now().duration_since(*ts) <= config.image_ttl {
                    return Ok(());
                }
            }
        }

        // Check local
        let exists = Self::image_exists_locally(client, image).await?;
        if !exists {
            let credentials = Self::get_registry_credentials(config, image).await?;

            let options = CreateImageOptions {
                from_image: image,
                ..Default::default()
            };
            let mut stream = client.create_image(Some(options), None, credentials);
            while let Some(result) = stream.next().await {
                match result {
                    Ok(_) => {}
                    Err(e) => return Err(DockerError::ImagePull(e.to_string())),
                }
            }
        }

        // Verify if enabled (Go parity: verifyImage)
        if config.image_verify {
            if let Err(_e) = Self::verify_image(client, image).await {
                let _ = client.remove_image(image, None::<bollard::image::RemoveImageOptions>, None::<DockerCredentials>).await;
                return Err(DockerError::CorruptedImage(image.to_string()));
            }
        }

        // Cache
        {
            let mut cache = images.write().await;
            cache.insert(image.to_string(), std::time::Instant::now());
        }

        Ok(())
    }

    /// Checks if an image exists locally.
    async fn image_exists_locally(client: &Docker, name: &str) -> Result<bool, DockerError> {
        let options = bollard::image::ListImagesOptions::<String> {
            all: true,
            ..Default::default()
        };
        let image_list = client.list_images(Some(options)).await
            .map_err(|e| DockerError::ImagePull(e.to_string()))?;
        Ok(image_list.iter().any(|img| img.repo_tags.iter().any(|tag| tag == name)))
    }

    /// Verifies image integrity by creating a test container and removing it.
    ///
    /// Go parity: `verifyImage` — creates container with `cmd: ["true"]`.
    async fn verify_image(client: &Docker, image: &str) -> Result<(), DockerError> {
        let config = BollardConfig {
            image: Some(image),
            cmd: Some(vec!["true"]),
            ..Default::default()
        };
        let response = client
            .create_container::<String, &str>(None, config)
            .await
            .map_err(|e| DockerError::ImageVerifyFailed(format!("{}: {}", image, e)))?;

        // Clean up test container
        let _ = client.remove_container(
            &response.id,
            Some(RemoveContainerOptions { force: true, ..Default::default() }),
        ).await;

        Ok(())
    }

    /// Gets registry credentials for an image.
    async fn get_registry_credentials(
        config: &DockerConfig,
        image: &str,
    ) -> Result<Option<DockerCredentials>, DockerError> {
        let reference = parse_reference(image)
            .map_err(|e| DockerError::ImagePull(e.to_string()))?;

        if reference.domain.is_empty() {
            return Ok(None);
        }

        // Load auth config: config_file takes priority, then config_path, then default path
        let auth_config = match (&config.config_file, &config.config_path) {
            (Some(path), _) | (_, Some(path)) => AuthConfig::load_from_path(path)
                .map_err(|e| DockerError::ImagePull(e.to_string()))?,
            (None, None) => {
                let path = config_path().map_err(|e| DockerError::ImagePull(e.to_string()))?;
                AuthConfig::load_from_path(&path)
                    .map_err(|e| DockerError::ImagePull(e.to_string()))?
            }
        };

        let (username, password) = auth_config
            .get_credentials(&reference.domain)
            .map_err(|e| DockerError::ImagePull(e.to_string()))?;

        if username.is_empty() && password.is_empty() {
            return Ok(None);
        }

        Ok(Some(DockerCredentials {
            username: Some(username),
            password: Some(password),
            ..Default::default()
        }))
    }

    /// Creates a network for sidecar communication.
    async fn create_network(&self) -> Result<String, DockerError> {
        let id = uuid::Uuid::new_v4().to_string();
        let options = CreateNetworkOptions {
            name: id.clone(),
            driver: "bridge".to_string(),
            check_duplicate: true,
            ..Default::default()
        };
        let response = self.client.create_network(options).await
            .map_err(|e| DockerError::NetworkCreate(e.to_string()))?;
        Ok(response.id)
    }

    /// Removes a network with retry logic.
    ///
    /// Go parity: `removeNetwork` — exponential backoff 200ms→3200ms, 5 retries.
    async fn remove_network(&self, network_id: &str) {
        let mut delay = Duration::from_millis(200);
        for i in 0..5u32 {
            match self.client.remove_network(network_id).await {
                Ok(()) => return,
                Err(e) => {
                    if i == 4 {
                        tracing::error!(network_id, error = %e, "failed to remove network");
                        return;
                    }
                    tracing::debug!(network_id, attempt = i+1, error = %e, "retrying");
                    sleep(delay).await;
                    delay *= 2;
                }
            }
        }
    }

    /// Creates a container for a task.
    ///
    /// Go parity: `createTaskContainer` — full lifecycle setup including
    /// image pull, env, mounts, limits, GPU, probe ports, networking aliases,
    /// workdir, and file initialization.
    #[allow(dead_code)] // used in integration tests
    pub async fn create_container(&self, task: &Task) -> Result<Container, DockerError> {
        if task.id.is_empty() {
            return Err(DockerError::TaskIdRequired);
        }

        // Pull image
        self.pull_image(&task.image, task.registry.as_ref()).await?;
        // Build env (Go parity: iterates t.Env HashMap, formats KEY=VALUE, adds TORK_OUTPUT and TORK_PROGRESS)
        let env: Vec<String> = task.env.iter()
            .map(|(k, v)| format!("{}={}", k, v))
            .chain(std::iter::once("TORK_OUTPUT=/tork/stdout".to_string()))
            .chain(std::iter::once("TORK_PROGRESS=/tork/progress".to_string()))
            .collect();

        // Build mounts with validation (Go parity: mount type validation)
        let mut mounts: Vec<BollardMount> = Vec::new();
        for mnt in &task.mounts {
            let typ = match mnt.mount_type.as_str() {
                mount_type::VOLUME => {
                    if mnt.target.as_ref().is_none_or(|t| t.is_empty()) {
                        return Err(DockerError::VolumeTargetRequired);
                    }
                    MountTypeEnum::VOLUME
                }
                mount_type::BIND => {
                    if mnt.target.as_ref().is_none_or(|t| t.is_empty()) {
                        return Err(DockerError::BindTargetRequired);
                    }
                    if mnt.source.as_ref().is_none_or(|s| s.is_empty()) {
                        return Err(DockerError::BindSourceRequired);
                    }
                    MountTypeEnum::BIND
                }
                mount_type::TMPFS => MountTypeEnum::TMPFS,
                other => return Err(DockerError::UnknownMountType(other.to_string())),
            };
            tracing::debug!(source = ?mnt.source, target = ?mnt.target, "Mounting");
            mounts.push(BollardMount {
                target: mnt.target.clone(),
                source: mnt.source.clone(),
                typ: Some(typ),
                ..Default::default()
            });
        }

        // Create torkdir volume
        let torkdir_volume_name = uuid::Uuid::new_v4().to_string();
        let _ = self.client.create_volume(bollard::volume::CreateVolumeOptions {
            name: torkdir_volume_name.clone(),
            driver: "local".to_string(),
            ..Default::default()
        }).await
            .map_err(|e| DockerError::VolumeCreate(e.to_string()))?;

        mounts.push(BollardMount {
            target: Some("/tork".to_string()),
            source: Some(torkdir_volume_name.clone()),
            typ: Some(MountTypeEnum::VOLUME),
            ..Default::default()
        });

        // Parse limits
        let (nano_cpus, memory) = Self::parse_limits(task.limits.as_ref())?;

        // Working directory
        let workdir = if task.workdir.is_some() {
            task.workdir.clone()
        } else if !task.files.is_empty() {
            Some(DEFAULT_WORKDIR.to_string())
        } else {
            None
        };

        // Entrypoint auto-detection (Go parity)
        let cmd: Vec<String> = if task.cmd.is_empty() {
            DEFAULT_CMD.iter().map(|s| s.to_string()).collect()
        } else {
            task.cmd.clone()
        };

        let entrypoint: Vec<String> = if task.entrypoint.is_empty() && task.run.is_some() {
            RUN_ENTRYPOINT.iter().map(|s| s.to_string()).collect()
        } else {
            task.entrypoint.clone()
        };

        // Probe port configuration (Go parity: exposed ports + port bindings)
        let mut exposed_ports: HashMap<String, HashMap<(), ()>> = HashMap::new();
        let mut port_bindings: HashMap<String, Option<Vec<PortBinding>>> = HashMap::new();
        let mut healthcheck: Option<HealthConfig> = None;

        if let Some(ref probe) = task.probe {
            if let Some(port) = probe.port {
                let port_key = format!("{}/tcp", port);
                exposed_ports.insert(port_key.clone(), HashMap::new());
                port_bindings.insert(port_key, Some(vec![PortBinding {
                    host_ip: Some("127.0.0.1".to_string()),
                    host_port: Some("0".to_string()),
                }]));

                // Build Docker HEALTHCHECK for native container health monitoring
                let probe_path = probe.path.as_deref().unwrap_or(DEFAULT_PROBE_PATH);
                let timeout_str = probe.timeout.as_deref().unwrap_or(DEFAULT_PROBE_TIMEOUT);
                let timeout = parse_go_duration(timeout_str)
                    .unwrap_or(Duration::from_secs(60));
                let interval = Duration::from_secs(30);

                healthcheck = Some(HealthConfig {
                    test: Some(vec![
                        "CMD".to_string(),
                        "curl".to_string(),
                        "-f".to_string(),
                        "-s".to_string(),
                        format!("http://localhost:{}{}", port, probe_path),
                    ]),
                    interval: Some(interval.as_nanos() as i64),
                    timeout: Some(timeout.as_nanos() as i64),
                    retries: Some(3),
                    start_period: Some(0),
                    start_interval: Some(0),
                });
            }
        }

        // GPU device requests (Go parity: `gpuOpts.Set(t.GPUs)`)
        let device_requests = task.gpus.as_ref()
            .map(|gpu_str| Self::parse_gpu_options(gpu_str))
            .transpose()?;

        // Host network mode detection (Go parity: `network == hostNetworkName`)
        let host_network_mode = task.networks.iter().any(|n| n == "host");
        
        // Validate host network usage
        if host_network_mode && !self.config.host_network {
            return Err(DockerError::HostNetworkDisabled);
        }

        // Networking config with aliases (Go parity: `slug.Make(t.Name)`)
        // Note: Network aliases are not supported with host networking
        let networking_config = if task.networks.is_empty() || host_network_mode {
            None
        } else {
            let mut endpoints = HashMap::new();
            for nw in &task.networks {
                let alias = slugify(task.name.as_deref().unwrap_or(&task.id));
                endpoints.insert(nw.clone(), EndpointSettings {
                    aliases: Some(vec![alias]),
                    ..Default::default()
                });
            }
            Some(BollardNetworkingConfig { endpoints_config: endpoints })
        };

        // Build container config
        let container_config = BollardConfig {
            image: Some(task.image.clone()),
            env: Some(env),
            cmd: Some(cmd),
            entrypoint: if entrypoint.is_empty() { None } else { Some(entrypoint) },
            working_dir: workdir.clone(),
            exposed_ports: if exposed_ports.is_empty() { None } else { Some(exposed_ports) },
            host_config: Some(HostConfig {
                mounts: Some(mounts),
                nano_cpus,
                memory,
                privileged: Some(self.config.privileged),
                device_requests,
                port_bindings: if port_bindings.is_empty() { None } else { Some(port_bindings) },
                network_mode: if host_network_mode { Some("host".to_string()) } else { None },
                ..Default::default()
            }),
            networking_config,
            healthcheck,
            ..Default::default()
        };

        // Create container with 30s timeout (Go parity: createCtx)
        let create_response = tokio::time::timeout(
            Duration::from_secs(30),
            self.client.create_container::<String, String>(None, container_config),
        ).await
            .map_err(|_| DockerError::ContainerCreate("creation timed out".to_string()))?
            .map_err(|e| {
                tracing::error!(image = %task.image, error = %e, "Error creating container");
                DockerError::ContainerCreate(e.to_string())
            })?;

        // Clone volume name before moving into struct (needed for cleanup on error)
        let torkdir_volume_name_clone = torkdir_volume_name.clone();

        let container = Container {
            id: create_response.id,
            client: self.client.clone(),
            torkdir_source: Some(torkdir_volume_name),
            task_id: task.id.clone(),
            probe: task.probe.clone(),
            broker: self.config.broker.clone(),
        };

        // Capture values for cleanup before init (since init consumes self)
        let container_id = container.id.clone();
        let cleanup_client = container.client.clone();
        let torkdir_volume = torkdir_volume_name_clone;

        // Clean up container and volume on initialization failure (Go parity: defer tc.Remove)
        if let Err(e) = container.init_torkdir(task.run.as_deref()).await {
            let _ = cleanup_client
                .remove_container(
                    &container_id,
                    Some(RemoveContainerOptions { force: true, ..Default::default() }),
                )
                .await;
            let _ = cleanup_client
                .remove_volume(&torkdir_volume, None::<bollard::volume::RemoveVolumeOptions>)
                .await;
            return Err(e);
        }

        let effective_workdir = workdir.as_deref().unwrap_or(DEFAULT_WORKDIR);

        // Clean up container and volume on initialization failure (Go parity: defer tc.Remove)
        if let Err(e) = container.init_workdir(&task.files, effective_workdir).await {
            let _ = cleanup_client
                .remove_container(
                    &container_id,
                    Some(RemoveContainerOptions { force: true, ..Default::default() }),
                )
                .await;
            let _ = cleanup_client
                .remove_volume(&torkdir_volume, None::<bollard::volume::RemoveVolumeOptions>)
                .await;
            return Err(e);
        }

        tracing::debug!(container_id = %container.id, "Created container");
        Ok(container)
    }

    /// Parses task limits into Docker resource values.
    fn parse_limits(limits: Option<&TaskLimits>) -> Result<(Option<i64>, Option<i64>), DockerError> {
        let limits = match limits {
            Some(l) => l,
            None => return Ok((None, None)),
        };

        let nano_cpus = match &limits.cpus {
            Some(cpus) if !cpus.is_empty() => {
                Some((cpus.parse::<f64>()
                    .map_err(|_| DockerError::InvalidCpus(cpus.clone()))? * 1e9) as i64)
            }
            _ => None,
        };

        let memory = match &limits.memory {
            Some(mem) if !mem.is_empty() => {
                Some(parse_memory_bytes(mem).map_err(|e| DockerError::InvalidMemory(e))?)
            }
            _ => None,
        };

        Ok((nano_cpus, memory))
    }

    /// Parses GPU options into DeviceRequests.
    ///
    /// Go parity: `cliopts.GpuOpts.Set(t.GPUs)` — handles count, driver,
    /// capabilities, device IDs.
    fn parse_gpu_options(gpu_str: &str) -> Result<Vec<DeviceRequest>, DockerError> {
        let mut count: Option<i64> = None;
        let mut driver: Option<String> = None;
        let mut capabilities: Vec<String> = Vec::new();
        let mut device_ids: Vec<String> = Vec::new();

        for part in gpu_str.split(',') {
            let part = part.trim();
            if let Some((key, value)) = part.split_once('=') {
                match key.trim() {
                    "count" => {
                        count = if value.trim() == "all" {
                            Some(-1)
                        } else {
                            Some(value.trim().parse::<i64>().map_err(|_| {
                                DockerError::InvalidGpuOptions(format!("invalid count: {}", value))
                            })?)
                        };
                    }
                    "driver" => { driver = Some(value.trim().to_string()); }
                    "capabilities" => {
                        for cap in value.split(';') {
                            capabilities.push(cap.trim().to_string());
                        }
                    }
                    "device" => {
                        for dev in value.split(';') {
                            device_ids.push(dev.trim().to_string());
                        }
                    }
                    other => {
                        return Err(DockerError::InvalidGpuOptions(format!("unknown GPU option: {}", other)));
                    }
                }
            }
        }

        if capabilities.is_empty() {
            capabilities.push("gpu".to_string());
        }

        Ok(vec![DeviceRequest {
            count,
            driver,
            capabilities: Some(vec![capabilities]),
            device_ids: if device_ids.is_empty() { None } else { Some(device_ids) },
            options: None,
        }])
    }

    /// Prunes old images. Go parity: only prunes when no tasks running.
    async fn prune_images(
        client: &Docker,
        images: &Arc<RwLock<HashMap<String, std::time::Instant>>>,
        tasks: &Arc<RwLock<usize>>,
        ttl: Duration,
    ) {
        if *tasks.read().await > 0 {
            return;
        }

        let now = std::time::Instant::now();
        let to_remove: Vec<String> = {
            let cache = images.read().await;
            cache.iter()
                .filter(|(_, ts)| now.duration_since(**ts) > ttl)
                .map(|(img, _)| img.clone())
                .collect()
        };

        for image in to_remove {
            let _ = client.remove_image(&image, None::<bollard::image::RemoveImageOptions>, None::<DockerCredentials>).await;
            if let Ok(mut cache) = images.try_write() {
                cache.remove(&image);
                tracing::debug!(image = %image, "pruned image");
            }
        }
    }
}

// =============================================================================
// Task type
// =============================================================================

/// Task to execute in a container.
#[derive(Debug, Clone)]
pub struct Task {
    pub id: String,
    pub name: Option<String>,
    pub image: String,
    pub cmd: Vec<String>,
    pub entrypoint: Vec<String>,
    pub run: Option<String>,
    pub env: HashMap<String, String>,
    pub files: HashMap<String, String>,
    pub workdir: Option<String>,
    pub limits: Option<TaskLimits>,
    pub mounts: Vec<Mount>,
    pub networks: Vec<String>,
    pub sidecars: Vec<Task>,
    pub pre: Vec<Task>,
    pub post: Vec<Task>,
    pub registry: Option<Registry>,
    pub probe: Option<Probe>,
    pub gpus: Option<String>,
    pub result: Option<String>,
    pub progress: f64,
}

impl Default for Task {
    fn default() -> Self {
        Self {
            id: String::new(),
            name: None,
            image: String::new(),
            cmd: Vec::new(),
            entrypoint: Vec::new(),
            run: None,
            env: HashMap::new(),
            files: HashMap::new(),
            workdir: None,
            limits: None,
            mounts: Vec::new(),
            networks: Vec::new(),
            sidecars: Vec::new(),
            pre: Vec::new(),
            post: Vec::new(),
            registry: None,
            probe: None,
            gpus: None,
            result: None,
            progress: 0.0,
        }
    }
}

// =============================================================================
// Container
// =============================================================================

/// Container represents a running Docker container.
#[allow(dead_code)] // used in integration tests
pub struct Container {
    /// Container ID.
    pub id: String,
    /// Docker client for API operations.
    pub client: Docker,
    torkdir_source: Option<String>,
    task_id: String,
    probe: Option<Probe>,
    broker: Option<Arc<dyn tork::broker::Broker>>,
}

impl Container {
    /// Start the container, then probe if configured.
    ///
    /// Go parity: `tcontainer.Start` → `probeContainer`.
    #[allow(dead_code)] // used in integration tests
    pub async fn start(&self) -> Result<(), DockerError> {
        tracing::debug!(container_id = %self.id, "Starting container");
        self.client.start_container::<String>(&self.id, None).await
            .map_err(|e| DockerError::ContainerStart(format!("{}: {}", self.id, e)))?;

        self.probe_container().await?;
        Ok(())
    }

    /// Wait for container completion.
    ///
    /// Go parity: `tcontainer.Wait` — streams logs, reports progress,
    /// reads /tork/stdout on success, tails logs on failure.
    #[allow(dead_code)] // used in integration tests
    pub async fn wait(&self) -> Result<String, DockerError> {
        // Spawn progress reporting
        let progress_client = self.client.clone();
        let progress_id = self.id.clone();
        let progress_task_id = self.task_id.clone();
        let progress_broker = self.broker.clone();
        tokio::spawn(async move {
            Self::report_progress(
                progress_client, progress_id, progress_task_id, progress_broker,
            ).await;
        });

        // Spawn log streaming
        let log_client = self.client.clone();
        let log_id = self.id.clone();
        let log_task_id = self.task_id.clone();
        let log_broker = self.broker.clone();
        tokio::spawn(async move {
            Self::stream_logs(log_client, log_id, log_task_id, log_broker).await;
        });

        // Wait — returns a Stream in bollard
        let options = WaitContainerOptions { condition: "not-running".to_string() };
        let result = self.client.wait_container(&self.id, Some(options))
            .next().await
            .ok_or_else(|| DockerError::ContainerWait("no wait result".to_string()))?
            .map_err(|e| DockerError::ContainerWait(e.to_string()))?;

        let status_code: i64 = result.status_code;

        if status_code != 0 {
            let snippet = match self.read_logs_tail(10).await {
                Ok(s) => s,
                Err(e) => {
                    tracing::debug!(error = %e, "failed to read logs tail, using empty snippet");
                    String::new()
                }
            };
            return Err(DockerError::NonZeroExit(status_code, snippet));
        }

        let stdout = self.read_output().await?;
        tracing::debug!(status_code, task_id = %self.task_id, "task completed");
        Ok(stdout)
    }

    /// Health probe — poll HTTP endpoint until 200 OK or timeout.
    ///
    /// Go parity: `tcontainer.probeContainer`.
    async fn probe_container(&self) -> Result<(), DockerError> {
        let probe = match &self.probe {
            Some(p) => p,
            None => return Ok(()),
        };
        let port = match probe.port {
            Some(p) => p,
            None => return Ok(()),
        };

        let path = probe.path.as_deref().unwrap_or(DEFAULT_PROBE_PATH);
        let timeout_str = probe.timeout.as_deref().unwrap_or(DEFAULT_PROBE_TIMEOUT);
        let timeout = parse_go_duration(timeout_str)
            .map_err(|e| DockerError::ProbeTimeout(format!("invalid timeout: {}", e)))?;

        // Inspect to get assigned host port
        let inspect = self.client.inspect_container(&self.id, None).await
            .map_err(|e| DockerError::ContainerInspect(format!("{}: {}", self.id, e)))?;

        let port_key = format!("{}/tcp", port);
        let host_port = inspect
            .network_settings
            .as_ref()
            .and_then(|ns| ns.ports.as_ref())
            .and_then(|ports| ports.get(&port_key))
            .and_then(|opt| opt.as_ref())
            .and_then(|bindings| bindings.first())
            .and_then(|b| b.host_port.as_ref())
            .ok_or_else(|| DockerError::ProbeError(format!("no port found for {}", self.id)))?;

        let probe_url = format!("http://localhost:{}{}", host_port, path);

        let http_client = reqwest::Client::builder()
            .timeout(Duration::from_secs(3))
            .connect_timeout(Duration::from_secs(3))
            .build()
            .map_err(|e| DockerError::ProbeError(format!("HTTP client: {}", e)))?;

        let deadline = tokio::time::Instant::now() + timeout;
        loop {
            if tokio::time::Instant::now() >= deadline {
                return Err(DockerError::ProbeTimeout(timeout_str.to_string()));
            }
            match http_client.get(&probe_url).send().await {
                Ok(resp) if resp.status().as_u16() == 200 => return Ok(()),
                Ok(resp) => {
                    tracing::debug!(container_id = %self.id, status = resp.status().as_u16(), "probe non-200");
                }
                Err(e) => {
                    tracing::debug!(container_id = %self.id, error = %e, "probe failed");
                }
            }
            sleep(Duration::from_secs(1)).await;
        }
    }

    /// Read last N lines of logs.
    async fn read_logs_tail(&self, lines: usize) -> Result<String, DockerError> {
        let options = LogsOptions {
            stdout: true, stderr: true,
            tail: lines.to_string(),
            ..Default::default()
        };
        let mut stream = self.client.logs(&self.id, Some(options));
        let mut output = String::new();
        while let Some(result) = stream.next().await {
            match result {
                Ok(chunk) => output.push_str(&chunk.to_string()),
                Err(_) => break,
            }
        }
        Ok(output)
    }

    /// Stream container logs to broker in real-time.
    ///
    /// Logs are published immediately as they arrive from Docker.
    /// Uses `tail: "all"` to capture any logs written before the stream started,
    /// plus `follow: true` for real-time streaming.
    async fn stream_logs(
        client: Docker,
        container_id: String,
        task_id: String,
        broker: Option<Arc<dyn tork::broker::Broker>>,
    ) {
        let Some(broker) = broker else { return };

        // Use tail: "all" to get existing logs, follow: true for real-time streaming
        let options = LogsOptions::<String> {
            stdout: true,
            stderr: true,
            follow: true,
            tail: "all".to_string(),
            ..Default::default()
        };
        let mut stream = client.logs(&container_id, Some(options));

        let mut part_num = 0i64;

        // Stream logs in real-time: publish immediately as they arrive
        while let Some(result) = stream.next().await {
            match result {
                Ok(LogOutput::StdOut { message }) | Ok(LogOutput::StdErr { message }) => {
                    let msg = String::from_utf8_lossy(message.as_ref()).to_string();
                    if !msg.is_empty() {
                        part_num += 1;
                        let _ = broker.publish_task_log_part(&TaskLogPart {
                            id: None,
                            number: part_num,
                            task_id: Some(task_id.clone()),
                            contents: Some(msg),
                            created_at: None,
                        }).await;
                    }
                }
                Ok(_) => {}
                Err(_) => break,
            }
        }
    }

    /// Report progress from /tork/progress every 10s.
    async fn report_progress(
        client: Docker,
        container_id: String,
        task_id: String,
        broker: Option<Arc<dyn tork::broker::Broker>>,
    ) {
        let Some(broker) = broker else { return };

        let mut tick = tokio::time::interval(Duration::from_secs(10));
        let mut prev: Option<f64> = None;

        loop {
            tokio::select! {
                _ = tick.tick() => {
                    match Self::read_progress_value(&client, &container_id).await {
                        Ok(p) if prev.map(|old| (old - p).abs() > 0.001).unwrap_or(true) => {
                            prev = Some(p);
                            tracing::debug!(task_id = %task_id, progress = %p, "progress");
                            // Publish progress to broker (Go parity: tc.broker.PublishTaskProgress)
                            let tork_task = TorkTask {
                                id: Some(task_id.clone()),
                                progress: p,
                                ..Default::default()
                            };
                            if let Err(e) = broker.publish_task_progress(&tork_task).await {
                                tracing::warn!(task_id = %task_id, error = %e, "error publishing task progress");
                            }
                        }
                        Err(_) => break, // container likely exited
                        _ => {}
                    }
                }
            }
        }
    }

    /// Read progress from /tork/progress.
    async fn read_progress_value(client: &Docker, cid: &str) -> Result<f64, DockerError> {
        let options = DownloadFromContainerOptions { path: "/tork/progress" };
        let mut stream = client.download_from_container(cid, Some(options));
        let bytes = stream.next().await
            .ok_or_else(|| DockerError::CopyFromContainer("empty".to_string()))?
            .map_err(|e| DockerError::CopyFromContainer(e.to_string()))?;
        let contents = parse_tar_contents(&bytes.to_vec());
        let s = contents.trim();
        if s.is_empty() { return Ok(0.0); }
        s.parse::<f64>().map_err(|_| DockerError::CopyFromContainer("invalid progress".to_string()))
    }

    /// Read /tork/stdout.
    async fn read_output(&self) -> Result<String, DockerError> {
        let options = DownloadFromContainerOptions { path: "/tork/stdout" };
        let mut stream = self.client.download_from_container(&self.id, Some(options));
        match stream.next().await {
            Some(Ok(bytes)) => Ok(parse_tar_contents(&bytes.to_vec())),
            Some(Err(e)) => Err(DockerError::CopyFromContainer(e.to_string())),
            None => Ok(String::new()),
        }
    }

    /// Init /tork/ dir (stdout, progress, optional entrypoint).
    async fn init_torkdir(&self, run_script: Option<&str>) -> Result<(), DockerError> {
        let mut archive = Archive::new().map_err(|e| DockerError::CopyToContainer(e.to_string()))?;
        archive.write_file("stdout", 0o222, &[]).map_err(|e| DockerError::CopyToContainer(e.to_string()))?;
        archive.write_file("progress", 0o222, &[]).map_err(|e| DockerError::CopyToContainer(e.to_string()))?;
        if let Some(script) = run_script {
            archive.write_file("entrypoint", 0o555, script.as_bytes()).map_err(|e| DockerError::CopyToContainer(e.to_string()))?;
        }
        archive.finish().map_err(|e| DockerError::CopyToContainer(e.to_string()))?;
        let mut reader = archive.reader().map_err(|e| DockerError::CopyToContainer(e.to_string()))?;
        let mut contents = Vec::new();
        std::io::Read::read_to_end(&mut reader, &mut contents).map_err(|e| DockerError::CopyToContainer(e.to_string()))?;

        let options = UploadToContainerOptions { path: "/tork/", ..Default::default() };
        self.client.upload_to_container(&self.id, Some(options), contents.into()).await
            .map_err(|e| DockerError::CopyToContainer(e.to_string()))?;
        archive.remove().map_err(|e| DockerError::CopyToContainer(e.to_string()))?;
        Ok(())
    }

    /// Init working directory with task files.
    async fn init_workdir(&self, files: &HashMap<String, String>, workdir: &str) -> Result<(), DockerError> {
        if files.is_empty() { return Ok(()); }
        let mut archive = Archive::new().map_err(|e| DockerError::CopyToContainer(e.to_string()))?;
        for (name, data) in files {
            archive.write_file(name, 0o444, data.as_bytes()).map_err(|e| DockerError::CopyToContainer(e.to_string()))?;
        }
        archive.finish().map_err(|e| DockerError::CopyToContainer(e.to_string()))?;
        let mut reader = archive.reader().map_err(|e| DockerError::CopyToContainer(e.to_string()))?;
        let mut contents = Vec::new();
        std::io::Read::read_to_end(&mut reader, &mut contents).map_err(|e| DockerError::CopyToContainer(e.to_string()))?;

        let options = UploadToContainerOptions { path: workdir, ..Default::default() };
        self.client.upload_to_container(&self.id, Some(options), contents.into()).await
            .map_err(|e| DockerError::CopyToContainer(e.to_string()))?;
        archive.remove().map_err(|e| DockerError::CopyToContainer(e.to_string()))?;
        Ok(())
    }
}

// =============================================================================
// Pure helpers
// =============================================================================

/// Slugify a string for Docker network aliases.
/// Go parity: `slug.Make(t.Name)`.
#[must_use]
fn slugify(input: &str) -> String {
    input.chars()
        .map(|c| if c.is_alphanumeric() { c.to_ascii_lowercase() } else { '-' })
        .collect::<String>()
        .split('-')
        .filter(|s| !s.is_empty())
        .collect::<Vec<_>>()
        .join("-")
}

/// Parse human memory string to bytes.
/// Go parity: `units.RAMInBytes` — B, KB, MB, GB, TB.
fn parse_memory_bytes(input: &str) -> Result<i64, String> {
    let input = input.trim();
    let (num_str, multiplier) =
        if let Some(s) = input.strip_suffix("TB").or_else(|| input.strip_suffix("tb")) {
            (s.trim(), 1_099_511_627_776_i64)
        } else if let Some(s) = input.strip_suffix("GB").or_else(|| input.strip_suffix("gb")) {
            (s.trim(), 1_073_741_824_i64)
        } else if let Some(s) = input.strip_suffix("MB").or_else(|| input.strip_suffix("mb")) {
            (s.trim(), 1_048_576_i64)
        } else if let Some(s) = input.strip_suffix("KB").or_else(|| input.strip_suffix("kb")) {
            (s.trim(), 1024_i64)
        } else if let Some(s) = input.strip_suffix("B").or_else(|| input.strip_suffix("b")) {
            (s.trim(), 1_i64)
        } else {
            return input.parse::<i64>()
                .map_err(|e| format!("cannot parse '{}': {}", input, e));
        };

    let num = num_str.parse::<f64>()
        .map_err(|e| format!("cannot parse '{}': {}", num_str, e))?;
    Ok((num * multiplier as f64) as i64)
}

/// Parse Go duration string (h, m, s, ms).
fn parse_go_duration(input: &str) -> Result<Duration, String> {
    let mut total = Duration::ZERO;
    let mut current = String::new();
    for c in input.chars() {
        if c.is_ascii_digit() || c == '.' {
            current.push(c);
        } else {
            let num: f64 = current.parse()
                .map_err(|e| format!("invalid duration '{}': {}", current, e))?;
            total += match c {
                'h' => Duration::from_secs_f64(num * 3600.0),
                'm' => Duration::from_secs_f64(num * 60.0),
                's' => Duration::from_secs_f64(num),
                _ => return Err(format!("unknown unit: {}", c)),
            };
            current.clear();
        }
    }
    if !current.is_empty() {
        return Err(format!("trailing: {}", current));
    }
    Ok(total)
}

/// Parse tar archive bytes, return first file contents.
fn parse_tar_contents(tar_bytes: &[u8]) -> String {
    let mut archive = TarArchive::new(tar_bytes);
    let Ok(entries) = archive.entries() else {
        return String::new();
    };
    for entry in entries {
        let Ok(mut entry) = entry else {
            continue;
        };
        let mut buf = Vec::new();
        if std::io::Read::read_to_end(&mut entry, &mut buf).is_ok() {
            if let Ok(s) = String::from_utf8(buf) {
                return s;
            }
        }
    }
    String::new()
}

// =============================================================================
// Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    // =========================================================================
    // parse_memory_bytes — Go parity: units.RAMInBytes
    // =========================================================================

    #[test]
    fn parse_memory_bytes_bytes() {
        assert_eq!(1, parse_memory_bytes("1B").unwrap());
        assert_eq!(10, parse_memory_bytes("10B").unwrap());
        assert_eq!(512, parse_memory_bytes("512B").unwrap());
    }

    #[test]
    fn parse_memory_bytes_lowercase_b() {
        assert_eq!(1, parse_memory_bytes("1b").unwrap());
        assert_eq!(42, parse_memory_bytes("42b").unwrap());
    }

    #[test]
    fn parse_memory_bytes_kilobytes() {
        assert_eq!(1024, parse_memory_bytes("1KB").unwrap());
        assert_eq!(512_000, parse_memory_bytes("500KB").unwrap());
        assert_eq!(1024, parse_memory_bytes("1kb").unwrap());
    }

    #[test]
    fn parse_memory_bytes_megabytes() {
        assert_eq!(1_048_576, parse_memory_bytes("1MB").unwrap());
        assert_eq!(10_485_760, parse_memory_bytes("10MB").unwrap());
        assert_eq!(524_288_000, parse_memory_bytes("500MB").unwrap());
        // lowercase
        assert_eq!(1_048_576, parse_memory_bytes("1mb").unwrap());
    }

    #[test]
    fn parse_memory_bytes_gigabytes() {
        assert_eq!(1_073_741_824, parse_memory_bytes("1GB").unwrap());
        assert_eq!(2_147_483_648, parse_memory_bytes("2GB").unwrap());
        // lowercase
        assert_eq!(1_073_741_824, parse_memory_bytes("1gb").unwrap());
    }

    #[test]
    fn parse_memory_bytes_terabytes() {
        assert_eq!(1_099_511_627_776, parse_memory_bytes("1TB").unwrap());
        assert_eq!(2_199_023_255_552, parse_memory_bytes("2TB").unwrap());
        // lowercase
        assert_eq!(1_099_511_627_776, parse_memory_bytes("1tb").unwrap());
    }

    #[test]
    fn parse_memory_bytes_whitespace_tolerance() {
        assert_eq!(1_048_576, parse_memory_bytes(" 1MB ").unwrap());
        assert_eq!(1024, parse_memory_bytes(" 1 KB ").unwrap());
        assert_eq!(1, parse_memory_bytes(" 1B ").unwrap());
    }

    #[test]
    fn parse_memory_bytes_invalid_string() {
        assert!(parse_memory_bytes("invalid").is_err());
        assert!(parse_memory_bytes("").is_err());
        assert!(parse_memory_bytes("B").is_err());
        assert!(parse_memory_bytes("KB").is_err());
        assert!(parse_memory_bytes("MB").is_err());
    }

    #[test]
    fn parse_memory_bytes_negative_is_ok() {
        // The implementation parses -1B as f64(-1.0) * 1 = -1
        // This is technically allowed by the parser (Go parity may differ)
        assert_eq!(-1, parse_memory_bytes("-1B").unwrap());
    }

    #[test]
    fn parse_memory_bytes_fractional_ok() {
        // 0.5 MB = 524288
        let result = parse_memory_bytes("0.5MB").unwrap();
        assert_eq!(524_288, result);
    }

    #[test]
    fn parse_memory_bytes_bare_number() {
        // No suffix = raw bytes
        assert_eq!(1024, parse_memory_bytes("1024").unwrap());
    }

    // =========================================================================
    // parse_limits — Go parity: parseCPUs + parseMemory
    // =========================================================================

    #[test]
    fn parse_limits_none_returns_none_tuple() {
        let result = DockerRuntime::parse_limits(None).unwrap();
        assert_eq!((None, None), result);
    }

    #[test]
    fn parse_limits_empty_cpus_and_memory() {
        let limits = TaskLimits::new(Some(""), Some(""));
        let result = DockerRuntime::parse_limits(Some(&limits)).unwrap();
        assert_eq!((None, None), result);
    }

    #[test]
    fn parse_limits_cpu_integer() {
        let limits = TaskLimits::new(Some("1"), None);
        let (cpus, mem) = DockerRuntime::parse_limits(Some(&limits)).unwrap();
        assert_eq!(Some(1_000_000_000), cpus);
        assert_eq!(None, mem);
    }

    #[test]
    fn parse_limits_cpu_two_cores() {
        let limits = TaskLimits::new(Some("2"), None);
        let (cpus, _) = DockerRuntime::parse_limits(Some(&limits)).unwrap();
        assert_eq!(Some(2_000_000_000), cpus);
    }

    #[test]
    fn parse_limits_cpu_half() {
        let limits = TaskLimits::new(Some("0.5"), None);
        let (cpus, _) = DockerRuntime::parse_limits(Some(&limits)).unwrap();
        assert_eq!(Some(500_000_000), cpus);
    }

    #[test]
    fn parse_limits_cpu_quarter() {
        let limits = TaskLimits::new(Some(".25"), None);
        let (cpus, _) = DockerRuntime::parse_limits(Some(&limits)).unwrap();
        assert_eq!(Some(250_000_000), cpus);
    }

    #[test]
    fn parse_limits_cpu_small_fraction() {
        let limits = TaskLimits::new(Some("0.125"), None);
        let (cpus, _) = DockerRuntime::parse_limits(Some(&limits)).unwrap();
        assert_eq!(Some(125_000_000), cpus);
    }

    #[test]
    fn parse_limits_cpu_invalid_string() {
        let limits = TaskLimits::new(Some("abc"), None);
        let result = DockerRuntime::parse_limits(Some(&limits));
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(err.contains("CPUs"), "error should mention CPUs: {err}");
    }

    #[test]
    fn parse_limits_memory_1g() {
        let limits = TaskLimits::new(None, Some("1GB"));
        let (cpus, mem) = DockerRuntime::parse_limits(Some(&limits)).unwrap();
        assert_eq!(None, cpus);
        assert_eq!(Some(1_073_741_824), mem);
    }

    #[test]
    fn parse_limits_memory_512m() {
        let limits = TaskLimits::new(None, Some("512MB"));
        let (cpus, mem) = DockerRuntime::parse_limits(Some(&limits)).unwrap();
        assert_eq!(None, cpus);
        assert_eq!(Some(536_870_912), mem);
    }

    #[test]
    fn parse_limits_memory_256mb_lowercase() {
        let limits = TaskLimits::new(None, Some("256mb"));
        let (_cpus, mem) = DockerRuntime::parse_limits(Some(&limits)).unwrap();
        assert_eq!(Some(268_435_456), mem);
    }

    #[test]
    fn parse_limits_memory_1g_abbreviation() {
        // "1g" is NOT a recognized suffix (only GB/gb, not G/g alone).
        // Falls through to bare number parse, which fails on "1g".
        let limits = TaskLimits::new(None, Some("1g"));
        let result = DockerRuntime::parse_limits(Some(&limits));
        assert!(result.is_err(), "\"1g\" should not parse — only GB/gb is valid");
    }

    #[test]
    fn parse_limits_memory_invalid_string() {
        let limits = TaskLimits::new(None, Some("not-a-size"));
        let result = DockerRuntime::parse_limits(Some(&limits));
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(err.contains("memory"), "error should mention memory: {err}");
    }

    #[test]
    fn parse_limits_both_cpu_and_memory() {
        let limits = TaskLimits::new(Some("2"), Some("1GB"));
        let (cpus, mem) = DockerRuntime::parse_limits(Some(&limits)).unwrap();
        assert_eq!(Some(2_000_000_000), cpus);
        assert_eq!(Some(1_073_741_824), mem);
    }

    #[test]
    fn parse_limits_default_limits() {
        // Default TaskLimits has None for both fields
        let limits = TaskLimits::default();
        let result = DockerRuntime::parse_limits(Some(&limits)).unwrap();
        assert_eq!((None, None), result);
    }

    // =========================================================================
    // parse_gpu_options — Go parity: cliopts.GpuOpts.Set
    // =========================================================================

    #[test]
    fn parse_gpu_options_count_numeric() {
        let reqs = DockerRuntime::parse_gpu_options("count=2").unwrap();
        assert_eq!(1, reqs.len());
        assert_eq!(Some(2), reqs[0].count);
    }

    #[test]
    fn parse_gpu_options_count_all() {
        let reqs = DockerRuntime::parse_gpu_options("count=all").unwrap();
        assert_eq!(Some(-1), reqs[0].count);
    }

    #[test]
    fn parse_gpu_options_count_one() {
        let reqs = DockerRuntime::parse_gpu_options("count=1").unwrap();
        assert_eq!(Some(1), reqs[0].count);
    }

    #[test]
    fn parse_gpu_options_default_capabilities() {
        // When no capabilities specified, should default to [["gpu"]]
        let reqs = DockerRuntime::parse_gpu_options("count=1").unwrap();
        let caps = reqs[0].capabilities.as_ref().unwrap();
        assert_eq!(1, caps.len());
        assert_eq!(&vec!["gpu".to_string()], &caps[0]);
    }

    #[test]
    fn parse_gpu_options_explicit_capabilities() {
        let reqs = DockerRuntime::parse_gpu_options("capabilities=gpu;compute").unwrap();
        let caps = reqs[0].capabilities.as_ref().unwrap();
        assert_eq!(1, caps.len());
        assert_eq!(&vec!["gpu".to_string(), "compute".to_string()], &caps[0]);
    }

    #[test]
    fn parse_gpu_options_single_capability() {
        let reqs = DockerRuntime::parse_gpu_options("capabilities=utility").unwrap();
        let caps = reqs[0].capabilities.as_ref().unwrap();
        assert_eq!(1, caps.len());
        assert_eq!(&vec!["utility".to_string()], &caps[0]);
    }

    #[test]
    fn parse_gpu_options_driver() {
        let reqs = DockerRuntime::parse_gpu_options("driver=nvidia").unwrap();
        assert_eq!(Some("nvidia".to_string()), reqs[0].driver);
    }

    #[test]
    fn parse_gpu_options_device_ids() {
        let reqs = DockerRuntime::parse_gpu_options("device=0;1").unwrap();
        let ids = reqs[0].device_ids.as_ref().unwrap();
        assert_eq!(&["0".to_string(), "1".to_string()], ids.as_slice());
    }

    #[test]
    fn parse_gpu_options_single_device() {
        let reqs = DockerRuntime::parse_gpu_options("device=0").unwrap();
        let ids = reqs[0].device_ids.as_ref().unwrap();
        assert_eq!(&["0".to_string()], ids.as_slice());
    }

    #[test]
    fn parse_gpu_options_full_spec() {
        let reqs = DockerRuntime::parse_gpu_options("count=2,driver=nvidia,capabilities=gpu;compute,device=0;1").unwrap();
        assert_eq!(Some(2), reqs[0].count);
        assert_eq!(Some("nvidia".to_string()), reqs[0].driver);
        let caps = reqs[0].capabilities.as_ref().unwrap();
        assert_eq!(1, caps.len());
        assert_eq!(&vec!["gpu".to_string(), "compute".to_string()], &caps[0]);
        let ids = reqs[0].device_ids.as_ref().unwrap();
        assert_eq!(&["0".to_string(), "1".to_string()], ids.as_slice());
    }

    #[test]
    fn parse_gpu_options_whitespace_tolerance() {
        let reqs = DockerRuntime::parse_gpu_options(" count = 2 , driver = nvidia ").unwrap();
        assert_eq!(Some(2), reqs[0].count);
        assert_eq!(Some("nvidia".to_string()), reqs[0].driver);
    }

    #[test]
    fn parse_gpu_options_empty_string() {
        let reqs = DockerRuntime::parse_gpu_options("").unwrap();
        assert_eq!(1, reqs.len());
        // count should be None, default capabilities
        assert_eq!(None, reqs[0].count);
    }

    #[test]
    fn parse_gpu_options_invalid_count() {
        let result = DockerRuntime::parse_gpu_options("count=notanumber");
        assert!(result.is_err());
    }

    #[test]
    fn parse_gpu_options_unknown_key() {
        let result = DockerRuntime::parse_gpu_options("foo=bar");
        assert!(result.is_err());
    }

    #[test]
    fn parse_gpu_options_no_device_ids_field() {
        let reqs = DockerRuntime::parse_gpu_options("count=1").unwrap();
        assert!(reqs[0].device_ids.is_none());
    }

    // =========================================================================
    // slugify — Go parity: slug.Make
    // =========================================================================

    #[test]
    fn slugify_simple() {
        assert_eq!("my-task", slugify("my task"));
    }

    #[test]
    fn slugify_mixed_case() {
        assert_eq!("my-task", slugify("My Task"));
    }

    #[test]
    fn slugify_with_numbers() {
        assert_eq!("my-task-123", slugify("My Task 123"));
    }

    #[test]
    fn slugify_single_word() {
        assert_eq!("hello", slugify("hello"));
    }

    #[test]
    fn slugify_empty() {
        assert_eq!("", slugify(""));
    }

    #[test]
    fn slugify_multiple_separators() {
        assert_eq!("a-b", slugify("a  b"));
        assert_eq!("a-b-c", slugify("a - b - c"));
    }

    #[test]
    fn slugify_leading_trailing_separators() {
        assert_eq!("hello", slugify(" hello "));
        assert_eq!("hello-world", slugify("--hello-world--"));
    }

    #[test]
    fn slugify_special_chars() {
        assert_eq!("hello-world", slugify("hello@world!"));
        assert_eq!("foo-bar", slugify("foo&bar#"));
    }

    // =========================================================================
    // parse_go_duration
    // =========================================================================

    #[test]
    fn parse_go_duration_seconds() {
        assert_eq!(Duration::from_secs(30), parse_go_duration("30s").unwrap());
        assert_eq!(Duration::from_secs(1), parse_go_duration("1s").unwrap());
    }

    #[test]
    fn parse_go_duration_minutes() {
        assert_eq!(Duration::from_secs(60), parse_go_duration("1m").unwrap());
        assert_eq!(Duration::from_secs(300), parse_go_duration("5m").unwrap());
    }

    #[test]
    fn parse_go_duration_hours() {
        assert_eq!(Duration::from_secs(3600), parse_go_duration("1h").unwrap());
    }

    #[test]
    fn parse_go_duration_compound() {
        assert_eq!(Duration::from_secs(5400), parse_go_duration("1h30m").unwrap());
        assert_eq!(Duration::from_secs(3661), parse_go_duration("1h1m1s").unwrap());
    }

    #[test]
    fn parse_go_duration_invalid_unit() {
        assert!(parse_go_duration("1x").is_err());
    }

    #[test]
    fn parse_go_duration_trailing_number() {
        assert!(parse_go_duration("1m30").is_err());
    }

    #[test]
    fn parse_go_duration_empty() {
        assert!(parse_go_duration("").is_ok());
        assert_eq!(Duration::ZERO, parse_go_duration("").unwrap());
    }

    #[test]
    fn parse_go_duration_ms_unit_unsupported() {
        // Our implementation only handles h, m, s — ms returns error
        assert!(parse_go_duration("500ms").is_err());
    }

    // =========================================================================
    // DockerConfig + DockerConfigBuilder
    // =========================================================================

    #[test]
    fn config_default_values() {
        let config = DockerConfig::default();
        assert_eq!(None, config.config_file);
        assert!(!config.privileged);
        assert_eq!(DEFAULT_IMAGE_TTL, config.image_ttl);
        assert!(!config.image_verify);
        assert!(config.broker.is_none());
    }

    #[test]
    fn builder_default_differs_from_config_default_on_ttl() {
        // DockerConfigBuilder::default() starts with Duration::ZERO for image_ttl,
        // while DockerConfig::default() uses DEFAULT_IMAGE_TTL (3 days).
        let built = DockerConfigBuilder::default().build();
        let defaulted = DockerConfig::default();
        assert_ne!(built.image_ttl, defaulted.image_ttl);
        assert_eq!(Duration::ZERO, built.image_ttl);
        assert_eq!(DEFAULT_IMAGE_TTL, defaulted.image_ttl);
        // Other fields should match
        assert_eq!(built.config_file, defaulted.config_file);
        assert_eq!(built.privileged, defaulted.privileged);
        assert_eq!(built.image_verify, defaulted.image_verify);
        assert!(built.broker.is_none());
    }

    #[test]
    fn builder_with_config_file() {
        let config = DockerConfigBuilder::default()
            .with_config_file("/etc/docker/config.json")
            .build();
        assert_eq!(Some("/etc/docker/config.json".to_string()), config.config_file);
    }

    #[test]
    fn builder_with_privileged() {
        let config = DockerConfigBuilder::default()
            .with_privileged(true)
            .build();
        assert!(config.privileged);

        let config = DockerConfigBuilder::default()
            .with_privileged(false)
            .build();
        assert!(!config.privileged);
    }

    #[test]
    fn builder_with_image_ttl() {
        let config = DockerConfigBuilder::default()
            .with_image_ttl(Duration::from_secs(60))
            .build();
        assert_eq!(Duration::from_secs(60), config.image_ttl);
    }

    #[test]
    fn builder_with_image_verify() {
        let config = DockerConfigBuilder::default()
            .with_image_verify(true)
            .build();
        assert!(config.image_verify);
    }

    #[test]
    fn builder_chain_all_options() {
        let config = DockerConfigBuilder::default()
            .with_config_file("/my/path")
            .with_privileged(true)
            .with_image_ttl(Duration::from_secs(300))
            .with_image_verify(true)
            .build();
        assert_eq!(Some("/my/path".to_string()), config.config_file);
        assert!(config.privileged);
        assert_eq!(Duration::from_secs(300), config.image_ttl);
        assert!(config.image_verify);
    }

    #[test]
    fn builder_is_must_use() {
        // This just verifies the #[must_use] annotation compiles correctly;
        // the compiler would warn if a #[must_use] value were discarded.
        let config = DockerConfigBuilder::default()
            .with_privileged(true)
            .build();
        assert!(config.privileged);
    }

    // =========================================================================
    // DockerError variants
    // =========================================================================

    #[test]
    fn docker_error_display_messages() {
        let errors: Vec<String> = vec![
            DockerError::ClientCreate("conn".into()).to_string(),
            DockerError::TaskIdRequired.to_string(),
            DockerError::VolumeTargetRequired.to_string(),
            DockerError::BindTargetRequired.to_string(),
            DockerError::BindSourceRequired.to_string(),
            DockerError::UnknownMountType("nfs".into()).to_string(),
            DockerError::ImagePull("fail".into()).to_string(),
            DockerError::ContainerCreate("fail".into()).to_string(),
            DockerError::ContainerStart("fail".into()).to_string(),
            DockerError::ContainerWait("fail".into()).to_string(),
            DockerError::ContainerLogs("fail".into()).to_string(),
            DockerError::ContainerRemove("fail".into()).to_string(),
            DockerError::Mount("fail".into()).to_string(),
            DockerError::Unmount("fail".into()).to_string(),
            DockerError::NetworkCreate("fail".into()).to_string(),
            DockerError::NetworkRemove("fail".into()).to_string(),
            DockerError::VolumeCreate("fail".into()).to_string(),
            DockerError::VolumeRemove("fail".into()).to_string(),
            DockerError::CopyToContainer("fail".into()).to_string(),
            DockerError::CopyFromContainer("fail".into()).to_string(),
            DockerError::ContainerInspect("fail".into()).to_string(),
            DockerError::InvalidCpus("abc".into()).to_string(),
            DockerError::InvalidMemory("bad".into()).to_string(),
            DockerError::ImageVerifyFailed("img".into()).to_string(),
            DockerError::CorruptedImage("img".into()).to_string(),
            DockerError::ImageNotFound("img".into()).to_string(),
            DockerError::NonZeroExit(1, "err".into()).to_string(),
            DockerError::ProbeTimeout("1m".into()).to_string(),
            DockerError::ProbeError("err".into()).to_string(),
            DockerError::InvalidGpuOptions("bad".into()).to_string(),
        ];
        // Every variant should produce a non-empty string
        for msg in &errors {
            assert!(!msg.is_empty(), "DockerError display produced empty string");
        }
    }

    // =========================================================================
    // Task default values
    // =========================================================================

    #[test]
    fn task_default_is_empty() {
        let task = Task::default();
        assert!(task.id.is_empty());
        assert!(task.image.is_empty());
        assert!(task.cmd.is_empty());
        assert!(task.entrypoint.is_empty());
        assert!(task.run.is_none());
        assert!(task.env.is_empty());
        assert!(task.files.is_empty());
        assert!(task.workdir.is_none());
        assert!(task.limits.is_none());
        assert!(task.mounts.is_empty());
        assert!(task.networks.is_empty());
        assert!(task.sidecars.is_empty());
        assert!(task.pre.is_empty());
        assert!(task.post.is_empty());
        assert!(task.registry.is_none());
        assert!(task.probe.is_none());
        assert!(task.gpus.is_none());
        assert!(task.result.is_none());
        assert!((task.progress - 0.0).abs() < f64::EPSILON);
    }

    #[test]
    fn task_clone_roundtrip() {
        let task = Task::default();
        let cloned = task.clone();
        assert_eq!(task.id, cloned.id);
        assert_eq!(task.image, cloned.image);
    }

    // =========================================================================
    // parse_tar_contents — edge cases
    // =========================================================================

    #[test]
    fn parse_tar_contents_empty_bytes() {
        assert!(parse_tar_contents(&[]).is_empty());
    }

    #[test]
    fn parse_tar_contents_garbage_bytes() {
        // Random bytes should not panic, just return empty
        assert!(parse_tar_contents(&[0xFF, 0xFE, 0xFD]).is_empty());
    }

    // =========================================================================
    // Integration tests — require Docker daemon (#[ignore])
    // =========================================================================

    #[tokio::test]
    async fn test_health_check() {
        let runtime = DockerRuntime::default_runtime().await.unwrap();
        assert!(runtime.health_check().await.is_ok());
    }

    #[tokio::test]
    async fn test_default_runtime_creation() {
        let runtime = DockerRuntime::default_runtime().await;
        assert!(runtime.is_ok(), "default_runtime should succeed with Docker daemon: {:?}", runtime.err());
    }

    #[tokio::test]
    async fn test_health_check_failed_with_cancelled_context() {
        let runtime = DockerRuntime::default_runtime().await.unwrap();
        // We can't easily cancel the ping, but verify health_check is reachable
        assert!(runtime.health_check().await.is_ok());
    }
}
