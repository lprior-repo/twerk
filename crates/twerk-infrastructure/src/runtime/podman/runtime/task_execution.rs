//! Task execution logic for `PodmanRuntime`.

#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;

use tracing::{error, instrument, warn};
use twerk_core::id::TaskId;
use twerk_core::uuid::new_uuid;

use super::super::errors::PodmanError;
use super::super::types::{CoreTask, Mount, HOST_NETWORK_NAME};
use super::types::PodmanRuntime;

impl PodmanRuntime {
    /// Main run method - validates task and executes pre/main/post tasks.
    #[instrument(name = "podman_run", skip_all)]
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
                mnt.id = core_mnt.id.as_ref().map_or_else(new_uuid, |id| id.clone());
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
    #[instrument(name = "podman_execute_task_tree", skip_all)]
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
                pre_clone.id = Some(TaskId::new(new_uuid())?);
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
                post_clone.id = Some(TaskId::new(new_uuid())?);
                post_clone.mounts = Some(task_mounts.clone());
                post_clone.networks = task.networks.clone();
                post_clone.limits = task.limits.clone();
                self.do_run(&mut post_clone).await?;
            }
        }

        Ok(())
    }

    /// Execute a single task (main, pre, or post).
    #[instrument(name = "podman_do_run", skip_all)]
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
        let task_id_str = task.id.as_ref().map_or("unknown", |id| id.as_str());
        let workdir = std::env::temp_dir().join("twerk").join(task_id_str);

        self.setup_workdir(&workdir, task).await?;

        let result = self
            .execute_container(
                task,
                &workdir,
                &workdir.join("stdout"),
                &workdir.join("progress"),
            )
            .await;

        if let Err(e) = tokio::fs::remove_dir_all(&workdir).await {
            warn!("error removing workdir {:?}: {}", workdir, e);
        }

        result
    }

    async fn setup_workdir(
        &self,
        workdir: &std::path::Path,
        task: &CoreTask,
    ) -> Result<(), PodmanError> {
        tokio::fs::create_dir_all(workdir)
            .await
            .map_err(|e| PodmanError::WorkdirCreation(e.to_string()))?;

        let output_file = workdir.join("stdout");
        create_file_with_permissions(&output_file, 0o777).await?;
        let progress_file = workdir.join("progress");
        create_file_with_permissions(&progress_file, 0o777).await?;

        let entrypoint_path = workdir.join("entrypoint.sh");
        let run_script = match task.run {
            Some(ref cmd) => cmd.clone(),
            None => task.cmd.as_ref().map_or(String::new(), |c| c.join(" ")),
        };
        tokio::fs::write(&entrypoint_path, &run_script)
            .await
            .map_err(|e| PodmanError::FileWrite(e.to_string()))?;
        #[cfg(unix)]
        tokio::fs::set_permissions(&entrypoint_path, PermissionsExt::from_mode(0o755))
            .await
            .map_err(|e| PodmanError::FileWrite(e.to_string()))?;

        write_task_files(workdir, &task.files).await?;

        Ok(())
    }
}

async fn create_file_with_permissions(
    path: &std::path::Path,
    mode: u32,
) -> Result<(), PodmanError> {
    tokio::fs::File::create(path)
        .await
        .map_err(|e| PodmanError::FileWrite(e.to_string()))?;
    #[cfg(unix)]
    tokio::fs::set_permissions(path, PermissionsExt::from_mode(mode))
        .await
        .map_err(|e| PodmanError::FileWrite(e.to_string()))?;
    Ok(())
}

async fn write_task_files(
    workdir: &std::path::Path,
    files: &Option<std::collections::HashMap<String, String>>,
) -> Result<(), PodmanError> {
    if let Some(ref files) = files {
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
    }
    Ok(())
}
