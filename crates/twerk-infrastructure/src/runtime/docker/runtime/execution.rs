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
    if let Some(ref mounts) = task.mounts {
        for mnt in mounts {
            let mut mnt = mnt.clone();
            mnt.id = Some(new_uuid());
            if let Err(e) = mounter.mount(&mnt).await {
                return Err(DockerError::Mount(e));
            }
            mounted_mounts.push(mnt);
        }
    }
    task.mounts = Some(mounted_mounts.clone());

    // Execute pre-tasks
    let pre_tasks: Vec<Task> = if let Some(ref pre) = task.pre {
        pre.clone()
    } else {
        Vec::new()
    };
    for mut pre_task in pre_tasks {
        pre_task.id = Some(TaskId::new(new_uuid())?);
        pre_task.mounts = Some(mounted_mounts.clone());
        pre_task.networks = task.networks.clone();
        pre_task.limits = task.limits.clone();
        run_task(&client, &config, &pull_tx, &mounter, &mut pre_task).await?;
    }

    // Run the actual task
    run_task(&client, &config, &pull_tx, &mounter, task).await?;

    // Execute post-tasks
    let post_tasks: Vec<Task> = if let Some(ref post) = task.post {
        post.clone()
    } else {
        Vec::new()
    };
    for mut post_task in post_tasks {
        post_task.id = Some(TaskId::new(new_uuid())?);
        post_task.mounts = Some(mounted_mounts.clone());
        post_task.networks = task.networks.clone();
        post_task.limits = task.limits.clone();
        run_task(&client, &config, &pull_tx, &mounter, &mut post_task).await?;
    }

    // Clean up mounts
    for mnt in mounted_mounts {
        if let Err(e) = mounter.unmount(&mnt).await {
            tracing::error!(error = %e, mount = ?mnt, "error unmounting");
        }
    }

    // Clean up network
    if let Some(ref id) = network_id {
        network::remove_network(&client, id).await;
    }

    Ok(())
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

                // Defer sidecar removal
                let sc = client.clone();
                tokio::spawn(async move {
                    let remove_container = sc.remove_container(
                        &sidecar_id,
                        Some(RemoveContainerOptions {
                            force: true,
                            ..Default::default()
                        }),
                    );
                    if let Some(source) = sidecar_twerkdir {
                        let remove_volume = sc.remove_volume(&source, None::<RemoveVolumeOptions>);
                        let (res_c, res_v) = tokio::join!(remove_container, remove_volume);
                        if let Err(e) = res_c {
                            tracing::warn!(error = %e, container_id = %sidecar_id, "failed to remove sidecar container");
                        }
                        if let Err(e) = res_v {
                            tracing::warn!(error = %e, volume = %source, "failed to remove sidecar volume");
                        }
                    } else if let Err(e) = remove_container.await {
                        tracing::warn!(error = %e, container_id = %sidecar_id, "failed to remove sidecar container");
                    }
                });
            }
        }

        // Start main container (includes probe if configured)
        container.start().await?;

        // Wait for completion and capture result
        task.result = Some(container.wait().await?);
        Ok(())
    }
    .await;

    // Clean up main container
    let remove_container = client.remove_container(
        &container_id,
        Some(RemoveContainerOptions {
            force: true,
            ..Default::default()
        }),
    );
    if let Some(source) = twerkdir_source {
        let remove_volume = client.remove_volume(&source, None::<RemoveVolumeOptions>);
        let (res_c, res_v) = tokio::join!(remove_container, remove_volume);
        if let Err(e) = res_c {
            tracing::warn!(error = %e, container_id = %container_id, "failed to remove main container");
        }
        if let Err(e) = res_v {
            tracing::warn!(error = %e, volume = %source, "failed to remove main volume");
        }
    } else if let Err(e) = remove_container.await {
        tracing::warn!(error = %e, container_id = %container_id, "failed to remove main container");
    }

    result
}
