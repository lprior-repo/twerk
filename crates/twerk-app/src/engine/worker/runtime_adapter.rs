use crate::engine::worker::docker::DockerRuntimeAdapter;
use crate::engine::worker::mounter::{
    BindConfig, BindMounter, MountPolicy, TmpfsMounter, VolumeMounter,
};
use crate::engine::worker::podman::PodmanRuntimeAdapter;
use crate::engine::worker::shell::ShellRuntimeAdapter;
use anyhow::{anyhow, Result};
use std::sync::Arc;
use twerk_infrastructure::broker::Broker;
use twerk_infrastructure::runtime::{MultiMounter, Runtime as RuntimeTrait};

use crate::engine::engine_helpers::ensure_config_loaded;

pub mod runtime_type {
    pub const DOCKER: &str = "docker";
    pub const SHELL: &str = "shell";
    pub const PODMAN: &str = "podman";
    pub const DEFAULT: &str = DOCKER;
}

#[derive(Debug, Clone, Default)]
pub struct RuntimeConfig {
    pub runtime_type: String,
    pub docker_privileged: bool,
    pub docker_image_ttl_secs: u64,
    pub docker_image_verify: bool,
    pub docker_config: String,
    pub shell_cmd: Vec<String>,
    pub shell_uid: String,
    pub shell_gid: String,
    pub podman_privileged: bool,
    pub podman_host_network: bool,
    pub bind_allowed: bool,
    pub bind_sources: Vec<String>,
    pub hostenv_vars: Vec<String>,
}

pub async fn create_runtime_from_config(
    config: &RuntimeConfig,
    broker: Arc<dyn Broker + Send + Sync>,
) -> Result<Box<dyn RuntimeTrait + Send + Sync>> {
    match config.runtime_type.as_str() {
        runtime_type::DOCKER => {
            let mut m = MultiMounter::default();
            let bind_policy = if config.bind_allowed {
                MountPolicy::Allowed(config.bind_sources.clone())
            } else {
                MountPolicy::Denied
            };
            m.register_mounter(
                "bind",
                Box::new(BindMounter::new(BindConfig {
                    policy: bind_policy,
                })),
            )
            .map_err(|e| anyhow!("{e}"))?;
            m.register_mounter("volume", Box::new(VolumeMounter::new()))
                .map_err(|e| anyhow!("{e}"))?;
            m.register_mounter("tmpfs", Box::new(TmpfsMounter::new()))
                .map_err(|e| anyhow!("{e}"))?;
            Ok(Box::new(DockerRuntimeAdapter::new(
                config.docker_privileged,
                config.docker_image_ttl_secs,
                Arc::new(m),
                broker,
            )))
        }
        runtime_type::SHELL => Ok(Box::new(ShellRuntimeAdapter::new(
            config.shell_cmd.clone(),
            config.shell_uid.clone(),
            config.shell_gid.clone(),
            Some(broker),
        ))),
        runtime_type::PODMAN => Ok(Box::new(PodmanRuntimeAdapter::new(
            config.podman_privileged,
            config.podman_host_network,
        ))),
        other => Err(anyhow!("unknown runtime type: {}", other)),
    }
}

pub fn read_runtime_config() -> RuntimeConfig {
    ensure_config_loaded();
    let rt = config_string_default("runtime.type", runtime_type::DEFAULT);
    RuntimeConfig {
        runtime_type: rt,
        docker_privileged: config_bool("runtime.docker.privileged"),
        docker_image_ttl_secs: config_u64("runtime.docker.image.ttl"),
        docker_image_verify: config_bool("runtime.docker.image.verify"),
        docker_config: config_string_default("runtime.docker.config", ""),
        shell_cmd: config_strings("runtime.shell.cmd"),
        shell_uid: config_string_default("runtime.shell.uid", "-"),
        shell_gid: config_string_default("runtime.shell.gid", "-"),
        podman_privileged: config_bool("runtime.podman.privileged"),
        podman_host_network: config_bool("runtime.podman.host.network"),
        bind_allowed: config_bool("mounts.bind.allowed"),
        bind_sources: config_strings("mounts.bind.sources"),
        hostenv_vars: config_strings("middleware.task.hostenv.vars"),
    }
}

fn config_string(k: &str) -> String {
    std::env::var(format!("TWERK_{}", k.to_uppercase().replace('.', "_")))
        .ok()
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| twerk_infrastructure::config::string(k))
}

fn config_string_default(k: &str, d: &str) -> String {
    let v = config_string(k);
    if v.is_empty() {
        d.to_string()
    } else {
        v
    }
}

fn config_bool(k: &str) -> bool {
    let v = config_string(k);
    v.eq_ignore_ascii_case("true") || v == "1"
}

fn config_strings(k: &str) -> Vec<String> {
    let v = config_string(k);
    if v.is_empty() {
        vec![]
    } else if v.starts_with('[') {
        v.trim_matches(|c| c == '[' || c == ']')
            .split(',')
            .map(|s| s.trim().trim_matches('"').to_string())
            .filter(|s| !s.is_empty())
            .collect()
    } else {
        v.split(',').map(|s| s.trim().to_string()).collect()
    }
}

fn config_u64(k: &str) -> u64 {
    let v = config_string(k);
    v.parse().unwrap_or(0)
}
