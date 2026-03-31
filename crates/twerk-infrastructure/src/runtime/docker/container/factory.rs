//! Container creation factory and resource parsing.
//!
//! Provides the `create_task_container` factory function and helpers
//! for parsing CPU, memory, and GPU resource specifications.

use super::archive::init_runtime_dir;
use super::probe::probe_if_configured;
use crate::broker::Broker;
use crate::runtime::docker::archive::Archive;
use crate::runtime::docker::error::DockerError;
use crate::runtime::docker::helpers::{parse_memory_bytes, slugify};
use crate::runtime::docker::mounters::Mounter;
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
use twerk_core::mount::mount_type;
use twerk_core::task::Task;
use twerk_core::uuid::new_uuid;

use super::tcontainer::Tcontainer;

const TWORK_OUTPUT: &str = "TWERK_OUTPUT=/twerk/stdout";
const TWORK_PROGRESS: &str = "TWERK_PROGRESS=/twerk/progress";

/// Parses CPU limits from task configuration.
fn parse_cpus(limits: Option<&twerk_core::task::TaskLimits>) -> Result<Option<i64>, DockerError> {
    let cpus = match limits.and_then(|l| l.cpus.as_ref()) {
        Some(cpus) if !cpus.is_empty() => {
            let value: f64 = cpus
                .parse()
                .map_err(|_| DockerError::InvalidCpus(cpus.clone()))?;
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

/// Parses GPU options string into DeviceRequest configuration.
fn parse_gpu_options(gpu_str: &str) -> Result<Vec<bollard::models::DeviceRequest>, DockerError> {
    use bollard::models::DeviceRequest;

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

/// Builds mount configuration from task mounts.
fn build_mounts(task: &Task) -> Result<Vec<BollardMount>, DockerError> {
    let mut mounts: Vec<BollardMount> = Vec::new();

    if let Some(ref task_mounts) = task.mounts {
        for mnt in task_mounts {
            let mount_type_str = mnt.mount_type.as_deref();
            let mt = match mount_type_str {
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
        env_map
            .iter()
            .map(|(k, v)| format!("{}={}", k, v))
            .collect()
    } else {
        Vec::new()
    };
    env.push(TWORK_OUTPUT.to_string());
    env.push(TWORK_PROGRESS.to_string());
    env
}

/// Builds port bindings for probe configuration.
fn build_port_bindings(task: &Task) -> Option<HashMap<String, Option<Vec<PortBinding>>>> {
    task.probe.as_ref().map(|probe| {
        let port_key = format!("{}/tcp", probe.port);
        [(
            port_key.clone(),
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
    if task.networks.as_ref().is_none_or(|n| n.is_empty()) {
        return None;
    }

    let mut endpoints = HashMap::new();
    if let Some(ref networks) = task.networks {
        let alias = slugify(task.name.as_deref().unwrap_or("unknown"));
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

/// Creates a task container for the given task.
/// Go parity: createTaskContainer in tcontainer.go
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

    let image = task
        .image
        .as_ref()
        .ok_or_else(|| DockerError::ImageRequired)?;

    crate::runtime::docker::pull::pull_image(
        client,
        &crate::runtime::docker::config::DockerConfig::default(),
        &Default::default(),
        image,
        task.registry.as_ref(),
    )
    .await
    .map_err(|e| DockerError::ImagePull(format!("{}: {}", image, e)))?;

    let env = build_env(task);
    let mut mounts = build_mounts(task)?;

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

    mounts.push(BollardMount {
        typ: Some(MountTypeEnum::VOLUME),
        source: Some(torkdir_volume_name.clone()),
        target: Some("/twerk".to_string()),
        ..Default::default()
    });

    let nano_cpus = parse_cpus(task.limits.as_ref())?;
    let memory = parse_memory(task.limits.as_ref())?;

    let device_requests = task
        .gpus
        .as_ref()
        .map(|gpu_str| parse_gpu_options(gpu_str))
        .transpose()?;

    let cmd: Vec<String> = if task.cmd.as_ref().is_none_or(|c| c.is_empty()) {
        vec!["/twerk/entrypoint".to_string()]
    } else {
        task.cmd.clone().unwrap_or_default()
    };

    let entrypoint: Vec<String> =
        if task.entrypoint.as_ref().is_none_or(|e| e.is_empty()) && task.run.is_some() {
            vec!["sh".to_string(), "-c".to_string()]
        } else {
            task.entrypoint.clone().unwrap_or_default()
        };

    let port_bindings = build_port_bindings(task);

    let host_config = HostConfig {
        mounts: Some(mounts),
        nano_cpus,
        memory,
        privileged: Some(false),
        device_requests,
        port_bindings,
        ..Default::default()
    };

    let networking_config = build_networking_config(task);

    let container_config = ContainerCreateBody {
        image: task.image.clone(),
        env: Some(env),
        cmd: Some(cmd),
        entrypoint: if entrypoint.is_empty() {
            None
        } else {
            Some(entrypoint)
        },
        exposed_ports: task
            .probe
            .as_ref()
            .map(|probe| vec![format!("{}/tcp", probe.port)].into_iter().collect()),
        host_config: Some(host_config),
        networking_config,
        ..Default::default()
    };

    let create_ctx = tokio::time::timeout(
        std::time::Duration::from_secs(30),
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
        tracing::error!(image = %image, error = %e, "Error creating container");
        DockerError::ContainerCreate(e.to_string())
    })?;

    let container_id = create_ctx.id;

    let tc = Tcontainer::new(
        container_id.clone(),
        client.clone(),
        mounter.clone(),
        broker,
        task.clone(),
        logger,
        torkdir.clone(),
    );

    if let Err(e) = tc.init_torkdir().await {
        cleanup_container(client, &container_id, &torkdir_volume_name).await;
        return Err(DockerError::CopyToContainer(format!(
            "error initializing torkdir: {}",
            e
        )));
    }

    let workdir_has_files = !task.files.as_ref().is_none_or(|f| f.is_empty());
    let effective_workdir: Option<String> = if task.workdir.is_some() {
        task.workdir.clone()
    } else if workdir_has_files {
        Some("/workspace".to_string())
    } else {
        None
    };

    if let Some(ref workdir) = effective_workdir {
        if let Err(e) = init_workdir_for_container(&tc, workdir).await {
            cleanup_container(client, &container_id, &torkdir_volume_name).await;
            return Err(DockerError::CopyToContainer(format!(
                "error initializing workdir: {}",
                e
            )));
        }
    }

    tracing::debug!(container_id = %container_id, "Created container");

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
    let _ = client
        .remove_volume(volume_name, Some(RemoveVolumeOptions { force: true }))
        .await;
}

/// Initializes the work directory for a container.
async fn init_workdir_for_container(tc: &Tcontainer, workdir: &str) -> Result<(), DockerError> {
    let files = match &tc.task.files {
        Some(f) => f,
        None => return Ok(()),
    };
    if files.is_empty() {
        return Ok(());
    }

    let mut archive = Archive::new().map_err(|e| DockerError::CopyToContainer(e.to_string()))?;

    for (name, data) in files {
        archive
            .write_file(name, 0o444, data.as_bytes())
            .map_err(|e| DockerError::CopyToContainer(e.to_string()))?;
    }

    archive
        .finish()
        .map_err(|e| DockerError::CopyToContainer(e.to_string()))?;

    let mut reader = archive
        .reader()
        .map_err(|e| DockerError::CopyToContainer(e.to_string()))?;

    let mut contents = Vec::new();
    Read::read_to_end(&mut reader, &mut contents)
        .map_err(|e| DockerError::CopyToContainer(e.to_string()))?;

    let options = UploadToContainerOptions {
        path: workdir.to_string(),
        ..Default::default()
    };

    tc.client
        .upload_to_container(&tc.id, Some(options), body_full(contents.into()))
        .await
        .map_err(|e| DockerError::CopyToContainer(e.to_string()))?;

    archive
        .remove()
        .map_err(|e| DockerError::CopyToContainer(e.to_string()))?;

    Ok(())
}
