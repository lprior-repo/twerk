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
        shell_cmd: config_strings_default("runtime.shell.cmd", &["bash", "-c"]),
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
    twerk_infrastructure::config::string(k)
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
    twerk_infrastructure::config::bool(k)
}

fn config_strings(k: &str) -> Vec<String> {
    twerk_infrastructure::config::strings(k)
}

fn config_strings_default(k: &str, default: &[&str]) -> Vec<String> {
    twerk_infrastructure::config::strings_default(k, default)
}

fn config_u64(k: &str) -> u64 {
    match u64::try_from(twerk_infrastructure::config::int_default(k, 0)) {
        Ok(value) => value,
        Err(_) => {
            tracing::warn!(config_key = %k, "invalid u64 value in config, using 0");
            0
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    struct EnvGuard {
        key: &'static str,
    }

    impl EnvGuard {
        fn set(key: &'static str, value: &str) -> Self {
            std::env::set_var(key, value);
            Self { key }
        }
    }

    impl Drop for EnvGuard {
        fn drop(&mut self) {
            std::env::remove_var(self.key);
        }
    }

    #[test]
    fn read_runtime_config_reads_shell_cmd_array_from_config_file() {
        let temp_dir = tempfile::tempdir().expect("temp dir");
        let config_path = temp_dir.path().join("shell-config.toml");
        fs::write(
            &config_path,
            "[runtime]\ntype = \"shell\"\n[runtime.shell]\ncmd = [\"bash\", \"-c\"]\n",
        )
        .expect("write config");
        let config_path_value = config_path.to_string_lossy().into_owned();
        let _guard = EnvGuard::set("TWERK_CONFIG", config_path_value.as_str());

        twerk_common::load_config().expect("load config");
        let config = read_runtime_config();

        assert_eq!(config.runtime_type, runtime_type::SHELL);
        assert_eq!(config.shell_cmd, vec!["bash", "-c"]);
    }
}
