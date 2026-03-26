//! Container setup logic for Podman runtime

use std::os::unix::fs::PermissionsExt;
use std::path::PathBuf;

use tokio::process::Command;

use super::errors::PodmanError;
use super::types::{MountType, PodmanRuntime, Task};

#[allow(dead_code)]
impl PodmanRuntime {
    /// Setup work directory and files for task execution
    pub(crate) async fn setup_workdir(&self, task: &mut Task) -> Result<(PathBuf, PathBuf, PathBuf), PodmanError> {
        // Create temp workdir
        let workdir = std::env::temp_dir().join("twerk").join(&task.id);
        tokio::fs::create_dir_all(&workdir)
            .await
            .map_err(|e| PodmanError::WorkdirCreation(e.to_string()))?;

        // Create output and progress files
        let output_file = workdir.join("stdout");
        let progress_file = workdir.join("progress");

        tokio::fs::File::create(&output_file)
            .await
            .map_err(|e| PodmanError::FileWrite(e.to_string()))?;
        tokio::fs::set_permissions(&output_file, std::fs::Permissions::from_mode(0o777))
            .await
            .map_err(|e| PodmanError::FileWrite(e.to_string()))?;

        tokio::fs::File::create(&progress_file)
            .await
            .map_err(|e| PodmanError::FileWrite(e.to_string()))?;
        tokio::fs::set_permissions(&progress_file, std::fs::Permissions::from_mode(0o777))
            .await
            .map_err(|e| PodmanError::FileWrite(e.to_string()))?;

        // Write entrypoint script
        let entrypoint_path = workdir.join("entrypoint.sh");
        let run_script = if !task.run.is_empty() {
            task.run.clone()
        } else {
            task.cmd.join(" ")
        };

        tokio::fs::write(&entrypoint_path, &run_script)
            .await
            .map_err(|e| PodmanError::FileWrite(e.to_string()))?;

        tokio::fs::set_permissions(&entrypoint_path, std::fs::Permissions::from_mode(0o755))
            .await
            .map_err(|e| PodmanError::FileWrite(e.to_string()))?;

        Ok((workdir, output_file, progress_file))
    }

    /// Write task files to workdir
    pub(crate) async fn write_task_files(
        workdir: &PathBuf,
        files: &std::collections::HashMap<String, String>,
    ) -> Result<(), PodmanError> {
        if files.is_empty() {
            return Ok(());
        }

        let files_dir = workdir.join("workdir");
        tokio::fs::create_dir_all(&files_dir)
            .await
            .map_err(|e| PodmanError::FileWrite(e.to_string()))?;

        for (filename, contents) in files {
            let file_path = files_dir.join(filename);
            if let Some(parent) = file_path.parent() {
                tokio::fs::create_dir_all(parent)
                    .await
                    .map_err(|e| PodmanError::FileWrite(e.to_string()))?;
            }
            tokio::fs::write(&file_path, contents)
                .await
                .map_err(|e| PodmanError::FileWrite(e.to_string()))?;
        }

        Ok(())
    }

    /// Build podman create command with all options
    pub(crate) fn build_create_command(
        workdir: &PathBuf,
        task: &Task,
        entrypoint: Vec<String>,
    ) -> Command {
        let mut create_cmd = Command::new("podman");
        create_cmd.arg("create");
        create_cmd
            .arg("-v")
            .arg(format!("{}:/twerk", workdir.display()));
        create_cmd.arg("--entrypoint").arg(&entrypoint[0]);

        // Environment variables
        let env_vars: Vec<String> = task
            .env
            .iter()
            .map(|(k, v)| format!("{}={}", k, v))
            .chain([
                "TWERK_OUTPUT=/twerk/stdout".to_string(),
                "TWERK_PROGRESS=/twerk/progress".to_string(),
            ])
            .collect();

        for env in &env_vars {
            create_cmd.arg("-e").arg(env);
        }

        // Networks
        for network in &task.networks {
            if network == super::types::HOST_NETWORK_NAME {
                create_cmd.arg("--network").arg(network);
            } else {
                let alias = super::slug::make(&task.name.clone().unwrap_or_default());
                create_cmd
                    .arg("--network")
                    .arg(network)
                    .arg("--network-alias")
                    .arg(alias);
            }
        }

        // Mounts
        for mount in &task.mounts {
            match mount.mount_type {
                MountType::Volume | MountType::Bind => {
                    let vol_spec = if let Some(ref opts) = mount.opts {
                        if opts.is_empty() {
                            format!("{}:{}", mount.source, mount.target)
                        } else {
                            let opt_str: String = opts
                                .iter()
                                .map(|(k, v)| format!("{}={}", k, v))
                                .collect::<Vec<_>>()
                                .join(",");
                            format!("{}:{}:{}", mount.source, mount.target, opt_str)
                        }
                    } else {
                        format!("{}:{}", mount.source, mount.target)
                    };
                    create_cmd.arg("-v").arg(vol_spec);
                }
                MountType::Tmpfs => {
                    let tmpfs_spec = if let Some(ref opts) = mount.opts {
                        opts.iter()
                            .map(|(k, v)| format!("{}={}", k, v))
                            .collect::<Vec<_>>()
                            .join(",")
                    } else {
                        String::new()
                    };
                    if tmpfs_spec.is_empty() {
                        create_cmd.arg("--tmpfs").arg(&mount.target);
                    } else {
                        create_cmd
                            .arg("--tmpfs")
                            .arg(format!("{}:{}", mount.target, tmpfs_spec));
                    }
                }
            }
        }

        // Resource limits
        if let Some(ref limits) = task.limits {
            if !limits.cpus.is_empty() {
                create_cmd.arg("--cpus").arg(limits.cpus.clone());
            }
            if !limits.memory.is_empty() {
                let bytes = super::types::PodmanRuntime::parse_memory(&limits.memory).unwrap_or(0);
                create_cmd.arg("--memory").arg(bytes.to_string());
            }
        }

        // GPU support
        if let Some(ref gpus) = task.gpus {
            if !gpus.is_empty() {
                create_cmd.arg("--gpus").arg(gpus);
            }
        }

        // Probe support
        if let Some(ref probe) = task.probe {
            let port_str = probe.port.to_string();
            create_cmd.arg("--expose").arg(format!("{}/tcp", port_str));
            create_cmd
                .arg("-p")
                .arg(format!("127.0.0.1::{}/tcp", port_str));
        }

        // Workdir
        let effective_workdir = if let Some(ref wd) = task.workdir {
            wd.clone()
        } else if !task.files.is_empty() {
            super::types::DEFAULT_WORKDIR.to_string()
        } else {
            String::new()
        };

        if !effective_workdir.is_empty() {
            create_cmd.arg("-w").arg(&effective_workdir);
        }

        // Privileged mode will be added by caller

        create_cmd.arg(&task.image);
        for arg in entrypoint.iter().skip(1) {
            create_cmd.arg(arg);
        }
        create_cmd.arg("/twerk/entrypoint.sh");

        create_cmd.stdout(std::process::Stdio::piped());
        create_cmd.stderr(std::process::Stdio::piped());

        create_cmd
    }

    /// Add privileged flag to command
    pub(crate) fn add_privileged_flag(&self, cmd: &mut Command) {
        if self.privileged {
            cmd.arg("--privileged");
        }
    }
}
