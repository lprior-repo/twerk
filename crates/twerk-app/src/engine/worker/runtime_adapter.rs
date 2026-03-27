use anyhow::{anyhow, Result};
use std::process::ExitCode;
use twerk_infrastructure::runtime::{MultiMounter, Runtime as RuntimeTrait, ShutdownResult};
use crate::engine::worker::mounter::{BindConfig, BindMounter, TmpfsMounter, VolumeMounter};
use crate::engine::worker::docker::DockerRuntimeAdapter;
use crate::engine::worker::shell::ShellRuntimeAdapter;
use crate::engine::worker::podman::PodmanRuntimeAdapter;
use twerk_core::task::Task;

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
) -> Result<Box<dyn RuntimeTrait + Send + Sync>> {
    match config.runtime_type.as_str() {
        runtime_type::DOCKER => {
            let mut m = MultiMounter::default();
            m.register_mounter(
                "bind",
                Box::new(BindMounter::new(BindConfig {
                    allowed: config.bind_allowed,
                    sources: config.bind_sources.clone(),
                })),
            )
            .map_err(|e| anyhow!("{e}"))?;
            m.register_mounter("volume", Box::new(VolumeMounter::new()))
                .map_err(|e| anyhow!("{e}"))?;
            m.register_mounter("tmpfs", Box::new(TmpfsMounter::new()))
                .map_err(|e| anyhow!("{e}"))?;
            Ok(Box::new(DockerRuntimeAdapter::new(config.docker_privileged, config.docker_image_ttl_secs)))
        }
        runtime_type::SHELL => {
            Ok(Box::new(ShellRuntimeAdapter::new(
                config.shell_cmd.clone(),
                config.shell_uid.clone(),
                config.shell_gid.clone(),
            )))
        }
        runtime_type::PODMAN => Ok(Box::new(PodmanRuntimeAdapter::new(
            config.podman_privileged,
            config.podman_host_network,
        ))),
        other => Err(anyhow!("unknown runtime type: {}", other)),
    }
}

pub fn read_runtime_config() -> RuntimeConfig {
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
        ..Default::default()
    }
}

fn config_string(k: &str) -> String {
    std::env::var(format!("TWERK_{}", k.to_uppercase().replace('.', "_")))
        .unwrap_or_default()
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
    v.to_lowercase() == "true" || v == "1"
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
        v.split(',')
            .map(|s| s.trim().to_string())
            .collect()
    }
}

fn config_u64(k: &str) -> u64 {
    let v = config_string(k);
    v.parse().unwrap_or(0)
}

#[derive(Debug)]
pub struct MockRuntime;

impl RuntimeTrait for MockRuntime {
    fn run(&self, _task: &Task) -> twerk_infrastructure::runtime::BoxedFuture<()> {
        Box::pin(async { Ok(()) })
    }

   fn stop(&self, _task: &Task) -> twerk_infrastructure::runtime::BoxedFuture<ShutdownResult<ExitCode>> {
        // Mock stop is idempotent - always returns success
        Box::pin(async { Ok(Ok(std::process::ExitCode::SUCCESS)) })
    }

    fn health_check(&self) -> twerk_infrastructure::runtime::BoxedFuture<()> {
        Box::pin(async { Ok(()) })
    }
}
