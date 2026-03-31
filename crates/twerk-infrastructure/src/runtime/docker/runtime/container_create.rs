//! Container creation logic for Docker runtime.

//!
//! This module handles the full lifecycle of container creation including
//! image pull, env, mounts, limits, GPU, probe ports, networking aliases,
//! workdir, and file initialization.

//!
//! Go parity: `createTaskContainer`

use std::sync::Arc;
use std::time::Duration;

use bollard::models::HostConfig;
use bollard::query_parameters::{
    CreateContainerOptions, RemoveContainerOptions, RemoveVolumeOptions,
};
use bollard::Docker;

use super::super::config::DockerConfig;
use super::super::container::Container;
use super::super::error::DockerError;
use super::super::mounters::Mounter;
use super::super::task::Task;
use super::container_config::{
    ContainerCmd, ContainerEnv, ContainerMounts, ContainerNetworking, ContainerProbe,
};
use super::image::pull_image;
use super::types::PullRequest;
use twerk_core::uuid::new_uuid;

/// Creates a container for a task.
///
/// Go parity: `createTaskContainer` — full lifecycle setup including
/// image pull, env, mounts, limits, GPU, probe ports, networking aliases,
/// workdir, and file initialization.
#[allow(dead_code)]
pub(super) async fn create_container(
    client: &Docker,
    config: &DockerConfig,
    pull_tx: &tokio::sync::mpsc::Sender<super::types::PullRequest>,
    mounter: &Arc<dyn Mounter>,
    task: &Task,
) -> Result<Container, DockerError> {
    if task.id.as_ref().is_none_or(|id| id.is_empty()) {
        return Err(DockerError::TaskIdRequired);
    }

    // Pull image
    let image = task
        .image
        .as_ref()
        .ok_or_else(|| DockerError::ImageRequired)?;
    pull_image(pull_tx, image, task.registry.as_ref()).await?;

    // Build configuration components
    let env = ContainerEnv::build(task);
    let cmd = ContainerCmd::build(task);
    let probe = ContainerProbe::build(task.probe.as_ref());
    let networking = ContainerNetworking::build(task, config.privileged)?;

    // Create twerkdir volume
    let twerkdir_volume_name = new_uuid();
    let _ = client
        .create_volume(bollard::models::VolumeCreateRequest {
            name: Some(twerkdir_volume_name.clone()),
            driver: Some("local".to_string()),
            ..Default::default()
        })
        .await
        .map_err(|e| DockerError::VolumeCreate(e.to_string()))?;

    // Build mounts
    let mounts = ContainerMounts::build(task, &twerkdir_volume_name)?;

    // Parse limits
    let (nano_cpus, memory) = super::container_config::parse_limits(task.limits.as_ref())?;

    // GPU device requests (Go parity: `gpuOpts.Set(t.GPUs)`)
    let device_requests = task
        .gpus
        .as_ref()
        .map(|gpu_str| super::container_config::parse_gpu_options(gpu_str))
        .transpose()?;

    // Build container config
    let container_config = bollard::models::ContainerCreateBody {
        image: task.image.clone(),
        env: Some(env.env),
        cmd: Some(cmd.cmd),
        entrypoint: if cmd.entrypoint.is_empty() {
            None
        } else {
            Some(cmd.entrypoint)
        },
        working_dir: cmd.workdir.clone(),
        exposed_ports: if probe.exposed_ports.is_empty() {
            None
        } else {
            Some(probe.exposed_ports)
        },
        host_config: Some(HostConfig {
            mounts: Some(mounts.mounts),
            nano_cpus,
            memory,
            privileged: Some(config.privileged),
            device_requests,
            port_bindings: if probe.port_bindings.is_empty() {
                None
            } else {
                Some(probe.port_bindings)
            },
            network_mode: if networking.host_network_mode {
                Some("host".to_string())
            } else {
                None
            },
            ..Default::default()
        }),
        networking_config: networking.networking_config,
        healthcheck: probe.healthcheck,
        ..Default::default()
    };

    // Create container with 30s timeout (Go parity: createCtx)
    let create_response = tokio::time::timeout(
        Duration::from_secs(30),
        client.create_container(
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
        id: create_response.id.clone(),
        client: client.clone(),
        twerkdir_source: Some(twerkdir_volume_name),
        task_id: task.id.clone().expect("Task ID must be set"),
        probe: task.probe.clone(),
        broker: config.broker.clone(),
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

    let effective_workdir = cmd.workdir.as_deref().map_or(super::types::DEFAULT_WORKDIR, |w| w);

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

    tracing::debug!(container_id = %container_id, "Created container");
    Ok(container)
}
