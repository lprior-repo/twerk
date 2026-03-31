//! Task execution logic for `PodmanRuntime`.

#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;

use tracing::{error, warn};
use twerk_core::id::TaskId;
use twerk_core::uuid::new_uuid;

use super::super::errors::PodmanError;
use super::super::types::{CoreTask, Mount, HOST_NETWORK_NAME};
use super::types::PodmanRuntime;

impl PodmanRuntime {
    /// Main run method - validates task and executes pre/main/post tasks.
    pub(crate) async fn run_inner(&self, task: &mut CoreTask) -> Result<(), PodmanError> {
        // Validate task - must have ID, image, and name
        let _task_id = task.id.as_ref().ok_or(PodmanError::TaskIdRequired)?;
        let _task_name = task.name.as_ref().ok_or(PodmanError::NameRequired)?;
        let _task_image = task.image.as_ref().ok_or(PodmanError::ImageRequired)?;

        // Check for sidecars (not supported)
        if task.sidecars.as_ref().is_some_and(|s| !s.is_empty()) {
            return Err(PodmanError::SidecarsNotSupported);
        }

        // Check host network access
        if !self.host_network {
            if let Some(ref networks) = task.networks {
                if networks.iter().any(|n| n == HOST_NETWORK_NAME) {
                    return Err(PodmanError::HostNetworkingDisabled);
                }
            }
        }

        // Mount volumes and execute
        let mounted_mounts = self.prepare_mounts(task).await?;

        let result = self.execute_task_tree(task, &mounted_mounts).await;

        // Cleanup mounts
        self.cleanup_mounts(&mounted_mounts).await;

        result
    }

    /// Prepare mounts for task.
    #[allow(clippy::unused_async)]
    async fn prepare_mounts(&self, task: &CoreTask) -> Result<Vec<Mount>, PodmanError> {
        let mut mounted = Vec::new();
        if let Some(ref mounts) = task.mounts {
            for core_mnt in mounts {
                let mut mnt = Mount::from(core_mnt);
                mnt.id = core_mnt.id.clone().unwrap_or_else(new_uuid);
                if let Err(e) = self.mounter.mount(&mut mnt) {
                    error!("error mounting volume: {}", e);
                    return Err(PodmanError::WorkdirCreation(e.to_string()));
                }
                mounted.push(mnt);
            }
        }
        Ok(mounted)
    }

    /// Cleanup mounts after task execution.
    #[allow(clippy::unused_async)]
    async fn cleanup_mounts(&self, mounts: &[Mount]) {
        for mnt in mounts {
            if let Err(e) = self.mounter.unmount(mnt) {
                warn!("error unmounting volume {}: {}", mnt.target, e);
            }
        }
    }

    /// Execute pre tasks, main task, and post tasks.
    async fn execute_task_tree(
        &self,
        task: &CoreTask,
        mounted_mounts: &[Mount],
    ) -> Result<(), PodmanError> {
        // Convert mounted mounts back to CoreMount format for task execution
        let task_mounts: Vec<twerk_core::mount::Mount> = mounted_mounts
            .iter()
            .map(|m| twerk_core::mount::Mount {
                id: Some(m.id.clone()),
                mount_type: Some(m.mount_type.as_str().to_string()),
                source: Some(m.source.clone()),
                target: Some(m.target.clone()),
                opts: m.opts.clone(),
            })
            .collect();

        // Execute pre tasks
        if let Some(ref pre_tasks) = task.pre {
            for pre in pre_tasks {
                let mut pre_clone = pre.clone();
                pre_clone.id = Some(TaskId::new(new_uuid()));
                pre_clone.mounts = Some(task_mounts.clone());
                pre_clone.networks = task.networks.clone();
                pre_clone.limits = task.limits.clone();
                self.do_run(&mut pre_clone).await?;
            }
        }

        // Execute main task
        let mut main_clone = task.clone();
        main_clone.mounts = Some(task_mounts.clone());
        self.do_run(&mut main_clone).await?;

        // Execute post tasks
        if let Some(ref post_tasks) = task.post {
            for post in post_tasks {
                let mut post_clone = post.clone();
                post_clone.id = Some(TaskId::new(new_uuid()));
                post_clone.mounts = Some(task_mounts.clone());
                post_clone.networks = task.networks.clone();
                post_clone.limits = task.limits.clone();
                self.do_run(&mut post_clone).await?;
            }
        }

        Ok(())
    }

    /// Execute a single task (main, pre, or post).
    async fn do_run(&self, task: &mut CoreTask) -> Result<(), PodmanError> {
        self.active_tasks
            .fetch_add(1, std::sync::atomic::Ordering::SeqCst);

        let result = self.do_run_inner(task).await;

        self.active_tasks
            .fetch_sub(1, std::sync::atomic::Ordering::SeqCst);

        result
    }

    /// Inner execution - setup workdir and run container.
    async fn do_run_inner(&self, task: &mut CoreTask) -> Result<(), PodmanError> {
        // Setup work directory
        let task_id_str = task.id.as_ref().map_or("unknown", |id| id.as_str());
        let workdir = std::env::temp_dir().join("twerk").join(task_id_str);
        tokio::fs::create_dir_all(&workdir)
            .await
            .map_err(|e| PodmanError::WorkdirCreation(e.to_string()))?;

        // Create output and progress files
        let output_file = workdir.join("stdout");
        let progress_file = workdir.join("progress");

        tokio::fs::File::create(&output_file)
            .await
            .map_err(|e| PodmanError::FileWrite(e.to_string()))?;
        #[cfg(unix)]
        tokio::fs::set_permissions(&output_file, PermissionsExt::from_mode(0o777))
            .await
            .map_err(|e| PodmanError::FileWrite(e.to_string()))?;

        tokio::fs::File::create(&progress_file)
            .await
            .map_err(|e| PodmanError::FileWrite(e.to_string()))?;
        #[cfg(unix)]
        tokio::fs::set_permissions(&progress_file, PermissionsExt::from_mode(0o777))
            .await
            .map_err(|e| PodmanError::FileWrite(e.to_string()))?;

        // Write entrypoint script
        let entrypoint_path = workdir.join("entrypoint.sh");
        let run_script = if let Some(ref run) = task.run {
            run.clone()
        } else {
            task.cmd.as_ref().map_or(String::new(), |cmd| cmd.join(" "))
        };

        tokio::fs::write(&entrypoint_path, &run_script)
            .await
            .map_err(|e| PodmanError::FileWrite(e.to_string()))?;
        #[cfg(unix)]
        tokio::fs::set_permissions(&entrypoint_path, PermissionsExt::from_mode(0o755))
            .await
            .map_err(|e| PodmanError::FileWrite(e.to_string()))?;

        // Write task files
        if let Some(ref files) = task.files {
            if !files.is_empty() {
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
            }
        }

        // Execute container
        let result = self
            .execute_container(task, &workdir, &output_file, &progress_file)
            .await;

        // Cleanup workdir
        if let Err(e) = tokio::fs::remove_dir_all(&workdir).await {
            warn!("error removing workdir {:?}: {}", workdir, e);
        }

        result
    }
}
