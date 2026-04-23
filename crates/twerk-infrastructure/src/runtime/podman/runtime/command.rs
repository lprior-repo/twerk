//! Command building logic for `PodmanRuntime`.

use std::path::Path;

use tokio::process::Command;

use super::super::types::{CoreTask, DEFAULT_WORKDIR, HOST_NETWORK_NAME};
use super::types::PodmanRuntime;
use crate::runtime::docker::helpers::slugify;
use twerk_common::constants::{DEFAULT_MOUNT_TYPE, DEFAULT_TASK_NAME};
use twerk_core::env::format_kv;

impl PodmanRuntime {
    /// Build podman create command with all options
    #[allow(clippy::needless_pass_by_value, clippy::unused_self)]
    pub(crate) fn build_create_command(
        &self,
        workdir: &Path,
        task: &CoreTask,
        entrypoint: Vec<String>,
    ) -> Command {
        let mut create_cmd = Command::new("podman");
        create_cmd.arg("create");
        create_cmd
            .arg("-v")
            .arg(format!("{}:/twerk", workdir.display()));

        if !entrypoint.is_empty() {
            create_cmd.arg("--entrypoint").arg(&entrypoint[0]);
        }

        // Environment variables
        let env_vars: Vec<String> = task.env.as_ref().map_or(Vec::new(), |env| {
            env.iter().map(|(k, v)| format_kv(k, v)).collect()
        });

        let mut all_env = env_vars;
        all_env.push("TWERK_OUTPUT=/twerk/stdout".to_string());
        all_env.push("TWERK_PROGRESS=/twerk/progress".to_string());

        for env in &all_env {
            create_cmd.arg("-e").arg(env);
        }

        // Networks
        if let Some(ref networks) = task.networks {
            let task_name = task.name.as_deref().map_or(DEFAULT_TASK_NAME, |s| s);
            for network in networks {
                if network == HOST_NETWORK_NAME {
                    create_cmd.arg("--network").arg(network);
                } else {
                    let alias = slugify(task_name);
                    create_cmd.arg("--network").arg(network);
                    create_cmd.arg("--network-alias").arg(alias);
                }
            }
        }

        // Mounts
        if let Some(ref mounts) = task.mounts {
            for mnt in mounts {
                let mount_type_str = mnt.mount_type.as_deref().map_or(DEFAULT_MOUNT_TYPE, |s| s);
                if mount_type_str == "tmpfs" {
                    let target = mnt.target.as_deref().map_or("", |s| s);
                    create_cmd.arg("--tmpfs").arg(target);
                } else {
                    // bind, volume, and any other mount type
                    let source = mnt.source.as_deref().map_or("", |s| s);
                    let target = mnt.target.as_deref().map_or("", |s| s);
                    create_cmd.arg("-v").arg(format!("{source}:{target}"));
                }
            }
        }

        // Resource limits
        if let Some(ref limits) = task.limits {
            if let Some(ref cpus) = limits.cpus {
                if !cpus.is_empty() {
                    create_cmd.arg("--cpus").arg(cpus);
                }
            }
            if let Some(ref memory) = limits.memory {
                if !memory.is_empty() {
                    let bytes = Self::parse_memory(memory).map_or(0, |v| v);
                    create_cmd.arg("--memory").arg(bytes.to_string());
                }
            }
        }

        // GPU support
        if let Some(ref gpus) = task.gpus {
            if !gpus.is_empty() {
                create_cmd.arg("--gpus").arg(gpus);
            }
        }

        // Workdir
        let effective_workdir = if task.workdir.is_some() {
            task.workdir.clone()
        } else if task.files.as_ref().is_some_and(|f| !f.is_empty()) {
            Some(DEFAULT_WORKDIR.to_string())
        } else {
            None
        };

        if let Some(ref wd) = effective_workdir {
            if !wd.is_empty() {
                create_cmd.arg("-w").arg(wd);
            }
        }

        // Image and entrypoint args
        if let Some(ref image) = task.image {
            create_cmd.arg(image);
        }
        for arg in entrypoint.iter().skip(1) {
            create_cmd.arg(arg);
        }
        create_cmd.arg("/twerk/entrypoint.sh");

        create_cmd.stdout(std::process::Stdio::piped());
        create_cmd.stderr(std::process::Stdio::piped());

        create_cmd
    }

    /// Parse memory string to bytes
    #[allow(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        clippy::cast_precision_loss
    )]
    pub(crate) fn parse_memory(memory: &str) -> Option<u64> {
        let memory = memory.trim();
        let (num_str, multiplier) = if let Some(suffix) = memory.strip_suffix("gb") {
            (suffix.trim_end(), 1_073_741_824u64)
        } else if let Some(suffix) = memory.strip_suffix("g") {
            (suffix.trim_end(), 1_073_741_824u64)
        } else if let Some(suffix) = memory.strip_suffix("mb") {
            (suffix.trim_end(), 1_048_576u64)
        } else if let Some(suffix) = memory.strip_suffix("m") {
            (suffix.trim_end(), 1_048_576u64)
        } else if let Some(suffix) = memory.strip_suffix("kb") {
            (suffix.trim_end(), 1024u64)
        } else if let Some(suffix) = memory.strip_suffix("k") {
            (suffix.trim_end(), 1024u64)
        } else if let Some(suffix) = memory.strip_suffix("b") {
            (suffix.trim_end(), 1u64)
        } else {
            (memory, 1u64)
        };

        let value: f64 = num_str.parse().ok()?;
        Some((value * multiplier as f64) as u64)
    }
}
