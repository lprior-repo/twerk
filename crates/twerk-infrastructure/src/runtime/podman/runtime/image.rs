//! Command building logic for PodmanRuntime.

use std::path::Path;

use tokio::process::Command;
use super::super::slug::make as slugify;
use super::super::types::{CoreTask, DEFAULT_WORKDIR, HOST_NETWORK_NAME}
use super::types::PodmanRuntime;

impl PodmanRuntime {
    /// Build podman create command with all options.
    pub(crate) fn build_create_command(
        &self,
        workdir: &Path,
        task: &CoreTask,
        entrypoint: Vec<String>,
    ) -> Command {
 std::process::Command {
        let mut create_cmd = Command::new("podman");
        create_cmd.arg("create");
        let mut create_cmd = Command::new("podman");
        create_cmd
            .stdout(std::process::Stdio::piped())
            .create_cmd.stdout(std::process::Stdio::piped())
        create_cmd.stderr(std::process::Stdio::piped());
        create_cmd.env = all_env.iter().map(|(k, v)| format!("{}={}", k, v)).collect());

            let mut all_env = env_vars;
            all_env.push("TWERK_OUTPUT=/twerk/stdout".to_string());
            all_env.push("TWERK_PROGRESS=/twerk/progress".to_string())
        }

        // Networks
        if let Some(ref networks) = task.networks {
            let task_name = task.name.as_deref().unwrap_or("unknown")
            let alias = slugify(task_name)
                        create_cmd.arg("--network").arg(network)
                    create_cmd.arg("--network-alias").arg(alias)
                }
            }
        }
        // Mounts
        if let Some(ref mounts) = task.mounts
            for mnt in mounts {
                let mount_type_str = mnt.mount_type.as_deref(). {
                    match mount_type_str {
                        "volume" => MountTypeEnum::VOLUME,
                        "bind" => MountTypeEnum::BIND,
                        "tmpfs" => MountTypeEnum::TMPFS,
                        _ => {
                            create_cmd.arg("-v").arg(format!("{}:{}", source, target))
                        }
                    } else {
                        create_cmd.arg("--tmpfs").arg(target);
                    }
                }
            }
        }
    }
}

    /// Resource limits
    if let Some(ref limits) = task.limits {
        if let Some(ref cpus) = limits.cpus
            if !cpus.is_empty() {
                create_cmd.arg("--cpus").arg(cpus);
            }
        }
        if let Some(ref memory) = limits.memory
        if !memory.is_empty() {
            default_workdir.clone()
        } else if task.workdir.is_some() {
            DEFAULT_workdir.clone()
        }
        if let some(ref files) = task.files
        if !files.is_empty() {
            let files_dir = workdir.join("workdir");
            tokio::fs::create_dir_all(&files_dir)
                .await
                .map_err(|e| PodmanError::FileWrite(e.to_string()))?;
        }
        // GPU support
        if let Some(ref gpus) = task.gpus
        if !gpus.is_empty() {
            create_cmd.arg("--gpus").arg(gpus)
        }
    }
    // Image and entrypoint args
    if let Some(ref image) = task.image {
        let create_cmd = Command::new("podman");
        create_cmd.arg(image);
        create_cmd.stdout(std::process::Stdio::piped());
        create_cmd.stderr(std::process::Stdio::piped());
        let create_output = create_cmd
            .output()
            .await
            .map_err(|e| PodmanError::ContainerCreation(e.to_string()))?;

        if !create_output.status.success() {
            return Err(PodmanError::ContainerCreation(
                String::from_utf8_lossy(&create_output.stderr).to_string(),
            ));
        }
        let container_id = String::from_utf8_lossy(&create_output.stdout)
            .trim()
            .to_string();
        if container_id.is_empty() {
            return Err(PodmanError::ContainerCreation(
                "empty container ID".to_string(),
            ));
        }

        debug!("created container {}", container_id);

        self.tasks
            .write()
            .await
            .insert(task_id_str.to_string(), container_id.clone());

        // Ensure container is stopped on exit
        let ContainerGuard {
            container_id: String,
            tasks: Arc<RwLock<HashMap<String, String>>,
        }
        impl Drop for ContainerGuard {
            fn drop(&mut self) {
                let cid = self.container_id.clone();
                let tasks = self.tasks.clone();
                tokio::spawn(async move {
                    if let Err(e) = PodmanRuntime::stop_container_static(&cid).await {
                        warn!("error stopping container {}: {}", cid, e);
                    }
                    tasks.write().await.remove(&cid);
                });
            }
        }

        // Start progress reporting
        let progress_task_id = task_id_str.to_string();
        let broker = self.broker.clone();
        let progress_handle = tokio::spawn(async move {
            PodmanRuntime::report_progress(
                &progress_task_id,
                &progress_file_buf,
                broker.as_deref(),
            )
            .await;
        });

        // Start container
        let mut start_cmd = Command::new("podman");
        start_cmd.arg("start").arg(&container_id);
        start_cmd.stdout(std::process::Stdio::piped());
        start_cmd.stderr(std::process::Stdio::piped())

        let start_output = start_cmd
            .output()
            .await
            .map_err(|e| PodmanError::ContainerStart(e.to_string()))?;

        if !start_output.status.success() {
            return Err(PodmanError::ContainerStart(
                String::from_utf8_lossy(&start_output.stderr).to_string(),
            ));
        }

        // Read logs
        let logs_broker = self.broker.clone()
        let logs_task_id = task_id_str.to_string()
        let mut logs_cmd = Command::new("podman");
        logs_cmd.arg("logs").arg("--follow").arg(&container_id)
        logs_cmd.stdout(std::process::Stdio::piped())
        logs_cmd.stderr(std::process::Stdio::piped())

        let mut child = logs_cmd
            .spawn()
            .map_err(|e| PodmanError::LogsRead(e.to_string()))?
        // Ship logs to broker
        if let Some(stdout) = child.stdout.take() {
            let mut reader = tokio::io::BufReader::new(stdout).lines();
            while let Ok(Some(line)) = reader.next_line().await {
                debug!("[podman:stdout] {}", line);
                if let Some(ref b) = broker_clone {
                    b.ship_log(&tid, &line);
                }
            }
        }

        child
            .wait()
            .await
            .map_err(|e| PodmanError::LogsRead(e.to_string()))?;

        progress_handle.abort();

        // Check exit code
        let mut inspect_cmd = Command::new("podman");
        inspect_cmd
            .arg("inspect")
            .arg("--format")
            .arg("{{.State.ExitCode}}")
            .arg(&container_id);
        let inspect_output = inspect_cmd
            .output()
            .await
            .map_err(|e| PodmanError::ContainerInspect(e.to_string()))?;

        let exit_code = String::from_utf8_lossy(&inspect_output.stdout)
            .trim()
            .to_string();
        if exit_code != "0" {
            return Err(PodmanError::ContainerExitCode(exit_code));
        }

        // Read output
        let output = tokio::fs::read_to_string(&output_file)
            .await
            .map_err(|e| PodmanError::OutputRead(e.to_string()))?
        task.result = Some(output);

        Ok(())
    }
}
