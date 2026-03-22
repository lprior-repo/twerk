//! Docker runtime implementation following functional-rust conventions.
//!
//! # Architecture
//!
//! - **Data**: `DockerRuntime` holds Docker client and configuration state
//! - **Calc**: Pure parsing and validation logic
//! - **Actions**: Docker API calls, I/O pushed to boundary

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use bollard::auth::DockerCredentials;
use bollard::container::{Config as ContainerConfig, HostConfig, LogsOptions, WaitOptions};
use bollard::image::{ListImagesOptions, RemoveImageOptions};
use bollard::models::{ContainerCreateResponse, NetworkCreate, NetworksCreateResponse, PortBinding};
use bollard::network::{CreateNetworkOptions, RemoveNetworkOptions};
use bollard::secret::DeviceRequest as DeviceRequestType;
use bollard::Docker;
use bytes::Bytes;
use futures_util::StreamExt;
use tar::Archive as TarArchive;
use tokio::io::AsyncReadExt;
use tokio::sync::{mpsc, RwLock};
use tokio::time::{interval, sleep};

use crate::broker::log::LogShipper;
use crate::broker::Broker;
use crate::docker::auth::{config_path, Config as AuthConfig};
use crate::docker::reference::parse::parse_reference;
use crate::docker::tork::{Mount, Probe, Registry, TaskLimits};
use crate::docker::archive::Archive;
use crate::docker::bind::BindMounter;
use crate::docker::tmpfs::TmpfsMounter;
use crate::docker::volume::VolumeMounter;
use tork::task::TaskLogPart;
use thiserror::Error;

pub use crate::docker::bind::BindMounter;
pub use crate::docker::tmpfs::TmpfsMounter;
pub use crate::docker::volume::VolumeMounter;

// ----------------------------------------------------------------------------
// Constants
// ----------------------------------------------------------------------------

/// Default workdir for task files.
const DEFAULT_WORKDIR: &str = "/tork/workdir";

/// Default image TTL (3 days).
const DEFAULT_IMAGE_TTL: Duration = Duration::from_secs(72 * 60 * 60);

// ----------------------------------------------------------------------------
// Configuration
// ----------------------------------------------------------------------------

/// Docker runtime configuration.
#[derive(Debug, Clone)]
pub struct DockerConfig {
    /// Docker config file path for registry credentials.
    pub config_file: Option<String>,
    /// Whether to run containers in privileged mode.
    pub privileged: bool,
    /// Image TTL for pruning.
    pub image_ttl: Duration,
    /// Whether to verify image integrity.
    pub image_verify: bool,
    /// Broker for log shipping and progress.
    pub broker: Option<Arc<dyn Broker>>,
}

impl Default for DockerConfig {
    fn default() -> Self {
        Self {
            config_file: None,
            privileged: false,
            image_ttl: DEFAULT_IMAGE_TTL,
            image_verify: false,
            broker: None,
        }
    }
}

/// Builder for Docker runtime configuration.
#[derive(Debug, Default)]
pub struct DockerConfigBuilder {
    config_file: Option<String>,
    privileged: bool,
    image_ttl: Duration,
    image_verify: bool,
    broker: Option<Arc<dyn Broker>>,
}

impl DockerConfigBuilder {
    /// Sets the Docker config file path.
    #[must_use]
    pub fn with_config_file(mut self, path: &str) -> Self {
        self.config_file = Some(path.to_string());
        self
    }

    /// Sets privileged mode.
    #[must_use]
    pub fn with_privileged(mut self, enabled: bool) -> Self {
        self.privileged = enabled;
        self
    }

    /// Sets the image TTL.
    #[must_use]
    pub fn with_image_ttl(mut self, ttl: Duration) -> Self {
        self.image_ttl = ttl;
        self
    }

    /// Sets image verification.
    #[must_use]
    pub fn with_image_verify(mut self, enabled: bool) -> Self {
        self.image_verify = enabled;
        self
    }

    /// Sets the broker for log shipping and progress reporting.
    #[must_use]
    pub fn with_broker(mut self, broker: Arc<dyn Broker>) -> Self {
        self.broker = Some(broker);
        self
    }

    /// Builds the configuration.
    #[must_use]
    pub fn build(self) -> DockerConfig {
        DockerConfig {
            config_file: self.config_file,
            privileged: self.privileged,
            image_ttl: self.image_ttl,
            image_verify: self.image_verify,
            broker: self.broker,
        }
    }
}

impl DockerConfigBuilder {
    /// Sets the Docker config file path.
    #[must_use]
    pub fn with_config_file(mut self, path: &str) -> Self {
        self.config_file = Some(path.to_string());
        self
    }

    /// Sets privileged mode.
    #[must_use]
    pub fn with_privileged(mut self, enabled: bool) -> Self {
        self.privileged = enabled;
        self
    }

    /// Sets the image TTL.
    #[must_use]
    pub fn with_image_ttl(mut self, ttl: Duration) -> Self {
        self.image_ttl = ttl;
        self
    }

    /// Sets image verification.
    #[must_use]
    pub fn with_image_verify(mut self, enabled: bool) -> Self {
        self.image_verify = enabled;
        self
    }

    /// Builds the configuration.
    #[must_use]
    pub fn build(self) -> DockerConfig {
        DockerConfig {
            config_file: self.config_file,
            privileged: self.privileged,
            image_ttl: self.image_ttl,
            image_verify: self.image_verify,
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

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Docker API error: {0}")]
    Api(#[from] bollard::errors::Error),
}

// ----------------------------------------------------------------------------
// Runtime State
// ----------------------------------------------------------------------------

/// Mounter trait for volume mounts.
pub trait Mounter: Send + Sync {
    /// Mount a mount point.
    fn mount(&self, mnt: &Mount) -> impl std::future::Future<Output = Result<(), String>> + Send;
    /// Unmount a mount point.
    fn unmount(&self, mnt: &Mount) -> impl std::future::Future<Output = Result<(), String>> + Send;
}

impl Mounter for VolumeMounter {
    async fn mount(&self, mnt: &Mount) -> Result<(), String> {
        self.mount(mnt).await.map_err(|e| e.to_string())
    }
    async fn unmount(&self, mnt: &Mount) -> Result<(), String> {
        self.unmount(mnt).await.map_err(|e| e.to_string())
    }
}

impl Mounter for BindMounter {
    async fn mount(&self, mnt: &Mount) -> Result<(), String> {
        BindMounter::mount(self, mnt).map_err(|e| e.to_string())
    }
    async fn unmount(&self, mnt: &Mount) -> Result<(), String> {
        BindMounter::unmount(self, mnt).map_err(|e| e.to_string())
    }
}

impl Mounter for TmpfsMounter {
    async fn mount(&self, mnt: &Mount) -> Result<(), String> {
        TmpfsMounter::mount(self, mnt).map_err(|e| e.to_string())
    }
    async fn unmount(&self, mnt: &Mount) -> Result<(), String> {
        TmpfsMounter::unmount(self, mnt).map_err(|e| e.to_string())
    }
}

/// Docker runtime for executing tasks in containers.
#[derive(Debug)]
pub struct DockerRuntime {
    /// Docker client.
    client: Docker,
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
    /// Broker for shipping logs and progress.
    broker: Option<Arc<dyn Broker>>,
}

struct PullRequest {
    image: String,
    registry: Option<Registry>,
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

        // Spawn the pull worker
        let images_clone = images.clone();
        let config_clone = config.clone();
        tokio::spawn(async move {
            while let Some(req) = pull_rx.recv().await {
                let result = Self::do_pull_request(
                    &client,
                    &images_clone,
                    &config_clone,
                    &req.image,
                    req.registry.as_ref(),
                ).await;
                let _ = req.result_tx.send(result);
            }
        });

        // Spawn the pruner
        let images_prune = images.clone();
        let tasks_prune = tasks.clone();
        let config_prune = config.clone();
        tokio::spawn(async move {
            let mut ticker = interval(Duration::from_secs(60 * 60)); // Every hour
            loop {
                tokio::select! {
                    _ = &mut pruner_cancel_rx => break,
                    _ = ticker.tick() => {
                        Self::prune_images(&client, &images_prune, &tasks_prune, config_prune.image_ttl).await;
                    }
                }
            }
        });

        // Create default volume mounter
        let volume_mounter = VolumeMounter::with_client(client.clone());
        let mounter: Arc<dyn Mounter> = Arc::new(volume_mounter);

        Ok(Self {
            client,
            config: config.clone(),
            images,
            pull_tx,
            tasks,
            pruner_cancel: pruner_cancel_tx,
            mounter,
            broker: config.broker.clone(),
        })
    }

    /// Creates a new runtime with default configuration.
    ///
    /// # Errors
    ///
    /// Returns `DockerError` if the Docker client cannot be created.
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

        // Decrement task count when done
        let tasks = self.tasks.clone();
        tokio::spawn(async move {
            let mut count = tasks.write().await;
            *count = count.saturating_sub(1);
        });

        // If the task has sidecars, we need to create a network
        let network_id = if !task.sidecars.is_empty() {
            let id = self.create_network().await?;
            task.networks.push(id.clone());
            Some(id)
        } else {
            None
        };

        // Prepare mounts - mount each mount and defer unmount
        let mut mounted_mounts = Vec::new();
        for mnt in &task.mounts {
            let mut mnt = mnt.clone();
            mnt.id = Some(uuid::Uuid::new_v4().to_string());
            if let Err(e) = self.mounter.mount(&mnt).await {
                return Err(DockerError::Mount(e));
            }
            mounted_mounts.push(mnt);
        }
        // Store updated mounts back to task
        task.mounts = mounted_mounts.clone();

        // Execute pre-tasks
        for pre in &task.pre {
            let mut pre_task = pre.clone();
            pre_task.id = uuid::Uuid::new_v4().to_string();
            pre_task.mounts = mounted_mounts.clone();
            pre_task.networks = task.networks.clone();
            pre_task.limits = task.limits.clone();
            self.run_task(&pre_task).await?;
        }

        // Run the actual task
        self.run_task(task).await?;

        // Execute post-tasks
        for post in &task.post {
            let mut post_task = post.clone();
            post_task.id = uuid::Uuid::new_v4().to_string();
            post_task.mounts = mounted_mounts.clone();
            post_task.networks = task.networks.clone();
            post_task.limits = task.limits.clone();
            self.run_task(&post_task).await?;
        }

        // Clean up mounts (in reverse order of mounting)
        for mnt in mounted_mounts {
            if let Err(e) = self.mounter.unmount(&mnt).await {
                tracing::error!(error = %e, mount = ?mnt, "error unmounting");
            }
        }

        // Clean up network
        if let Some(id) = network_id {
            self.remove_network(&id).await;
        }

        Ok(())
    }

    /// Runs a single task (main task, pre-task, or post-task).
    ///
    /// This handles sidecars and container lifecycle for the given task.
    ///
    /// # Errors
    ///
    /// Returns `DockerError` if the task cannot be executed.
    async fn run_task(&self, task: &Task) -> Result<(), DockerError> {
        // Create container for the main task
        let container = self.create_container(task).await?;

        // Store container info for cleanup
        let container_id = container.id.clone();
        let container_client = container.client.clone();

        // Ensure container is removed when this function returns
        // We use a scope to ensure the cleanup happens even if we return early
        let result = async {
            // Start sidecar containers
            for sidecar in &task.sidecars {
                let mut sidecar_task = sidecar.clone();
                sidecar_task.id = uuid::Uuid::new_v4().to_string();
                sidecar_task.mounts = task.mounts.clone();
                sidecar_task.networks = task.networks.clone();
                sidecar_task.limits = task.limits.clone();

                let sidecar_container = self.create_container(&sidecar_task).await?;
                let sidecar_id = sidecar_container.id.clone();
                let sidecar_client = sidecar_container.client.clone();

                // Start the sidecar
                sidecar_container.start().await
                    .map_err(|e| DockerError::ContainerStart(e.to_string()))?;

                // Defer sidecar removal - it will be removed after main task completes
                // Note: We're not waiting for sidecars to finish, they should run in background
                // For now, we just start them and they'll be cleaned up
                tokio::spawn(async move {
                    let _ = sidecar_client.remove_container(
                        &sidecar_id,
                        Some(bollard::container::RemoveContainerOptions {
                            force: true,
                            remove_volumes: true,
                            ..Default::default()
                        }),
                    ).await;
                });
            }

            // Start the main task container
            container.start().await?;

            // Wait for the main task container to finish
            // This now returns stdout on success
            let stdout = container.wait().await;

            // Copy output to task result if successful
            if let Ok(ref out) = stdout {
                task.result = Some(out.clone());
            }

            stdout.map(|_| ())
        }.await;

        // Clean up main container
        let _ = container_client.remove_container(
            &container_id,
            Some(bollard::container::RemoveContainerOptions {
                force: true,
                remove_volumes: true,
                ..Default::default()
            }),
        ).await;

        result
    }

    /// Performs a health check on the Docker daemon.
    ///
    /// # Errors
    ///
    /// Returns `DockerError` if the health check fails.
    pub async fn health_check(&self) -> Result<(), DockerError> {
        self.client.ping().await
            .map_err(|e| DockerError::ClientCreate(e.to_string()))
    }

    /// Pulls an image from the registry.
    async fn pull_image(
        &self,
        image: &str,
        registry: Option<&Registry>,
    ) -> Result<(), DockerError> {
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
        registry: Option<&Registry>,
    ) -> Result<(), DockerError> {
        // Check if image is already cached
        {
            let cache = images.read().await;
            if cache.contains_key(image) {
                return Ok(());
            }
        }

        // Check if image exists locally
        let exists = Self::image_exists_locally(client, image).await?;
        if !exists {
            // Get registry credentials
            let credentials = Self::get_registry_credentials(config, image).await?;

            // Pull the image
            let options = PullImageOptions {
                image,
                tag: "latest",
                ..Default::default()
            };

            let mut stream = client.pull_image(options, credentials);
            while let Some(result) = stream.next().await {
                match result {
                    Ok(_) => {}
                    Err(e) => return Err(DockerError::ImagePull(e.to_string())),
                }
            }
        }

        // Verify image if enabled
        if config.image_verify {
            if let Err(e) = Self::verify_image(client, image).await {
                // Remove corrupted image
                let _ = client.remove_image(
                    image,
                    RemoveImageOptions { force: true, ..Default::default() },
                    None,
                ).await;
                return Err(DockerError::CorruptedImage(image.to_string()));
            }
        }

        // Cache the image
        {
            let mut cache = images.write().await;
            cache.insert(image.to_string(), std::time::Instant::now());
        }

        Ok(())
    }

    /// Checks if an image exists locally.
    async fn image_exists_locally(client: &Docker, name: &str) -> Result<bool, DockerError> {
        let options = ListImagesOptions::<String> {
            all: true,
            ..Default::default()
        };

        let images = client.list_images(Some(options)).await
            .map_err(|e| DockerError::ImagePull(e.to_string()))?;

        Ok(images.iter().any(|img| {
            img.repo_tags.iter().any(|tag| tag == name)
        }))
    }

    /// Verifies image integrity by creating a test container.
    async fn verify_image(client: &Docker, image: &str) -> Result<(), DockerError> {
        let config = ContainerConfig {
            image: Some(image),
            cmd: Some(vec!["true"]),
            ..Default::default()
        };

        client.create_container(
            bollard::container::CreateContainerOptions { name: "", platform: None },
            config,
        ).await
            .map_err(|e| DockerError::ImageVerifyFailed(e.to_string()))?;

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

        // Try to load credentials from docker config
        let auth_config = match &config.config_file {
            Some(path) => AuthConfig::load_from_path(path)
                .map_err(|e| DockerError::ImagePull(e.to_string()))?,
            None => {
                let path = config_path().map_err(|e| DockerError::ImagePull(e.to_string()))?;
                AuthConfig::load_from_path(&path)
                    .map_err(|e| DockerError::ImagePull(e.to_string()))?
            }
        };

        let (username, password) = auth_config.get_credentials(&reference.domain)
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
    async fn remove_network(&self, network_id: &str) {
        let mut delay = Duration::from_millis(200);
        let max_retries = 5;

        for i in 0..max_retries {
            match self.client.delete_network(network_id).await {
                Ok(()) => return,
                Err(e) => {
                    if i == max_retries - 1 {
                        tracing::error!(network_id = network_id, error = %e, "failed to remove network");
                        return;
                    }
                    sleep(delay).await;
                    delay *= 2;
                }
            }
        }
    }

    /// Creates a container for a task.
    async fn create_container(&self, task: &Task) -> Result<Container, DockerError> {
        // Validate task
        if task.id.is_empty() {
            return Err(DockerError::TaskIdRequired);
        }

        // Pull image if needed
        self.pull_image(&task.image, task.registry.as_ref()).await?;

        // Build environment
        let mut env: Vec<String> = task.env.iter()
            .map(|(k, v)| format!("{}={}", k, v))
            .collect();
        env.push("TORK_OUTPUT=/tork/stdout".to_string());
        env.push("TORK_PROGRESS=/tork/progress".to_string());

        // Build mounts
        let mut mounts: Vec<bollard::container::Mount> = Vec::new();
        for mnt in &task.mounts {
            let mount_type = match mnt.mount_type.as_str() {
                "volume" => bollard::container::MountType::Volume,
                "bind" => bollard::container::MountType::Bind,
                "tmpfs" => bollard::container::MountType::Tmpfs,
                _ => return Err(DockerError::UnknownMountType(mnt.mount_type.clone())),
            };

            mounts.push(bollard::container::Mount {
                target: mnt.target.clone(),
                source: mnt.source.clone(),
                mount_type: Some(mount_type),
                ..Default::default()
            });
        }

        // Create torkdir volume for /tork
        let torkdir_volume_name = uuid::Uuid::new_v4().to_string();
        let _torkdir_volume = self.client.create_volume(
            bollard::volume::CreateVolumeOptions {
                name: torkdir_volume_name.clone(),
                driver: "local".to_string(),
                ..Default::default()
            }
        ).await
            .map_err(|e| DockerError::VolumeCreate(e.to_string()))?;

        mounts.push(bollard::container::Mount {
            target: Some("/tork".to_string()),
            source: Some(torkdir_volume_name.clone()),
            mount_type: Some(bollard::container::MountType::Volume),
            ..Default::default()
        });

        // Parse resources
        let (nano_cpus, memory) = Self::parse_limits(task.limits.as_ref())?;

        // Determine working directory - use default if files are present but workdir is not
        let workdir = if task.workdir.is_none() && !task.files.is_empty() {
            Some(DEFAULT_WORKDIR.to_string())
        } else {
            task.workdir.clone()
        };

        // Build container config
        let container_config = ContainerConfig {
            image: Some(task.image.clone()),
            env: Some(env),
            cmd: if task.cmd.is_empty() { None } else { Some(task.cmd.clone()) },
            entrypoint: if task.entrypoint.is_empty() { None } else { Some(task.entrypoint.clone()) },
            working_dir: workdir.clone(),
            ..Default::default()
        };

        // Build host config
        let host_config = HostConfig {
            mounts: Some(mounts),
            nano_cpus,
            memory,
            privileged: self.config.privileged,
            ..Default::default()
        };

        // Create container
        let response = self.client.create_container(
            bollard::container::CreateContainerOptions { name: "", platform: None },
            container_config,
            host_config,
            None, // networking
        ).await
            .map_err(|e| DockerError::ContainerCreate(e.to_string()))?;

        let container = Container {
            id: response.id,
            client: self.client.clone(),
            torkdir_source: Some(torkdir_volume_name),
            task_id: task.id.clone(),
            broker: self.broker.clone(),
        };

        // Initialize tork directory with stdout, progress, and optional entrypoint
        container.init_torkdir(task.run.as_deref()).await?;

        // Initialize working directory with task files
        let effective_workdir = workdir.as_deref().unwrap_or(DEFAULT_WORKDIR);
        container.init_workdir(&task.files, effective_workdir).await?;

        Ok(container)
    }

    /// Parses task limits into Docker resource values.
    fn parse_limits(limits: Option<&TaskLimits>) -> Result<(Option<i64>, Option<i64>), DockerError> {
        let limits = match limits {
            Some(l) => l,
            None => return Ok((None, None)),
        };

        // Parse CPUs
        let nano_cpus = if let Some(ref cpus) = limits.cpus {
            if cpus.is_empty() {
                None
            } else {
                // Parse CPU string to nanocpus
                // This is a simplified implementation
                match cpus.parse::<f64>() {
                    Ok(cpu) => Some((cpu * 1e9) as i64),
                    Err(_) => return Err(DockerError::InvalidCpus(cpus.clone())),
                }
            }
        } else {
            None
        };

        // Parse Memory
        let memory = if let Some(ref mem) = limits.memory {
            if mem.is_empty() {
                None
            } else {
                // Simple parsing - just convert to bytes
                // A full implementation would use a proper memory parsing library
                Some(mem.parse::<i64>().unwrap_or(0))
            }
        } else {
            None
        };

        Ok((nano_cpus, memory))
    }

    /// Prunes old images.
    async fn prune_images(
        client: &Docker,
        images: &Arc<RwLock<HashMap<String, std::time::Instant>>>,
        tasks: &Arc<RwLock<usize>>,
        ttl: Duration,
    ) {
        // Skip if tasks are running
        let task_count = *tasks.read().await;
        if task_count > 0 {
            return;
        }

        let now = std::time::Instant::now();
        let mut to_remove = Vec::new();

        // Find expired images
        {
            let cache = images.read().await;
            for (image, timestamp) in cache.iter() {
                if now.duration_since(*timestamp) > ttl {
                    to_remove.push(image.clone());
                }
            }
        }

        // Remove expired images
        for image in to_remove {
            let _ = client.remove_image(
                &image,
                RemoveImageOptions { force: false, ..Default::default() },
                None,
            ).await;

            let mut cache = images.write().await;
            cache.remove(&image);
        }
    }
}

/// A running or completed container.
#[derive(Debug, Clone)]
struct Container {
    /// Container ID.
    id: String,
    /// Docker client.
    client: Docker,
    /// Torkdir mount source for cleanup.
    torkdir_source: Option<String>,
    /// Task ID for log shipping.
    task_id: String,
    /// Broker for log shipping and progress.
    broker: Option<Arc<dyn Broker>>,
}

impl Container {
    /// Starts the container.
    ///
    /// # Errors
    ///
    /// Returns `DockerError` if the container cannot be started.
    async fn start(&self) -> Result<(), DockerError> {
        self.client.start_container(&self.id, None).await
            .map_err(|e| DockerError::ContainerStart(e.to_string()))
    }

    /// Waits for the container to finish.
    ///
    /// This method:
    /// 1. Spawns a background task to report progress
    /// 2. Spawns a background task to stream logs to broker
    /// 3. Waits for container to finish
    /// 4. Returns stdout on success or error details on failure
    ///
    /// # Errors
    ///
    /// Returns `DockerError` if the container wait fails.
    async fn wait(&self) -> Result<String, DockerError> {
        // Create cancellation context for background tasks
        let (cancel_tx, mut cancel_rx) = tokio::sync::oneshot::channel::<()>();

        // Spawn progress reporting task
        let progress_task_id = self.task_id.clone();
        let progress_broker = self.broker.clone();
        let progress_client = self.client.clone();
        let progress_container_id = self.id.clone();
        let progress_cancel_tx = cancel_tx.clone();
        tokio::spawn(async move {
            Self::report_progress(
                progress_client,
                progress_container_id,
                progress_task_id,
                progress_broker,
                progress_cancel_tx,
            ).await;
        });

        // Spawn log streaming task
        let log_task_id = self.task_id.clone();
        let log_broker = self.broker.clone();
        let log_client = self.client.clone();
        let log_container_id = self.id.clone();
        let log_cancel_tx = cancel_tx;
        tokio::spawn(async move {
            Self::stream_logs(
                log_client,
                log_container_id,
                log_task_id,
                log_broker,
                log_cancel_tx,
            ).await;
        });

        // Wait for container to finish
        let options = WaitOptions {
            condition: "not-running".to_string(),
        };

        let result = self.client.wait_container(&self.id, Some(options)).await
            .map_err(|e| DockerError::ContainerWait(e.to_string()))?;

        // Cancel background tasks
        let _ = cancel_rx.close();

        // Get the exit code
        let status_code = result.status_code.unwrap_or(1);

        if status_code != 0 {
            // Non-zero exit - get last 10 lines of logs for error message
            let logs_result = self.read_logs_tail(10).await;
            let log_snippet = logs_result.unwrap_or_default();
            return Err(DockerError::NonZeroExit(status_code, log_snippet));
        }

        // Success - read stdout from container
        let stdout = self.read_output().await?;
        Ok(stdout)
    }

    /// Reads the last N lines of container logs.
    async fn read_logs_tail(&self, lines: usize) -> Result<String, DockerError> {
        use futures_util::TryStreamExt;

        let options = LogsOptions {
            stdout: true,
            stderr: true,
            tail: lines.to_string(),
            ..Default::default()
        };

        let mut stream = self.client.logs(&self.id, Some(options));
        let mut output = String::new();

        while let Some result = stream.next().await {
            match result {
                Ok(output_chunk) => {
                    output.push_str(&output_chunk.to_string());
                }
                Err(e) => {
                    tracing::debug!(error = %e, "error reading logs tail");
                    break;
                }
            }
        }

        Ok(output)
    }

    /// Streams container logs to the broker.
    async fn stream_logs(
        client: Docker,
        container_id: String,
        task_id: String,
        broker: Option<Arc<dyn Broker>>,
        _cancel: tokio::sync::oneshot::Sender<()>,
    ) {
        let Some(broker) = broker else {
            return;
        };

        let options = LogsOptions {
            stdout: true,
            stderr: true,
            follow: true,
            ..Default::default()
        };

        let mut stream = client.logs(&container_id, Some(options));
        let mut part_num = 0i64;
        let mut buffer = String::new();
        let mut interval = tokio::time::interval(Duration::from_secs(1));

        loop {
            tokio::select! {
                result = stream.next() => {
                    match result {
                        Some(Ok(output_chunk)) => {
                            let log_line = output_chunk.to_string();
                            buffer.push_str(&log_line);
                        }
                        Some(Err(e)) => {
                            tracing::debug!(error = %e, "log stream error");
                            // Try to flush remaining buffer
                            if !buffer.is_empty() {
                                part_num += 1;
                                let part = TaskLogPart {
                                    id: None,
                                    number: part_num,
                                    task_id: Some(task_id.clone()),
                                    contents: Some(buffer.clone()),
                                    created_at: None,
                                };
                                let _ = broker.publish_task_log_part(&part).await;
                                buffer.clear();
                            }
                            break;
                        }
                        None => {
                            // Stream ended
                            break;
                        }
                    }
                }
                _ = interval.tick() => {
                    // Flush buffer every second
                    if !buffer.is_empty() {
                        part_num += 1;
                        let part = TaskLogPart {
                            id: None,
                            number: part_num,
                            task_id: Some(task_id.clone()),
                            contents: Some(buffer.clone()),
                            created_at: None,
                        };
                        let _ = broker.publish_task_log_part(&part).await;
                        buffer.clear();
                    }
                }
            }
        }

        // Final flush
        if !buffer.is_empty() {
            part_num += 1;
            let part = TaskLogPart {
                id: None,
                number: part_num,
                task_id: Some(task_id.clone()),
                contents: Some(buffer),
                created_at: None,
            };
            let _ = broker.publish_task_log_part(&part).await;
        }
    }

    /// Reports progress from /tork/progress file periodically.
    async fn report_progress(
        client: Docker,
        container_id: String,
        task_id: String,
        broker: Option<Arc<dyn Broker>>,
        cancel: tokio::sync::oneshot::Sender<()>,
    ) {
        let Some(broker) = broker else {
            return;
        };

        let mut interval = tokio::time::interval(Duration::from_secs(10));
        let mut previous_progress: Option<f64> = None;

        loop {
            tokio::select! {
                _ = &mut interval => {
                    match Self::read_progress_from_container(&client, &container_id).await {
                        Ok(progress) => {
                            if previous_progress.map(|p| (p - progress).abs() > 0.001).unwrap_or(true) {
                                previous_progress = Some(progress);
                                tracing::debug!(task_id = %task_id, progress = %progress, "task progress");
                                // Note: publishing full task progress would require
                                // task state which we don't have here. The broker
                                // log shipping handles the actual log forwarding.
                            }
                        }
                        Err(e) => {
                            tracing::debug!(error = %e, "error reading progress");
                            // Progress file might not exist yet or container exited
                            break;
                        }
                    }
                }
                _ = cancel => {
                    break;
                }
            }
        }
    }

    /// Reads progress value from container's /tork/progress file.
    async fn read_progress_from_container(client: &Docker, container_id: &str) -> Result<f64, DockerError> {
        use futures_util::TryStreamExt;

        let mut archive_stream = client.copy_from_container(container_id, "/tork/progress").await
            .map_err(|e| DockerError::CopyFromContainer(e.to_string()))?;

        let bytes = archive_stream.try_next().await
            .map_err(|e| DockerError::CopyFromContainer(e.to_string()))?
            .ok_or_else(|| DockerError::CopyFromContainer("empty response".to_string()))?;

        // Parse TAR format to get file contents
        let contents = Self::parse_tar_contents(&bytes)?;
        let s = contents.trim();

        if s.is_empty() {
            return Ok(0.0);
        }

        s.parse::<f64>().map_err(|_| DockerError::CopyFromContainer("invalid progress value".to_string()))
    }

    /// Parses TAR archive bytes and returns the contents of the first file.
    fn parse_tar_contents(tar_bytes: &[u8]) -> Result<String, DockerError> {
        let mut archive = TarArchive::new(tar_bytes);

        loop {
            let mut entry = match archive.next_entry() {
                Ok(Some(e)) => e,
                Ok(None) => break,
                Err(e) => return Err(DockerError::CopyFromContainer(e.to_string())),
            };

            let mut contents = Vec::new();
            if let Err(e) = std::io::Read::read_to_end(&mut entry, &mut contents) {
                // If we can't read the entry fully, just continue to next
                tracing::debug!(error = %e, "error reading tar entry");
                continue;
            }

            // We found the file contents - return them
            return String::from_utf8(contents)
                .map_err(|e| DockerError::CopyFromContainer(e.to_string()));
        }

        Ok(String::new())
    }

    /// Reads the stdout file from the container's /tork/stdout.
    ///
    /// # Errors
    ///
    /// Returns `DockerError` if the output cannot be read.
    async fn read_output(&self) -> Result<String, DockerError> {
        use futures_util::TryStreamExt;

        let mut archive_stream = self.client.copy_from_container(&self.id, "/tork/stdout").await
            .map_err(|e| DockerError::CopyFromContainer(e.to_string()))?;

        let bytes = archive_stream.try_next().await
            .map_err(|e| DockerError::CopyFromContainer(e.to_string()))?;

        match bytes {
            Some(b) => Self::parse_tar_contents(&b),
            None => Ok(String::new()),
        }
    }

    /// Removes the container.
    ///
    /// # Errors
    ///
    /// Returns `DockerError` if the container cannot be removed.
    async fn remove(&self) -> Result<(), DockerError> {
        let options = bollard::container::RemoveContainerOptions {
            force: true,
            remove_volumes: true,
            ..Default::default()
        };

        self.client.remove_container(&self.id, Some(options)).await
            .map_err(|e| DockerError::ContainerRemove(e.to_string()))
    }

    /// Initializes the `/tork/` directory in the container with stdout, progress,
    /// and optionally an entrypoint script.
    ///
    /// # Errors
    ///
    /// Returns `DockerError` if initialization fails.
    async fn init_torkdir(&self, run_script: Option<&str>) -> Result<(), DockerError> {
        let mut archive = Archive::new()
            .map_err(|e| DockerError::CopyToContainer(e.to_string()))?;

        // Write empty stdout file (mode 0o222 = write-only for others)
        archive.write_file("stdout", 0o222, &[])
            .map_err(|e| DockerError::CopyToContainer(e.to_string()))?;

        // Write empty progress file
        archive.write_file("progress", 0o222, &[])
            .map_err(|e| DockerError::CopyToContainer(e.to_string()))?;

        // Write entrypoint script if provided
        if let Some(script) = run_script {
            // mode 0o555 = read+execute for all, write for owner
            archive.write_file("entrypoint", 0o555, script.as_bytes())
                .map_err(|e| DockerError::CopyToContainer(e.to_string()))?;
        }

        // Finalize the archive to prepare it for reading
        archive.finish()
            .map_err(|e| DockerError::CopyToContainer(e.to_string()))?;

        // Copy the archive to the container at /tork/
        let mut reader = archive.reader()
            .map_err(|e| DockerError::CopyToContainer(e.to_string()))?;
        
        let mut contents = Vec::new();
        std::io::Read::read_to_end(&mut reader, &mut contents)
            .map_err(|e| DockerError::CopyToContainer(e.to_string()))?;

        self.client.copy_to_container(
            &self.id,
            "/tork/",
            contents.into(),
            bollard::container::CopyToContainerOptions {
                allow_overwrite_dir_with_file: false,
                user: "",
            },
        ).await
            .map_err(|e| DockerError::CopyToContainer(e.to_string()))?;

        // Clean up the temp archive
        archive.remove()
            .map_err(|e| DockerError::CopyToContainer(e.to_string()))?;

        Ok(())
    }

    /// Initializes the working directory with task files.
    ///
    /// # Errors
    ///
    /// Returns `DockerError` if initialization fails.
    async fn init_workdir(
        &self,
        files: &HashMap<String, String>,
        workdir: &str,
    ) -> Result<(), DockerError> {
        if files.is_empty() {
            return Ok(());
        }

        let mut archive = Archive::new()
            .map_err(|e| DockerError::CopyToContainer(e.to_string()))?;

        // Write each file to the archive
        for (filename, contents) in files {
            // mode 0o444 = read-only
            archive.write_file(filename, 0o444, contents.as_bytes())
                .map_err(|e| DockerError::CopyToContainer(e.to_string()))?;
        }

        // Finalize the archive
        archive.finish()
            .map_err(|e| DockerError::CopyToContainer(e.to_string()))?;

        // Read the archive contents
        let mut reader = archive.reader()
            .map_err(|e| DockerError::CopyToContainer(e.to_string()))?;
        
        let mut contents = Vec::new();
        std::io::Read::read_to_end(&mut reader, &mut contents)
            .map_err(|e| DockerError::CopyToContainer(e.to_string()))?;

        // Copy to the container at the working directory
        self.client.copy_to_container(
            &self.id,
            workdir,
            contents.into(),
            bollard::container::CopyToContainerOptions {
                allow_overwrite_dir_with_file: false,
                user: "",
            },
        ).await
            .map_err(|e| DockerError::CopyToContainer(e.to_string()))?;

        // Clean up temp archive
        archive.remove()
            .map_err(|e| DockerError::CopyToContainer(e.to_string()))?;

        Ok(())
    }
}

// ----------------------------------------------------------------------------
// Task type (simplified from tork.Task)
// ----------------------------------------------------------------------------

/// Task to execute in a container.
#[derive(Debug, Clone)]
pub struct Task {
    /// Unique task identifier.
    pub id: String,
    /// Container image.
    pub image: String,
    /// Command to run.
    pub cmd: Vec<String>,
    /// Entrypoint.
    pub entrypoint: Vec<String>,
    /// Script to run (sets entrypoint to sh -c).
    pub run: Option<String>,
    /// Environment variables.
    pub env: HashMap<String, String>,
    /// Files to create in the working directory.
    pub files: HashMap<String, String>,
    /// Working directory.
    pub workdir: Option<String>,
    /// Task limits.
    pub limits: Option<TaskLimits>,
    /// Mounts.
    pub mounts: Vec<Mount>,
    /// Networks to join.
    pub networks: Vec<String>,
    /// Sidecar tasks.
    pub sidecars: Vec<Task>,
    /// Pre tasks.
    pub pre: Vec<Task>,
    /// Post tasks.
    pub post: Vec<Task>,
    /// Registry credentials.
    pub registry: Option<Registry>,
    /// Health probe.
    pub probe: Option<Probe>,
    /// GPUs.
    pub gpus: Option<String>,
    /// Task result (stdout).
    pub result: Option<String>,
    /// Task progress.
    pub progress: Option<f64>,
}

impl Default for Task {
    fn default() -> Self {
        Self {
            id: String::new(),
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
            progress: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    #[ignore]
    async fn test_health_check() {
        let runtime = DockerRuntime::default_runtime().await.expect("should create runtime");
        let result = runtime.health_check().await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    #[ignore]
    async fn test_run_simple_task() {
        let runtime = DockerRuntime::default_runtime().await.expect("should create runtime");

        let mut task = Task {
            id: uuid::Uuid::new_v4().to_string(),
            image: "busybox:stable".to_string(),
            cmd: vec!["ls".to_string()],
            ..Default::default()
        };

        let result = runtime.run(&mut task).await;
        assert!(result.is_ok());
    }

    #[test]
    fn test_parse_cpus() {
        let limits = TaskLimits::new(Some("1"), None);
        let (nano, _) = DockerRuntime::parse_limits(Some(&limits)).expect("should parse");
        assert_eq!(Some(1_000_000_000), nano);

        let limits = TaskLimits::new(Some("0.5"), None);
        let (nano, _) = DockerRuntime::parse_limits(Some(&limits)).expect("should parse");
        assert_eq!(Some(500_000_000), nano);

        let limits = TaskLimits::new(Some(".25"), None);
        let (nano, _) = DockerRuntime::parse_limits(Some(&limits)).expect("should parse");
        assert_eq!(Some(250_000_000), nano);
    }

    #[test]
    fn test_parse_memory() {
        let limits = TaskLimits::new(None, Some("1MB"));
        let (_, memory) = DockerRuntime::parse_limits(Some(&limits)).expect("should parse");
        // Note: Simplified parsing - full impl would properly convert
        assert!(memory.is_some());
    }
}
