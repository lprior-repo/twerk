//! Task execution logic for Docker runtime.

use std::sync::Arc;

use bollard::query_parameters::{RemoveContainerOptions, RemoveVolumeOptions};
use dashmap::DashMap;
use tokio::sync::{mpsc, RwLock};

use super::super::config::DockerConfig;
use super::super::error::DockerError;
use super::super::mounters::Mounter;
use super::super::network;
use super::container_create::create_container;
use super::types::PullRequest;
use tracing::instrument;
use twerk_core::id::TaskId;
use twerk_core::task::Task;
use twerk_core::uuid::new_uuid;

/// Runs a task in a Docker container.
///
/// # Errors
///
/// Returns `DockerError` if the task cannot be executed.
#[instrument(name = "docker_run", skip_all, fields(task_id = %task.id.as_ref().map_or("unknown", |id| id.as_str())))]
pub(super) async fn run(
    client: bollard::Docker,
    config: DockerConfig,
    _images: Arc<DashMap<String, std::time::Instant>>,
    pull_tx: mpsc::Sender<PullRequest>,
    tasks: Arc<RwLock<usize>>,
    mounter: Arc<dyn Mounter>,
    task: &mut Task,
) -> Result<(), DockerError> {
    // Decrement task count when done (deferred via drop guard)
    struct Guard(Arc<RwLock<usize>>);
    impl Drop for Guard {
        fn drop(&mut self) {
            if let Ok(mut count) = self.0.try_write() {
                *count = count.saturating_sub(1);
            }
        }
    }

    // Increment task count
    {
        let mut count = tasks.write().await;
        *count += 1;
    }
    let _guard = Guard(tasks.clone());

    // If the task has sidecars, create a network
    let network_id = if let Some(ref sidecars) = task.sidecars {
        if sidecars.is_empty() {
            None
        } else {
            let id = network::create_network(&client).await?;
            if let Some(ref mut networks) = task.networks {
                networks.push(id.clone());
            }
            Some(id)
        }
    } else {
        None
    };

    // Prepare mounts
    let mut mounted_mounts = Vec::new();
    let mut mount_error = None;
    if let Some(ref mounts) = task.mounts {
        for mnt in mounts {
            let mut mnt = mnt.clone();
            mnt.id = Some(new_uuid());
            if let Err(e) = mounter.mount(&mnt).await {
                mount_error = Some(DockerError::Mount(e));
                break;
            }
            mounted_mounts.push(mnt);
        }
    }
    task.mounts = Some(mounted_mounts.clone());

    // Run all task phases, collecting the first error
    let result = if mount_error.is_some() {
        mount_error
    } else {
        run_phases(&client, &config, &pull_tx, &mounter, task, &mounted_mounts)
            .await
            .err()
    };

    // Clean up mounts (always, even on error)
    for mnt in &mounted_mounts {
        if let Err(e) = mounter.unmount(mnt).await {
            tracing::error!(error = %e, mount = ?mnt, "error unmounting");
        }
    }

    // Clean up network (always, even on error)
    if let Some(ref id) = network_id {
        network::remove_network(&client, id).await;
    }

    match result {
        Some(e) => Err(e),
        None => Ok(()),
    }
}

async fn run_phases(
    client: &bollard::Docker,
    config: &DockerConfig,
    pull_tx: &mpsc::Sender<PullRequest>,
    mounter: &Arc<dyn Mounter>,
    task: &mut Task,
    mounted_mounts: &[twerk_core::mount::Mount],
) -> Result<(), DockerError> {
    let pre_tasks: Vec<Task> = task
        .pre
        .as_ref()
        .map_or_else(Vec::new, std::clone::Clone::clone);
    for mut pre_task in pre_tasks {
        pre_task.id = Some(TaskId::new(new_uuid())?);
        pre_task.mounts = Some(mounted_mounts.to_vec());
        pre_task.networks = task.networks.clone();
        pre_task.limits = task.limits.clone();
        run_task(client, config, pull_tx, mounter, &mut pre_task).await?;
    }

    run_task(client, config, pull_tx, mounter, task).await?;

    let post_tasks: Vec<Task> = task
        .post
        .as_ref()
        .map_or_else(Vec::new, std::clone::Clone::clone);
    for mut post_task in post_tasks {
        post_task.id = Some(TaskId::new(new_uuid())?);
        post_task.mounts = Some(mounted_mounts.to_vec());
        post_task.networks = task.networks.clone();
        post_task.limits = task.limits.clone();
        run_task(client, config, pull_tx, mounter, &mut post_task).await?;
    }

    Ok(())
}

async fn remove_container_and_volume(
    client: &bollard::Docker,
    container_id: &str,
    volume_name: Option<&str>,
    label: &str,
) {
    if let Err(e) = client
        .remove_container(
            container_id,
            Some(RemoveContainerOptions {
                force: true,
                ..Default::default()
            }),
        )
        .await
    {
        tracing::warn!(error = %e, container_id = %container_id, "failed to remove {label} container");
    }

    if let Some(volume) = volume_name {
        for attempt in 0..3 {
            if attempt > 0 {
                tokio::time::sleep(std::time::Duration::from_millis(
                    100 * 2u64.pow(attempt - 1),
                ))
                .await;
            }
            match client
                .remove_volume(volume, Some(RemoveVolumeOptions { force: true }))
                .await
            {
                Ok(()) => break,
                Err(e) if attempt < 2 => {
                    tracing::debug!(error = %e, volume = %volume, attempt, "retrying {label} volume removal");
                }
                Err(e) => {
                    tracing::warn!(error = %e, volume = %volume, "failed to remove {label} volume after 3 attempts");
                }
            }
        }
    }
}

/// Runs a single task (main, pre, or post).
#[instrument(name = "docker_run_task", skip_all)]
async fn run_task(
    client: &bollard::Docker,
    config: &DockerConfig,
    pull_tx: &mpsc::Sender<PullRequest>,
    mounter: &Arc<dyn Mounter>,
    task: &mut Task,
) -> Result<(), DockerError> {
    let container = create_container(client, config, pull_tx, mounter, task).await?;

    // Start monitoring (logs & progress) immediately after creation
    container.start_monitoring();

    let container_id = container.id.clone();
    let twerkdir_source = container.twerkdir_source.clone();

    let mut sidecar_ids: Vec<(String, Option<String>)> = Vec::new();

    let result = async {
        // Start sidecars
        if let Some(ref sidecars) = task.sidecars {
            for sidecar in sidecars {
                let mut sidecar_task = sidecar.clone();
                sidecar_task.id = Some(TaskId::new(new_uuid())?);
                sidecar_task.mounts = task.mounts.clone();
                sidecar_task.networks = task.networks.clone();
                sidecar_task.limits = task.limits.clone();

                let sidecar_container =
                    create_container(client, config, pull_tx, mounter, &sidecar_task).await?;
                let sidecar_id = sidecar_container.id.clone();
                let sidecar_twerkdir = sidecar_container.twerkdir_source.clone();

                sidecar_container
                    .start()
                    .await
                    .map_err(|e| DockerError::ContainerStart(e.to_string()))?;

                sidecar_ids.push((sidecar_id, sidecar_twerkdir));
            }
        }

        // Start main container (includes probe if configured)
        container.start().await?;

        // Wait for completion and capture result
        task.result = Some(container.wait().await?);
        Ok(())
    }
    .await;

    // Clean up sidecars (awaited, not fire-and-forget)
    for (sc_id, sc_twerkdir) in sidecar_ids {
        remove_container_and_volume(client, &sc_id, sc_twerkdir.as_deref(), "sidecar").await;
    }

    // Clean up main container
    remove_container_and_volume(client, &container_id, twerkdir_source.as_deref(), "main").await;

    result
}
