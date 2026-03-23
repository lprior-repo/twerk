// =============================================================================

/// Docker runtime for container-based task execution.
///
/// Go parity: `docker.NewDockerRuntime(docker.WithMounter(mounter), ...)`
#[derive(Debug)]
pub struct DockerRuntimeAdapter {
    /// Whether the runtime runs in privileged mode
    privileged: bool,
}

impl DockerRuntimeAdapter {
    /// Creates a new Docker runtime adapter.
    #[must_use]
    pub fn new(privileged: bool) -> Self {
        Self { privileged }
    }
}

impl RuntimeTrait for DockerRuntimeAdapter {
    fn run(
        &self,
        _ctx: std::sync::Arc<tokio::sync::RwLock<()>>,
        task: &mut tork::task::Task,
    ) -> tork::runtime::BoxedFuture<()> {
        let privileged = self.privileged;
        let task_id = task.id.clone().unwrap_or_default();

        if task_id.is_empty() {
            return Box::pin(async { Err(anyhow!("task id is required")) });
        }

        // Get image (required for docker)
        let image = task.image.clone().unwrap_or_default();
        if image.is_empty() {
            return Box::pin(async { Err(anyhow!("task image is required for docker runtime")) });
        }

        // Get command
        let cmd = task.cmd.clone().unwrap_or_default();
        let entrypoint = task.entrypoint.clone().unwrap_or_default();
        let run_script = task.run.clone().unwrap_or_default();

        // Build environment
        let env_vars: Vec<String> = task
            .env
            .as_ref()
            .map(|e| {
                e.iter()
                    .map(|(k, v)| format!("{}={}", k, v))
                    .collect()
            })
            .unwrap_or_default();

        // Get working directory
        let workdir = task.workdir.clone();

        Box::pin(async move {
            debug!(
                "[docker-runtime] running task {} with image {} (privileged={})",
                task_id, image, privileged
            );

            // Connect to Docker
            let docker = Docker::connect_with_local_defaults()
                .map_err(|e| anyhow!("failed to connect to Docker: {}", e))?;

            // Pull image if needed
            let image_exists = docker
                .inspect_image(&image)
                .await
                .is_ok();

            if !image_exists {
                debug!("[docker-runtime] pulling image {}", image);
                let options = CreateImageOptions {
                    from_image: image.clone(),
                    ..Default::default()
                };
                let mut stream = docker.create_image(Some(options), None, None);
                while let Some(result) = stream.next().await {
                    if let Err(e) = result {
                        debug!("[docker-runtime] warning: pull error: {}", e);
                    }
                }
            }

            // Build container config
            let mut cmd_args: Vec<&String> = Vec::new();
            if !entrypoint.is_empty() {
                cmd_args.extend(entrypoint.iter());
            }
            if !run_script.is_empty() {
                // Use run script as entrypoint command
                cmd_args.push(&run_script);
            } else if !cmd.is_empty() {
                cmd_args.extend(cmd.iter());
            }

            // Build container config with all required fields
            let config = ContainerConfig::<String> {
                image: Some(image.clone()),
                cmd: if cmd_args.is_empty() {
                    None
                } else {
                    Some(cmd_args.into_iter().map(|s| s.clone()).collect())
                },
                env: if env_vars.is_empty() {
                    None
                } else {
                    Some(env_vars)
                },
                working_dir: workdir,
                host_config: Some(bollard::secret::HostConfig {
                    privileged: Some(privileged),
                    ..Default::default()
                }),
                ..Default::default()
            };

            // Create container
            let container_id = docker
                .create_container(
                    None::<CreateContainerOptions<String>>,
                    config,
                )
                .await
                .map_err(|e| anyhow!("failed to create container: {}", e))?
                .id;

            debug!("[docker-runtime] created container {}", container_id);

            // Start container
            docker
                .start_container::<String>(&container_id, None)
                .await
                .map_err(|e| anyhow!("failed to start container: {}", e))?;

            // Wait for completion using a simple polling approach
            let mut exit_code = None;
            let max_attempts = 60; // 60 seconds timeout
            for _ in 0..max_attempts {
                tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;
                
                // Check if container is still running
                let info = docker
                    .inspect_container(&container_id, None::<bollard::container::InspectContainerOptions>)
                    .await;
                
                if let Ok(info) = info {
                    if let Some(state) = info.state {
                        if !state.running.unwrap_or(false) {
                            exit_code = state.exit_code;
                            break;
                        }
                    }
                }
            }

            let exit_code = exit_code.unwrap_or(1);

            // Log output
            if exit_code != 0 {
                debug!("[docker-runtime] container exited with code {}", exit_code);
            } else {
                debug!("[docker-runtime] container completed successfully");
            }

            // Cleanup - remove container
            let remove_options = RemoveContainerOptions {
                force: true,
                ..Default::default()
            };
            let _ = docker.remove_container(&container_id, Some(remove_options)).await;

            if exit_code != 0 {
                return Err(anyhow!(
                    "container exited with non-zero status: {}",
                    exit_code
                ));
            }

            debug!("[docker-runtime] task {} completed successfully", task_id);
            Ok(())
        })
    }

    fn health_check(&self) -> tork::runtime::BoxedFuture<()> {
        Box::pin(async {
            match Docker::connect_with_local_defaults() {
                Ok(docker) => {
                    docker.ping().await?;
                    Ok(())
                }
                Err(e) => Err(anyhow!("docker health check failed: {}", e)),
            }
        })
    }
}

// =============================================================================