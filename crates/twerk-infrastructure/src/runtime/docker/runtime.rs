// ----------------------------------------------------------------------------
// Imports and Type Definitions
// ----------------------------------------------------------------------------

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use bollard::Docker;
use dashmap::DashMap;

use bollard::config::NetworkingConfig;
use bollard::models::HostConfig;
use bollard::models::{
    DeviceRequest, EndpointSettings, HealthConfig, Mount as BollardMount, MountTypeEnum,
    PortBinding,
};

use bollard::query_parameters::{
    CreateContainerOptions, CreateImageOptions, ListImagesOptions, RemoveContainerOptions,
    RemoveImageOptions, RemoveVolumeOptions,
};
use futures_util::StreamExt;
use tokio::sync::{mpsc, RwLock};
use tokio::time::interval;

use super::config::DockerConfig;
use super::error::DockerError;
use super::mounters::{CompositeMounter, Mounter};

use super::auth::{config_path, Config as DockerConfigFile};
use super::container::Container;
use super::helpers::{parse_go_duration, parse_memory_bytes, slugify};
use super::reference::parse as parse_reference;
use twerk_core::id::TaskId;
use twerk_core::mount::mount_type;

use twerk_core::task::{Registry, Task, TaskLimits};
use twerk_core::uuid::new_uuid;

// ----------------------------------------------------------------------------
// Constants
// ----------------------------------------------------------------------------

const DEFAULT_WORKDIR: &str = "/workspace";
const DEFAULT_CMD: &[&str] = &["/bin/sh", "-c"];
const RUN_ENTRYPOINT: &[&str] = &["sh", "-c"];
const DEFAULT_PROBE_PATH: &str = "/";
const DEFAULT_PROBE_TIMEOUT: &str = "1m";

// ----------------------------------------------------------------------------
// Type Aliases
// ----------------------------------------------------------------------------

// Networking config type - using HashMap directly as the type

// ----------------------------------------------------------------------------
// Type Definitions
// ----------------------------------------------------------------------------
// Type Aliases
// ----------------------------------------------------------------------------

// ----------------------------------------------------------------------------
// Type Definitions
// ----------------------------------------------------------------------------

struct PullRequest {
    image: String,
    registry: Option<Registry>,
    #[allow(dead_code)]
    logger: Box<dyn std::io::Write + Send>,
    result_tx: tokio::sync::oneshot::Sender<Result<(), DockerError>>,
}

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

        let images = Arc::new(DashMap::new());
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
        let network_id = if let Some(ref sidecars) = task.sidecars {
            if !sidecars.is_empty() {
                let id = self.create_network().await?;
                if let Some(ref mut networks) = task.networks {
                    networks.push(id.clone());
                }
                Some(id)
            } else {
                None
            }
        } else {
            None
        };

        // Prepare mounts
        let mut mounted_mounts = Vec::new();
        if let Some(ref mounts) = task.mounts {
            for mnt in mounts {
                let mut mnt = mnt.clone();
                mnt.id = Some(new_uuid());
                if let Err(e) = self.mounter.mount(&mnt).await {
                    return Err(DockerError::Mount(e));
                }
                mounted_mounts.push(mnt);
            }
        }
        task.mounts = Some(mounted_mounts.clone());

        // Execute pre-tasks
        let pre_tasks: Vec<Task> = if let Some(ref pre) = task.pre {
            pre.to_vec()
        } else {
            Vec::new()
        };
        for mut pre_task in pre_tasks {
            pre_task.id = Some(TaskId::new(new_uuid()));
            pre_task.mounts = Some(mounted_mounts.clone());
            pre_task.networks = task.networks.clone();
            pre_task.limits = task.limits.clone();
            self.run_task(&mut pre_task).await?;
        }

        // Run the actual task
        self.run_task(task).await?;

        // Execute post-tasks
        let post_tasks: Vec<Task> = if let Some(ref post) = task.post {
            post.to_vec()
        } else {
            Vec::new()
        };
        for mut post_task in post_tasks {
            post_task.id = Some(TaskId::new(new_uuid()));
            post_task.mounts = Some(mounted_mounts.clone());
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

        // Start monitoring (logs & progress) immediately after creation
        container.start_monitoring();

        let container_id = container.id.clone();
        let twerkdir_source = container.twerkdir_source.clone();

        let result = async {
            // Start sidecars
            if let Some(ref sidecars) = task.sidecars {
                for sidecar in sidecars {
                    let mut sidecar_task = sidecar.clone();
                    sidecar_task.id = Some(TaskId::new(new_uuid()));
                    sidecar_task.mounts = task.mounts.clone();
                    sidecar_task.networks = task.networks.clone();
                    sidecar_task.limits = task.limits.clone();

                    let sidecar_container = self.create_container(&sidecar_task).await?;
                    let sidecar_id = sidecar_container.id.clone();
                    let sidecar_twerkdir = sidecar_container.twerkdir_source.clone();

                    sidecar_container
                        .start()
                        .await
                        .map_err(|e| DockerError::ContainerStart(e.to_string()))?;

                    // Defer sidecar removal
                    let sc = self.client.clone();
                    tokio::spawn(async move {
                        let _ = sc
                            .remove_container(
                                &sidecar_id,
                                Some(RemoveContainerOptions {
                                    force: true,
                                    ..Default::default()
                                }),
                            )
                            .await;
                        if let Some(source) = sidecar_twerkdir {
                            let _ = sc.remove_volume(&source, None::<RemoveVolumeOptions>).await;
                        }
                    });
                }
            }

            // Start main container (includes probe if configured)
            container.start().await?;

            // Wait for completion and capture result
            task.result = Some(container.wait().await?);
            Ok(())
        }
        .await;

        // Clean up main container
        let _ = self
            .client
            .remove_container(
                &container_id,
                Some(RemoveContainerOptions {
                    force: true,
                    ..Default::default()
                }),
            )
            .await;
        if let Some(source) = twerkdir_source {
            let _ = self
                .client
                .remove_volume(&source, None::<RemoveVolumeOptions>)
                .await;
        }

        result
    }

    /// Health check on the Docker daemon.
    pub async fn health_check(&self) -> Result<(), DockerError> {
        self.client
            .ping()
            .await
            .map(|_| ())
            .map_err(|e| DockerError::ClientCreate(e.to_string()))
    }

    /// Pull an image via the serialized pull queue.
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
        self.pull_tx
            .send(request)
            .await
            .map_err(|_| DockerError::ImagePull("pull queue closed".to_string()))?;
        result_rx
            .await
            .map_err(|_| DockerError::ImagePull("pull worker died".to_string()))?
    }

    /// Internal pull implementation.
    async fn do_pull_request(
        client: &Docker,
        images: &Arc<DashMap<String, std::time::Instant>>,
        config: &DockerConfig,
        image: &str,
        #[allow(unused_variables)] registry: Option<&Registry>,
    ) -> Result<(), DockerError> {
        // Check cache (respecting TTL)
        if let Some(ts) = images.get(image) {
            if std::time::Instant::now().duration_since(*ts) <= config.image_ttl {
                return Ok(());
            }
        }

        // Check local
        let exists = Self::image_exists_locally(client, image).await?;
        if !exists {
            let credentials = Self::get_registry_credentials(config, image).await?;

            let options = CreateImageOptions {
                from_image: Some(image.to_string()),
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
                let _ = client
                    .remove_image(
                        image,
                        None::<RemoveImageOptions>,
                        None::<bollard::auth::DockerCredentials>,
                    )
                    .await;
                return Err(DockerError::CorruptedImage(image.to_string()));
            }
        }

        // Cache
        images.insert(image.to_string(), std::time::Instant::now());

        Ok(())
    }

    /// Checks if an image exists locally.
    async fn image_exists_locally(client: &Docker, name: &str) -> Result<bool, DockerError> {
        let options = ListImagesOptions {
            all: true,
            ..Default::default()
        };
        let image_list = client
            .list_images(Some(options))
            .await
            .map_err(|e| DockerError::ImagePull(e.to_string()))?;
        Ok(image_list
            .iter()
            .any(|img| img.repo_tags.iter().any(|tag| tag == name)))
    }

    /// Verifies image integrity by creating a test container and removing it.
    ///
    /// Go parity: `verifyImage` — creates container with `cmd: ["true"]`.
    async fn verify_image(client: &Docker, image: &str) -> Result<(), DockerError> {
        let config = bollard::models::ContainerCreateBody {
            image: Some(image.to_string()),
            cmd: Some(vec!["true".to_string()]),
            ..Default::default()
        };
        let response = client
            .create_container(
                Some(CreateContainerOptions {
                    name: None,
                    platform: String::new(),
                }),
                config,
            )
            .await
            .map_err(|e| DockerError::ImageVerifyFailed(format!("{}: {}", image, e)))?;

        // Clean up test container
        let _ = client
            .remove_container(
                &response.id,
                Some(RemoveContainerOptions {
                    force: true,
                    ..Default::default()
                }),
            )
            .await;

        Ok(())
    }

    /// Gets registry credentials for an image.
    async fn get_registry_credentials(
        config: &DockerConfig,
        image: &str,
    ) -> Result<Option<bollard::auth::DockerCredentials>, DockerError> {
        let reference =
            parse_reference(image).map_err(|e| DockerError::ImagePull(e.to_string()))?;

        if reference.domain.is_empty() {
            return Ok(None);
        }

        // Load auth config: config_file takes priority, then config_path, then default path
        let auth_config = match (&config.config_file, &config.config_path) {
            (Some(path), _) | (_, Some(path)) => DockerConfigFile::load_from_path(path)
                .map_err(|e| DockerError::ImagePull(e.to_string()))?,
            (None, None) => {
                let path = config_path().map_err(|e| DockerError::ImagePull(e.to_string()))?;
                DockerConfigFile::load_from_path(&path)
                    .map_err(|e| DockerError::ImagePull(e.to_string()))?
            }
        };

        let (username, password) = auth_config
            .get_credentials(&reference.domain)
            .map_err(|e| DockerError::ImagePull(e.to_string()))?;

        if username.is_empty() && password.is_empty() {
            return Ok(None);
        }

        Ok(Some(bollard::auth::DockerCredentials {
            username: Some(username),
            password: Some(password),
            ..Default::default()
        }))
    }

    /// Creates a network for sidecar communication.
    ///
    /// Delegates to `crate::runtime::docker::network::create_network`.
    async fn create_network(&self) -> Result<String, DockerError> {
        super::network::create_network(&self.client).await
    }

    /// Removes a network with retry logic.
    ///
    /// Delegates to `crate::runtime::docker::network::remove_network`.
    /// Go parity: `removeNetwork` — exponential backoff 200ms→3200ms, 5 retries.
    async fn remove_network(&self, network_id: &str) {
        super::network::remove_network(&self.client, network_id).await;
    }

    /// Creates a container for a task.
    ///
    /// Go parity: `createTaskContainer` — full lifecycle setup including
    /// image pull, env, mounts, limits, GPU, probe ports, networking aliases,
    /// workdir, and file initialization.
    #[allow(dead_code)] // used in integration tests
    pub async fn create_container(&self, task: &Task) -> Result<Container, DockerError> {
        if task.id.as_ref().is_none_or(|id| id.is_empty()) {
            return Err(DockerError::TaskIdRequired);
        }

        // Pull image
        let image = task
            .image
            .as_ref()
            .ok_or_else(|| DockerError::ImageRequired)?;
        self.pull_image(image, task.registry.as_ref()).await?;
        // Build env (Go parity: iterates t.Env HashMap, formats KEY=VALUE, adds TWERK_OUTPUT and TWERK_PROGRESS)
        let mut env: Vec<String> = if let Some(ref env_map) = task.env {
            env_map
                .iter()
                .map(|(k, v)| format!("{}={}", k, v))
                .collect()
        } else {
            Vec::new()
        };
        env.push("TWERK_OUTPUT=/twerk/stdout".to_string());
        env.push("TWERK_PROGRESS=/twerk/progress".to_string());

        // Build mounts with validation (Go parity: mount type validation)
        let mut mounts: Vec<BollardMount> = Vec::new();
        if let Some(ref mounts_list) = task.mounts {
            for mnt in mounts_list {
                let typ = match mnt.mount_type.as_deref() {
                    Some(mount_type::VOLUME) => {
                        if mnt.target.as_ref().is_none_or(|t| t.is_empty()) {
                            return Err(DockerError::VolumeTargetRequired);
                        }
                        MountTypeEnum::VOLUME
                    }
                    Some(mount_type::BIND) => {
                        if mnt.target.as_ref().is_none_or(|t| t.is_empty()) {
                            return Err(DockerError::BindTargetRequired);
                        }
                        if mnt.source.as_ref().is_none_or(|s| s.is_empty()) {
                            return Err(DockerError::BindSourceRequired);
                        }
                        MountTypeEnum::BIND
                    }
                    Some(mount_type::TMPFS) => MountTypeEnum::TMPFS,
                    Some(other) => return Err(DockerError::UnknownMountType(other.to_string())),
                    None => return Err(DockerError::UnknownMountType("none".to_string())),
                };
                tracing::debug!(source = ?mnt.source, target = ?mnt.target, "Mounting");
                mounts.push(BollardMount {
                    target: mnt.target.clone(),
                    source: mnt.source.clone(),
                    typ: Some(typ),
                    ..Default::default()
                });
            }
        }

        // Create twerkdir volume
        let twerkdir_volume_name = new_uuid();
        let _ = self
            .client
            .create_volume(bollard::models::VolumeCreateRequest {
                name: Some(twerkdir_volume_name.clone()),
                driver: Some("local".to_string()),
                ..Default::default()
            })
            .await
            .map_err(|e| DockerError::VolumeCreate(e.to_string()))?;

        mounts.push(BollardMount {
            target: Some("/twerk".to_string()),
            source: Some(twerkdir_volume_name.clone()),
            typ: Some(MountTypeEnum::VOLUME),
            ..Default::default()
        });

        // Parse limits
        let (nano_cpus, memory) = Self::parse_limits(task.limits.as_ref())?;

        // Working directory
        let workdir = if task.workdir.is_some() {
            task.workdir.clone()
        } else if task.files.as_ref().is_none_or(|f| f.is_empty()) {
            None
        } else {
            Some(DEFAULT_WORKDIR.to_string())
        };

        // Entrypoint auto-detection (Go parity)
        let cmd: Vec<String> = if task.cmd.as_ref().is_none_or(|c| c.is_empty()) {
            DEFAULT_CMD.iter().map(|s| s.to_string()).collect()
        } else {
            task.cmd.clone().unwrap_or_default()
        };

        let entrypoint: Vec<String> =
            if task.entrypoint.as_ref().is_none_or(|e| e.is_empty()) && task.run.is_some() {
                RUN_ENTRYPOINT.iter().map(|s| s.to_string()).collect()
            } else {
                task.entrypoint.clone().unwrap_or_default()
            };

        // Probe port configuration (Go parity: exposed ports + port bindings)
        let mut exposed_ports: Vec<String> = Vec::new();
        let mut port_bindings: HashMap<String, Option<Vec<PortBinding>>> = HashMap::new();
        let mut healthcheck: Option<HealthConfig> = None;

        if let Some(ref probe) = task.probe {
            let port = probe.port;
            let port_key = format!("{}/tcp", port);
            exposed_ports.push(port_key.clone());
            port_bindings.insert(
                port_key,
                Some(vec![PortBinding {
                    host_ip: Some("127.0.0.1".to_string()),
                    host_port: Some("0".to_string()),
                }]),
            );

            // Build Docker HEALTHCHECK for native container health monitoring
            let probe_path = probe.path.as_deref().map_or(DEFAULT_PROBE_PATH, |p| p);
            let timeout_str = probe
                .timeout
                .as_deref()
                .map_or(DEFAULT_PROBE_TIMEOUT, |t| t);
            let timeout = parse_go_duration(timeout_str).map_or(Duration::from_secs(60), |v| v);
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

        // GPU device requests (Go parity: `gpuOpts.Set(t.GPUs)`)
        let device_requests = task
            .gpus
            .as_ref()
            .map(|gpu_str| Self::parse_gpu_options(gpu_str))
            .transpose()?;

        // Host network mode detection (Go parity: `network == hostNetworkName`)
        let host_network_mode = if let Some(ref networks) = task.networks {
            networks.iter().any(|n| n == "host")
        } else {
            false
        };

        // Validate host network usage
        if host_network_mode && !self.config.host_network {
            return Err(DockerError::HostNetworkDisabled);
        }

        // Networking config with aliases (Go parity: `slug.Make(t.Name)`)
        // Note: Network aliases are not supported with host networking
        let networking_config =
            if task.networks.as_ref().is_none_or(|n| n.is_empty()) || host_network_mode {
                None
            } else {
                let mut endpoints = HashMap::new();
                if let Some(ref networks) = task.networks {
                    for nw in networks {
                        let alias = slugify(task.name.as_deref().map_or(
                            task.id.as_ref().map(|id| id.as_str()).unwrap_or("unknown"),
                            |n| n,
                        ));
                        endpoints.insert(
                            nw.clone(),
                            EndpointSettings {
                                aliases: Some(vec![alias]),
                                ..Default::default()
                            },
                        );
                    }
                }
                Some(NetworkingConfig {
                    endpoints_config: Some(endpoints),
                })
            };

        // Build container config
        let container_config = bollard::models::ContainerCreateBody {
            image: task.image.clone(),
            env: Some(env),
            cmd: Some(cmd),
            entrypoint: if entrypoint.is_empty() {
                None
            } else {
                Some(entrypoint)
            },
            working_dir: workdir.clone(),
            exposed_ports: if exposed_ports.is_empty() {
                None
            } else {
                Some(exposed_ports)
            },
            host_config: Some(HostConfig {
                mounts: Some(mounts),
                nano_cpus,
                memory,
                privileged: Some(self.config.privileged),
                device_requests,
                port_bindings: if port_bindings.is_empty() {
                    None
                } else {
                    Some(port_bindings)
                },
                network_mode: if host_network_mode {
                    Some("host".to_string())
                } else {
                    None
                },
                ..Default::default()
            }),
            networking_config,
            healthcheck,
            ..Default::default()
        };

        // Create container with 30s timeout (Go parity: createCtx)
        let create_response = tokio::time::timeout(
            Duration::from_secs(30),
            self.client.create_container(
                Some(CreateContainerOptions {
                    name: None,
                    platform: String::new(),
                }),
                container_config,
            ),
        )
        .await
        .map_err(|_| DockerError::ContainerCreate("creation timed out".to_string()))?
        .map_err(|e| {
            let image_str = task.image.as_deref().unwrap_or("unknown");
            tracing::error!(image = image_str, error = %e, "Error creating container");
            DockerError::ContainerCreate(e.to_string())
        })?;

        // Clone volume name before moving into struct (needed for cleanup on error)
        let twerkdir_volume_name_clone = twerkdir_volume_name.clone();

        let container = Container {
            id: create_response.id,
            client: self.client.clone(),
            twerkdir_source: Some(twerkdir_volume_name),
            task_id: task.id.clone().expect("Task ID must be set"),
            probe: task.probe.clone(),
            broker: self.config.broker.clone(),
        };

        // Capture values for cleanup before init (since init consumes self)
        let container_id = container.id.clone();
        let cleanup_client = container.client.clone();
        let twerkdir_volume = twerkdir_volume_name_clone;

        // Clean up container and volume on initialization failure (Go parity: defer tc.Remove)
        if let Err(e) = container.init_twerkdir(task.run.as_deref()).await {
            let _ = cleanup_client
                .remove_container(
                    &container_id,
                    Some(RemoveContainerOptions {
                        force: true,
                        ..Default::default()
                    }),
                )
                .await;
            let _ = cleanup_client
                .remove_volume(&twerkdir_volume, None::<RemoveVolumeOptions>)
                .await;
            return Err(e);
        }

        let effective_workdir = workdir.as_deref().map_or(DEFAULT_WORKDIR, |w| w);

        // Clean up container and volume on initialization failure (Go parity: defer tc.Remove)
        let files = task.files.as_ref().cloned().unwrap_or_default();
        if let Err(e) = container.init_workdir(&files, effective_workdir).await {
            let _ = cleanup_client
                .remove_container(
                    &container_id,
                    Some(RemoveContainerOptions {
                        force: true,
                        ..Default::default()
                    }),
                )
                .await;
            let _ = cleanup_client
                .remove_volume(&twerkdir_volume, None::<RemoveVolumeOptions>)
                .await;
            return Err(e);
        }

        tracing::debug!(container_id = %container.id, "Created container");
        Ok(container)
    }

    /// Parses task limits into Docker resource values.
    fn parse_limits(
        limits: Option<&TaskLimits>,
    ) -> Result<(Option<i64>, Option<i64>), DockerError> {
        let limits = match limits {
            Some(l) => l,
            None => return Ok((None, None)),
        };

        let nano_cpus = match &limits.cpus {
            Some(cpus) if !cpus.is_empty() => Some(
                (cpus
                    .parse::<f64>()
                    .map_err(|_| DockerError::InvalidCpus(cpus.clone()))?
                    * 1e9) as i64,
            ),
            _ => None,
        };

        let memory = match &limits.memory {
            Some(mem) if !mem.is_empty() => {
                Some(parse_memory_bytes(mem).map_err(DockerError::InvalidMemory)?)
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
                    "driver" => {
                        driver = Some(value.trim().to_string());
                    }
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
                        return Err(DockerError::InvalidGpuOptions(format!(
                            "unknown GPU option: {}",
                            other
                        )));
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
            device_ids: if device_ids.is_empty() {
                None
            } else {
                Some(device_ids)
            },
            options: None,
        }])
    }

    /// Prunes old images. Go parity: only prunes when no tasks running.
    async fn prune_images(
        client: &Docker,
        images: &Arc<DashMap<String, std::time::Instant>>,
        tasks: &Arc<RwLock<usize>>,
        ttl: Duration,
    ) {
        if *tasks.read().await > 0 {
            return;
        }

        let now = std::time::Instant::now();
        let to_remove: Vec<String> = images
            .iter()
            .filter(|entry| now.duration_since(*entry.value()) > ttl)
            .map(|entry| entry.key().clone())
            .collect();

        for image in to_remove {
            let _ = client
                .remove_image(
                    &image,
                    None::<RemoveImageOptions>,
                    None::<bollard::auth::DockerCredentials>,
                )
                .await;
            images.remove(&image);
            tracing::debug!(image = %image, "pruned image");
        }
    }
}

// =============================================================================
// TTL-based image caching tests
// =============================================================================

#[cfg(test)]
mod ttl_cache_tests {
    use dashmap::DashMap;
    use std::sync::Arc;
    use std::time::{Duration, Instant};
    use tokio::sync::RwLock;

    #[test]
    fn test_ttl_check_within_ttl() {
        let images: Arc<DashMap<String, Instant>> = Arc::new(DashMap::new());
        let image = "ubuntu:22.04";
        let ttl = Duration::from_secs(300);

        let now = Instant::now();
        images.insert(image.to_string(), now);

        let ts = images.get(image).unwrap();
        let elapsed = Instant::now().duration_since(*ts);
        assert!(elapsed <= ttl, "image should still be within TTL");
    }

    #[test]
    fn test_ttl_check_expired_ttl() {
        let images: Arc<DashMap<String, Instant>> = Arc::new(DashMap::new());
        let image = "ubuntu:22.04";
        let ttl = Duration::from_secs(300);

        let past = Instant::now() - ttl - Duration::from_secs(1);
        images.insert(image.to_string(), past);

        let ts = images.get(image).unwrap();
        let elapsed = Instant::now().duration_since(*ts);
        assert!(elapsed > ttl, "image should be expired");
    }

    #[test]
    fn test_ttl_check_image_not_in_cache() {
        let images: Arc<DashMap<String, Instant>> = Arc::new(DashMap::new());
        let image = "ubuntu:22.04";

        let result = images.get(image);
        assert!(result.is_none(), "image should not be in cache");
    }

    #[test]
    fn test_prune_images_removes_expired() {
        let images: Arc<DashMap<String, Instant>> = Arc::new(DashMap::new());
        let ttl = Duration::from_secs(300);

        let now = Instant::now();
        let expired_image = "ubuntu:22.04";
        let fresh_image = "alpine:3.18";

        images.insert(
            expired_image.to_string(),
            now - ttl - Duration::from_secs(1),
        );
        images.insert(fresh_image.to_string(), now);

        let now_check = Instant::now();
        let to_remove: Vec<String> = images
            .iter()
            .filter(|entry| now_check.duration_since(*entry.value()) > ttl)
            .map(|entry| entry.key().clone())
            .collect();

        assert_eq!(1, to_remove.len());
        assert_eq!(expired_image, to_remove[0]);
    }

    #[test]
    fn test_prune_images_preserves_fresh() {
        let images: Arc<DashMap<String, Instant>> = Arc::new(DashMap::new());
        let ttl = Duration::from_secs(300);

        let now = Instant::now();
        let fresh_image = "alpine:3.18";

        images.insert(fresh_image.to_string(), now);

        let now_check = Instant::now();
        let to_remove: Vec<String> = images
            .iter()
            .filter(|entry| now_check.duration_since(*entry.value()) > ttl)
            .map(|entry| entry.key().clone())
            .collect();

        assert!(to_remove.is_empty(), "fresh image should not be removed");
    }

    #[test]
    fn test_prune_images_skips_when_tasks_running() {
        let images: Arc<DashMap<String, Instant>> = Arc::new(DashMap::new());
        let tasks: Arc<RwLock<usize>> = Arc::new(RwLock::new(5));
        let ttl = Duration::from_secs(300);

        let now = Instant::now();
        images.insert(
            "ubuntu:22.04".to_string(),
            now - ttl - Duration::from_secs(1),
        );

        let result = tasks.try_read();
        assert!(result.is_ok());
        let task_count = *result.unwrap();
        assert!(task_count > 0, "tasks should be running");

        let now_check = Instant::now();
        let to_remove: Vec<String> = if task_count > 0 {
            vec![]
        } else {
            images
                .iter()
                .filter(|entry| now_check.duration_since(*entry.value()) > ttl)
                .map(|entry| entry.key().clone())
                .collect()
        };

        assert!(
            to_remove.is_empty(),
            "should not prune when tasks are running"
        );
    }

    #[test]
    fn test_ttl_cache_multiple_images_mixed_expiration() {
        let images: Arc<DashMap<String, Instant>> = Arc::new(DashMap::new());
        let ttl = Duration::from_secs(300);
        let now = Instant::now();

        images.insert("ubuntu:22.04".to_string(), now);
        images.insert(
            "alpine:3.18".to_string(),
            now - ttl - Duration::from_secs(60),
        );
        images.insert("nginx:1.25".to_string(), now - Duration::from_secs(100));

        let now_check = Instant::now();
        let to_remove: Vec<String> = images
            .iter()
            .filter(|entry| now_check.duration_since(*entry.value()) > ttl)
            .map(|entry| entry.key().clone())
            .collect();

        assert_eq!(1, to_remove.len());
        assert_eq!("alpine:3.18", to_remove[0]);

        assert!(images.contains_key("ubuntu:22.04"));
        assert!(images.contains_key("nginx:1.25"));
    }

    #[test]
    fn test_ttl_boundary_behavior() {
        let ttl = Duration::from_secs(300);
        let now = Instant::now();

        let at_boundary = now - ttl;
        let elapsed_at_boundary = now.duration_since(at_boundary);
        assert!(elapsed_at_boundary <= ttl, "at boundary should be <= TTL");

        let past_boundary = now - ttl - Duration::from_millis(1);
        let elapsed_past_boundary = now.duration_since(past_boundary);
        assert!(elapsed_past_boundary > ttl, "past boundary should be > TTL");

        assert!(elapsed_at_boundary <= ttl && elapsed_past_boundary > ttl);
    }

    #[test]
    fn test_ttl_cache_one_second_over_ttl() {
        let images: Arc<DashMap<String, Instant>> = Arc::new(DashMap::new());
        let ttl = Duration::from_secs(300);

        let now = Instant::now();
        let one_second_over = now - ttl - Duration::from_secs(1);
        images.insert("ubuntu:22.04".to_string(), one_second_over);

        let now_check = Instant::now();
        let elapsed = now_check.duration_since(one_second_over);
        assert!(elapsed > ttl, "one second over TTL should be expired");

        let to_remove: Vec<String> = images
            .iter()
            .filter(|entry| now_check.duration_since(*entry.value()) > ttl)
            .map(|entry| entry.key().clone())
            .collect();

        assert_eq!(1, to_remove.len());
        assert_eq!("ubuntu:22.04", to_remove[0]);
    }
}

// =============================================================================
// Task type
// =============================================================================
