//! Container creation factory and resource parsing.
//!
//! Provides the `create_task_container` factory function and helpers
//! for parsing CPU, memory, and GPU resource specifications.

use crate::broker::Broker;
use crate::runtime::docker::archive::Archive;
use crate::runtime::docker::config::{
    CREATED_CONTAINER, CREATION_TIMED_OUT, UNKNOWN_MOUNT_TYPE_NONE,
};
use crate::runtime::docker::error::DockerError;
use crate::runtime::docker::helpers::{parse_gpu_options, parse_memory_bytes, slugify};
use crate::runtime::docker::mounters::Mounter;
use crate::runtime::DEFAULT_TIMEOUT;
use bollard::config::{HostConfig, NetworkingConfig};
use bollard::models::{
    ContainerCreateBody, EndpointSettings, Mount as BollardMount, MountTypeEnum, PortBinding,
};
use bollard::query_parameters::{
    CreateContainerOptions, RemoveContainerOptions, RemoveVolumeOptions, UploadToContainerOptions,
};
use bollard::{body_full, Docker};
use std::collections::HashMap;
use std::io::Read;
use std::sync::Arc;
use twerk_common::constants::DEFAULT_TASK_NAME;
use twerk_core::env::format_kv;
use twerk_core::mount::mount_type;
use twerk_core::task::Task;
use twerk_core::uuid::new_uuid;

use super::tcontainer::Tcontainer;

const TWERK_OUTPUT: &str = "TWERK_OUTPUT=/twerk/stdout";
const TWERK_PROGRESS: &str = "TWERK_PROGRESS=/twerk/progress";

/// Container resource limits (CPU, memory, GPU device requests).
type ContainerResources = (
    Option<i64>,
    Option<i64>,
    Option<Vec<bollard::models::DeviceRequest>>,
);

/// Parameters for building container configuration.
struct ContainerConfigParams {
    task: Task,
    env: Vec<String>,
    mounts: Vec<BollardMount>,
    networking_config: Option<NetworkingConfig>,
    port_bindings: Option<HashMap<String, Option<Vec<PortBinding>>>>,
    nano_cpus: Option<i64>,
    memory: Option<i64>,
    device_requests: Option<Vec<bollard::models::DeviceRequest>>,
}

/// Context for container instantiation.
struct ContainerContext {
    container_id: String,
    client: Docker,
    mounter: Arc<dyn Mounter>,
    broker: Arc<dyn Broker>,
    task: Task,
    logger: Box<dyn std::io::Write + Send + Sync>,
    torkdir: twerk_core::mount::Mount,
    torkdir_volume_name: String,
    task_id: twerk_core::id::TaskId,
}

/// Parses CPU limits from task configuration.
fn parse_cpus(limits: Option<&twerk_core::task::TaskLimits>) -> Result<Option<i64>, DockerError> {
    let cpus = match limits.and_then(|l| l.cpus.as_ref()) {
        Some(cpus) if !cpus.is_empty() => {
            let value: f64 = cpus
                .parse()
                .map_err(|_| DockerError::InvalidCpus(cpus.clone()))?;
            #[allow(clippy::cast_possible_truncation)]
            Some((value * 1e9) as i64)
        }
        _ => None,
    };
    Ok(cpus)
}

/// Parses memory limits from task configuration.
fn parse_memory(limits: Option<&twerk_core::task::TaskLimits>) -> Result<Option<i64>, DockerError> {
    let memory = match limits.and_then(|l| l.memory.as_ref()) {
        Some(mem) if !mem.is_empty() => {
            Some(parse_memory_bytes(mem).map_err(DockerError::InvalidMemory)?)
        }
        _ => None,
    };
    Ok(memory)
}

/// Builds mount configuration from task mounts.
fn build_mounts(task: &Task) -> Result<Vec<BollardMount>, DockerError> {
    let mut mounts: Vec<BollardMount> = Vec::new();

    if let Some(ref task_mounts) = task.mounts {
        for mnt in task_mounts {
            let mount_type_str = mnt.mount_type.as_deref();
            let mt = match mount_type_str {
                Some(mount_type::VOLUME) => {
                    if mnt.target.as_ref().is_none_or(String::is_empty) {
                        return Err(DockerError::VolumeTargetRequired);
                    }
                    MountTypeEnum::VOLUME
                }
                Some(mount_type::BIND) => {
                    if mnt.target.as_ref().is_none_or(String::is_empty) {
                        return Err(DockerError::BindTargetRequired);
                    }
                    if mnt.source.as_ref().is_none_or(String::is_empty) {
                        return Err(DockerError::BindSourceRequired);
                    }
                    MountTypeEnum::BIND
                }
                Some(mount_type::TMPFS) => MountTypeEnum::TMPFS,
                Some(other) => return Err(DockerError::UnknownMountType(other.to_string())),
                None => {
                    return Err(DockerError::UnknownMountType(
                        UNKNOWN_MOUNT_TYPE_NONE.to_string(),
                    ))
                }
            };

            tracing::debug!(source = ?mnt.source, target = ?mnt.target, "Mounting");
            mounts.push(BollardMount {
                target: mnt.target.clone(),
                source: mnt.source.clone(),
                typ: Some(mt),
                ..Default::default()
            });
        }
    }

    Ok(mounts)
}

/// Builds environment variables from task configuration.
fn build_env(task: &Task) -> Vec<String> {
    let mut env: Vec<String> = if let Some(ref env_map) = task.env {
        env_map.iter().map(|(k, v)| format_kv(k, v)).collect()
    } else {
        Vec::new()
    };
    env.push(TWERK_OUTPUT.to_string());
    env.push(TWERK_PROGRESS.to_string());
    env
}

/// Builds port bindings for probe configuration.
fn build_port_bindings(task: &Task) -> Option<HashMap<String, Option<Vec<PortBinding>>>> {
    task.probe.as_ref().map(|probe| {
        let port_key = format!("{}/tcp", probe.port);
        [(
            port_key,
            Some(vec![PortBinding {
                host_ip: Some("127.0.0.1".to_string()),
                host_port: Some("0".to_string()),
            }]),
        )]
        .into_iter()
        .collect()
    })
}

/// Builds networking configuration from task networks.
fn build_networking_config(task: &Task) -> Option<NetworkingConfig> {
    if task.networks.as_ref().is_none_or(Vec::is_empty) {
        return None;
    }

    let mut endpoints = HashMap::new();
    if let Some(ref networks) = task.networks {
        let alias = slugify(task.name.as_deref().unwrap_or(DEFAULT_TASK_NAME));
        for nw in networks {
            endpoints.insert(
                nw.clone(),
                EndpointSettings {
                    aliases: Some(vec![alias.clone()]),
                    ..Default::default()
                },
            );
        }
    }

    Some(NetworkingConfig {
        endpoints_config: Some(endpoints),
    })
}

/// Pulls the task image from the registry.
async fn pull_task_image(client: &Docker, task: &Task) -> Result<(), DockerError> {
    let image = task.image.as_ref().ok_or(DockerError::ImageRequired)?;
    crate::runtime::docker::pull::pull_image::<std::collections::hash_map::RandomState>(
        client,
        &crate::runtime::docker::config::DockerConfig::default(),
        &Arc::new(tokio::sync::RwLock::new(std::collections::HashMap::new())),
        image,
        task.registry.as_ref(),
    )
    .await
    .map_err(|e| DockerError::ImagePull(format!("{image}: {e}")))
}

/// Creates the torkdir volume and returns (`volume_name`, `torkdir_mount`).
async fn create_torkdir_volume(
    client: &Docker,
) -> Result<(String, twerk_core::mount::Mount), DockerError> {
    let torkdir_id = new_uuid();
    let torkdir_volume_name = torkdir_id.clone();

    client
        .create_volume(bollard::models::VolumeCreateRequest {
            name: Some(torkdir_volume_name.clone()),
            driver: Some("local".to_string()),
            ..Default::default()
        })
        .await
        .map_err(|e| DockerError::VolumeCreate(e.to_string()))?;

    let torkdir = twerk_core::mount::Mount {
        id: Some(torkdir_id.clone()),
        mount_type: Some(mount_type::VOLUME.to_string()),
        target: Some("/twerk".to_string()),
        source: Some(torkdir_volume_name.clone()),
        opts: None,
    };

    Ok((torkdir_volume_name, torkdir))
}

/// Parses container resource limits (CPU, memory, GPU).
fn parse_container_resources(task: &Task) -> Result<ContainerResources, DockerError> {
    let nano_cpus = parse_cpus(task.limits.as_ref())?;
    let memory = parse_memory(task.limits.as_ref())?;
    let device_requests = task
        .gpus
        .as_ref()
        .map(|gpu_str| parse_gpu_options(gpu_str))
        .transpose()?;
    Ok((nano_cpus, memory, device_requests))
}

/// Builds the command and entrypoint for the container.
fn build_command_and_entrypoint(task: &Task) -> (Vec<String>, Vec<String>) {
    let cmd: Vec<String> = if task.cmd.as_ref().is_none_or(Vec::is_empty) {
        vec!["/twerk/entrypoint".to_string()]
    } else {
        task.cmd.clone().unwrap_or_default()
    };

    let entrypoint: Vec<String> =
        if task.entrypoint.as_ref().is_none_or(Vec::is_empty) && task.run.is_some() {
            vec!["sh".to_string(), "-c".to_string()]
        } else {
            task.entrypoint.clone().unwrap_or_default()
        };

    (cmd, entrypoint)
}

/// Builds container configuration parameters.
fn build_container_config_params(
    task: &Task,
    env: Vec<String>,
    mounts: Vec<BollardMount>,
    nano_cpus: Option<i64>,
    memory: Option<i64>,
    device_requests: Option<Vec<bollard::models::DeviceRequest>>,
) -> ContainerConfigParams {
    let port_bindings = build_port_bindings(task);
    let networking_config = build_networking_config(task);

    ContainerConfigParams {
        task: task.clone(),
        env,
        mounts,
        networking_config,
        port_bindings,
        nano_cpus,
        memory,
        device_requests,
    }
}

/// Builds the host configuration for the container.
fn build_host_config(
    mounts: Vec<BollardMount>,
    nano_cpus: Option<i64>,
    memory: Option<i64>,
    device_requests: Option<Vec<bollard::models::DeviceRequest>>,
    port_bindings: Option<HashMap<String, Option<Vec<PortBinding>>>>,
) -> HostConfig {
    HostConfig {
        mounts: Some(mounts),
        nano_cpus,
        memory,
        privileged: Some(false),
        device_requests,
        port_bindings,
        ..Default::default()
    }
}

/// Builds the exposed ports from probe configuration.
fn build_exposed_ports(task: &Task) -> Option<Vec<String>> {
    task.probe
        .as_ref()
        .map(|probe| vec![format!("{}/tcp", probe.port)])
}

/// Builds the container configuration body.
fn build_container_config(params: ContainerConfigParams) -> ContainerCreateBody {
    let (cmd, entrypoint) = build_command_and_entrypoint(&params.task);
    let host_config = build_host_config(
        params.mounts,
        params.nano_cpus,
        params.memory,
        params.device_requests,
        params.port_bindings,
    );

    ContainerCreateBody {
        image: params.task.image.clone(),
        env: Some(params.env),
        cmd: Some(cmd),
        entrypoint: if entrypoint.is_empty() {
            None
        } else {
            Some(entrypoint)
        },
        exposed_ports: build_exposed_ports(&params.task),
        host_config: Some(host_config),
        networking_config: params.networking_config,
        ..Default::default()
    }
}

/// Creates the Docker container and returns the container ID.
async fn create_container(
    client: &Docker,
    container_config: ContainerCreateBody,
    image: &str,
) -> Result<String, DockerError> {
    let create_ctx = tokio::time::timeout(
        DEFAULT_TIMEOUT,
        client.create_container(
            Some(CreateContainerOptions {
                name: None,
                platform: String::new(),
            }),
            container_config,
        ),
    )
    .await
    .map_err(|_| DockerError::ContainerCreate(CREATION_TIMED_OUT.to_string()))?
    .map_err(|e| {
        tracing::error!(image = %image, error = %e, "Error creating container");
        DockerError::ContainerCreate(e.to_string())
    })?;

    Ok(create_ctx.id)
}

/// Creates the Tcontainer instance.
fn instantiate_container(ctx: ContainerContext) -> Tcontainer {
    let probe = ctx.task.probe.clone();
    Tcontainer::new(
        ctx.container_id,
        ctx.client,
        ctx.mounter,
        Some(ctx.broker),
        ctx.task,
        ctx.logger,
        ctx.torkdir,
        Some(ctx.torkdir_volume_name),
        ctx.task_id,
        probe,
    )
}

/// Adds torkdir volume mount to the mounts list.
fn add_torkdir_to_mounts(
    mounts: Vec<BollardMount>,
    torkdir_volume_name: &str,
) -> Vec<BollardMount> {
    let mut mounts = mounts;
    mounts.push(BollardMount {
        typ: Some(MountTypeEnum::VOLUME),
        source: Some(torkdir_volume_name.to_string()),
        target: Some("/twerk".to_string()),
        ..Default::default()
    });
    mounts
}

/// Initializes the twerk directory in the container.
async fn initialize_twerkdir(
    client: &Docker,
    tc: &Tcontainer,
    container_id: &str,
    torkdir_volume_name: &str,
    task: &Task,
) -> Result<(), DockerError> {
    if let Err(e) = tc.init_twerkdir(task.run.as_deref()).await {
        cleanup_container(client, container_id, torkdir_volume_name).await;
        Err(DockerError::CopyToContainer(format!(
            "error initializing torkdir: {e}"
        )))
    } else {
        Ok(())
    }
}

/// Determines the effective workdir based on task configuration.
fn determine_effective_workdir(task: &Task) -> Option<String> {
    let workdir_has_files = !task
        .files
        .as_ref()
        .is_none_or(std::collections::HashMap::is_empty);

    if task.workdir.is_some() {
        task.workdir.clone()
    } else if workdir_has_files {
        Some("/workspace".to_string())
    } else {
        None
    }
}

/// Initializes the work directory in the container if needed.
async fn initialize_workdir(
    client: &Docker,
    tc: &Tcontainer,
    container_id: &str,
    torkdir_volume_name: &str,
    task: &Task,
) -> Result<(), DockerError> {
    let effective_workdir = determine_effective_workdir(task);

    if let Some(ref workdir) = effective_workdir {
        if let Err(e) = init_workdir_for_container(tc, workdir).await {
            cleanup_container(client, container_id, torkdir_volume_name).await;
            return Err(DockerError::CopyToContainer(format!(
                "error initializing workdir: {e}"
            )));
        }
    }

    Ok(())
}

/// Creates a task container for the given task.
///
/// # Errors
///
/// Returns `DockerError` if the container cannot be created.
///
/// # Panics
///
/// Panics if the task ID is not set (but this is checked first and returns an error).
///
/// Go parity: `createTaskContainer` in tcontainer.go
pub async fn create_task_container(
    client: &Docker,
    mounter: Arc<dyn Mounter>,
    broker: Arc<dyn Broker>,
    task: &Task,
    logger: Box<dyn std::io::Write + Send + Sync>,
) -> Result<Tcontainer, DockerError> {
    if task.id.as_ref().is_none_or(|id| id.is_empty()) {
        return Err(DockerError::TaskIdRequired);
    }

    pull_task_image(client, task).await?;

    let env = build_env(task);
    let mounts = build_mounts(task)?;
    let (nano_cpus, memory, device_requests) = parse_container_resources(task)?;

    let (torkdir_volume_name, torkdir) = create_torkdir_volume(client).await?;
    let mounts = add_torkdir_to_mounts(mounts, &torkdir_volume_name);

    let container_params =
        build_container_config_params(task, env, mounts, nano_cpus, memory, device_requests);
    let container_config = build_container_config(container_params);

    let image = task.image.as_deref().unwrap_or_default();
    let container_id = create_container(client, container_config, image).await?;

    let task_id = task.id.as_ref().ok_or_else(|| {
        DockerError::ContainerCreate("task ID is required but was empty".to_string())
    })?;

    let tc = instantiate_container(ContainerContext {
        container_id: container_id.clone(),
        client: client.clone(),
        mounter: mounter.clone(),
        broker,
        task: task.clone(),
        logger,
        torkdir,
        torkdir_volume_name: torkdir_volume_name.clone(),
        task_id: task_id.clone(),
    });

    initialize_twerkdir(client, &tc, &container_id, &torkdir_volume_name, task).await?;
    initialize_workdir(client, &tc, &container_id, &torkdir_volume_name, task).await?;

    tracing::debug!(container_id = %container_id, CREATED_CONTAINER);
    Ok(tc)
}

/// Cleans up a container and its volume on error.
async fn cleanup_container(client: &Docker, container_id: &str, volume_name: &str) {
    let _ = client
        .remove_container(
            container_id,
            Some(RemoveContainerOptions {
                force: true,
                ..Default::default()
            }),
        )
        .await;

    for attempt in 0..3 {
        if attempt > 0 {
            tokio::time::sleep(std::time::Duration::from_millis(100 * 2u64.pow(attempt - 1)))
                .await;
        }
        match client
            .remove_volume(volume_name, Some(RemoveVolumeOptions { force: true }))
            .await
        {
            Ok(()) => break,
            Err(e) if attempt < 2 => {
                tracing::debug!(error = %e, volume = %volume_name, attempt, "retrying volume removal");
            }
            Err(e) => {
                tracing::warn!(error = %e, volume = %volume_name, "failed to remove volume after 3 attempts");
            }
        }
    }
}

/// Initializes the work directory for a container.
async fn init_workdir_for_container(tc: &Tcontainer, workdir: &str) -> Result<(), DockerError> {
    let Some(files) = &tc.task.files else {
        return Ok(());
    };
    if files.is_empty() {
        return Ok(());
    }

    let mut archive = Archive::new().map_err(|e| DockerError::copy_to_container(&e))?;

    for (name, data) in files {
        archive
            .write_file(name, 0o444, data.as_bytes())
            .map_err(|e| DockerError::copy_to_container(&e))?;
    }

    archive
        .finish()
        .map_err(|e| DockerError::copy_to_container(&e))?;

    let mut reader = archive
        .reader()
        .map_err(|e| DockerError::copy_to_container(&e))?;

    let mut contents = Vec::new();
    Read::read_to_end(&mut reader, &mut contents)
        .map_err(|e| DockerError::copy_to_container(&e))?;

    let options = UploadToContainerOptions {
        path: workdir.to_string(),
        ..Default::default()
    };

    tc.client
        .upload_to_container(&tc.id, Some(options), body_full(contents.into()))
        .await
        .map_err(|e| DockerError::copy_to_container(&e))?;

    archive
        .remove()
        .map_err(|e| DockerError::copy_to_container(&e))?;

    Ok(())
}
