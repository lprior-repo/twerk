//! Container configuration building for Docker runtime.

use std::collections::HashMap;

use bollard::config::NetworkingConfig;
use bollard::models::{
    EndpointSettings, HealthConfig, Mount as BollardMount, MountTypeEnum, PortBinding,
};

use super::super::config::UNKNOWN_MOUNT_TYPE_NONE;
use super::super::error::DockerError;
use super::super::helpers::{parse_go_duration, parse_memory_bytes, port_key, slugify};
use super::types::{
    DEFAULT_CMD, DEFAULT_PROBE_PATH, DEFAULT_PROBE_TIMEOUT, DEFAULT_WORKDIR, PROBE_TIMEOUT_SECS,
    RUN_ENTRYPOINT,
};
use twerk_core::env::format_kv;
use twerk_core::mount::mount_type;
use twerk_core::task::{Probe, Task, TaskLimits};

/// Parses task limits into Docker resource values.
pub(super) fn parse_limits(
    limits: Option<&TaskLimits>,
) -> Result<(Option<i64>, Option<i64>), DockerError> {
    let Some(limits) = limits else {
        return Ok((None, None));
    };

    let nano_cpus = match &limits.cpus {
        Some(cpus) if !cpus.is_empty() =>
        {
            #[allow(clippy::cast_possible_truncation)]
            Some(
                (cpus
                    .parse::<f64>()
                    .map_err(|_| DockerError::InvalidCpus(cpus.clone()))?
                    * 1e9) as i64,
            )
        }
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

/// Container environment configuration.
pub(super) struct ContainerEnv {
    pub env: Vec<String>,
}

impl ContainerEnv {
    /// Builds environment variables from task configuration.
    pub(super) fn build(task: &Task) -> Self {
        let mut env: Vec<String> = if let Some(ref env_map) = task.env {
            env_map.iter().map(|(k, v)| format_kv(k, v)).collect()
        } else {
            Vec::new()
        };
        env.push("TWERK_OUTPUT=/twerk/stdout".to_string());
        env.push("TWERK_PROGRESS=/twerk/progress".to_string());
        Self { env }
    }
}

/// Container mounts configuration.
pub(super) struct ContainerMounts {
    pub mounts: Vec<BollardMount>,
}

impl ContainerMounts {
    /// Builds mounts from task configuration with validation.
    pub(super) fn build(task: &Task, twerkdir_volume_name: &str) -> Result<Self, DockerError> {
        let mut mounts: Vec<BollardMount> = Vec::new();

        if let Some(ref mounts_list) = task.mounts {
            for mnt in mounts_list {
                let typ = match mnt.mount_type.as_deref() {
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
                    typ: Some(typ),
                    ..Default::default()
                });
            }
        }

        // Add twerkdir volume mount
        mounts.push(BollardMount {
            target: Some("/twerk".to_string()),
            source: Some(twerkdir_volume_name.to_string()),
            typ: Some(MountTypeEnum::VOLUME),
            ..Default::default()
        });

        Ok(Self { mounts })
    }
}

/// Container probe configuration.
pub(super) struct ContainerProbe {
    pub exposed_ports: Vec<String>,
    pub port_bindings: HashMap<String, Option<Vec<PortBinding>>>,
    pub healthcheck: Option<HealthConfig>,
}

impl ContainerProbe {
    /// Builds probe configuration from task probe settings.
    pub(super) fn build(probe: Option<&Probe>) -> Self {
        let mut exposed_ports: Vec<String> = Vec::new();
        let mut port_bindings: HashMap<String, Option<Vec<PortBinding>>> = HashMap::new();
        let mut healthcheck: Option<HealthConfig> = None;

        if let Some(probe) = probe {
            let port = probe.port;
            let port_key = port_key(port.cast_unsigned());
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
            let timeout = parse_go_duration(timeout_str)
                .map_or(std::time::Duration::from_secs(PROBE_TIMEOUT_SECS), |v| v);
            let interval = crate::runtime::DEFAULT_TIMEOUT;

            healthcheck = Some(HealthConfig {
                test: Some(vec![
                    "CMD".to_string(),
                    "curl".to_string(),
                    "-f".to_string(),
                    "-s".to_string(),
                    format!("http://localhost:{port}{probe_path}"),
                ]),
                #[allow(clippy::cast_possible_truncation)]
                interval: Some(interval.as_nanos() as i64),
                #[allow(clippy::cast_possible_truncation)]
                timeout: Some(timeout.as_nanos() as i64),
                retries: Some(3),
                start_period: Some(0),
                start_interval: Some(0),
            });
        }

        Self {
            exposed_ports,
            port_bindings,
            healthcheck,
        }
    }
}

/// Container command and entrypoint configuration.
pub(super) struct ContainerCmd {
    pub cmd: Vec<String>,
    pub entrypoint: Vec<String>,
    pub workdir: Option<String>,
}

impl ContainerCmd {
    /// Builds command configuration from task settings.
    pub(super) fn build(task: &Task) -> Self {
        // Working directory
        let workdir = if task.workdir.is_some() {
            task.workdir.clone()
        } else if task
            .files
            .as_ref()
            .is_none_or(std::collections::HashMap::is_empty)
        {
            None
        } else {
            Some(DEFAULT_WORKDIR.to_string())
        };

        // Entrypoint auto-detection (Go parity)
        let cmd: Vec<String> = if task.cmd.as_ref().is_none_or(Vec::is_empty) {
            DEFAULT_CMD.iter().map(ToString::to_string).collect()
        } else {
            task.cmd
                .as_ref()
                .map_or_else(Vec::new, std::clone::Clone::clone)
        };

        let entrypoint: Vec<String> =
            if task.entrypoint.as_ref().is_none_or(Vec::is_empty) && task.run.is_some() {
                RUN_ENTRYPOINT.iter().map(ToString::to_string).collect()
            } else {
                task.entrypoint
                    .as_ref()
                    .map_or_else(Vec::new, std::clone::Clone::clone)
            };

        Self {
            cmd,
            entrypoint,
            workdir,
        }
    }
}

/// Container networking configuration.
pub(super) struct ContainerNetworking {
    pub networking_config: Option<NetworkingConfig>,
    pub host_network_mode: bool,
}

impl ContainerNetworking {
    /// Builds networking configuration from task settings.
    pub(super) fn build(task: &Task, host_network_allowed: bool) -> Result<Self, DockerError> {
        // Host network mode detection (Go parity: `network == hostNetworkName`)
        let host_network_mode = if let Some(ref networks) = task.networks {
            networks.iter().any(|n| n == "host")
        } else {
            false
        };

        // Validate host network usage
        if host_network_mode && !host_network_allowed {
            return Err(DockerError::HostNetworkDisabled);
        }

        // Networking config with aliases (Go parity: `slug.Make(t.Name)`)
        // Note: Network aliases are not supported with host networking
        let networking_config =
            if task.networks.as_ref().is_none_or(Vec::is_empty) || host_network_mode {
                None
            } else {
                let mut endpoints = HashMap::new();
                if let Some(ref networks) = task.networks {
                    for nw in networks {
                        let alias =
                            slugify(task.name.as_deref().map_or(
                                task.id.as_ref().map_or("unknown", |id| id.as_str()),
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

        Ok(Self {
            networking_config,
            host_network_mode,
        })
    }
}
