// =============================================================================

/// Podman runtime adapter for container-based task execution.
///
/// Go parity: `podman.NewPodmanRuntime(podman.WithBroker(...), ...)`
#[derive(Debug)]
pub struct PodmanRuntimeAdapter {
    /// Whether the runtime runs in privileged mode
    privileged: bool,
    /// Whether to use host networking
    host_network: bool,
}

impl PodmanRuntimeAdapter {
    /// Creates a new Podman runtime adapter.
    #[must_use]
    pub fn new(privileged: bool, host_network: bool) -> Self {
        Self {
            privileged,
            host_network,
        }
    }
}

impl RuntimeTrait for PodmanRuntimeAdapter {
    fn run(
        &self,
        _ctx: std::sync::Arc<tokio::sync::RwLock<()>>,
        task: &mut tork::task::Task,
    ) -> tork::runtime::BoxedFuture<()> {
        let privileged = self.privileged;
        let host_network = self.host_network;
        let task_id = task.id.clone().unwrap_or_default();
        let image = task.image.clone().unwrap_or_default();

        if task_id.is_empty() {
            return Box::pin(async { Err(anyhow!("task id is required")) });
        }
        if image.is_empty() {
            return Box::pin(async { Err(anyhow!("task image is required")) });
        }

        // Clone task data for async block to avoid lifetime issues
        let cmd_clone = task.cmd.clone();
        let workdir_clone = task.workdir.clone();
        let env_clone = task.env.clone();

        Box::pin(async move {
            debug!(
                "[podman-runtime] running task {} image={} (privileged={}, host_network={})",
                task_id, image, privileged, host_network
            );

            // Build podman command
            let mut cmd = tokio::process::Command::new("podman");
            cmd.arg("run");

            if privileged {
                cmd.arg("--privileged");
            }

            if host_network {
                cmd.arg("--network").arg("host");
            }

            cmd.arg(&image);

            if let Some(ref c) = cmd_clone {
                for a in c {
                    cmd.arg(a);
                }
            }

            if let Some(ref wd) = workdir_clone {
                cmd.arg("--workdir").arg(wd);
            }

            if let Some(ref e) = env_clone {
                for (k, v) in e {
                    cmd.env(k, v);
                }
            }

            let output = cmd
                .output()
                .await
                .map_err(|e| anyhow!("podman failed: {}", e))?;

            if !output.status.success() {
                let stderr = String::from_utf8_lossy(&output.stderr);
                return Err(anyhow!("podman run failed: {}", stderr));
            }

            debug!("[podman-runtime] task {} completed successfully", task_id);
            Ok(())
        })
    }

    fn health_check(&self) -> tork::runtime::BoxedFuture<()> {
        Box::pin(async { Ok(()) })
    }
}

// =============================================================================
// Mock runtime