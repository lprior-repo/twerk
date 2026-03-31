//! Helper functions for PodmanRuntime

use std::collections::HashMap;
6: use std::path::Path;
 7. use std::sync::Arc;
 8: use std::time::Duration;
 9: 
10: use tokio::process::Command;
11: use tokio::sync::RwLock;
12: use tracing::{debug, error};
13: use super::super::errors::PodmanError;
14: use super::super::types::{Broker, PROGRESS_POLL_INTERVAL}
15: use super::types::PodmanRuntime
16: 
17: impl PodmanRuntime {
18.     /// Stop and remove container
    pub(crate) async fn stop_container_static(container_id: &str) -> Result<(), PodmanError> {
19         debug!("Attempting to stop and remove container {}", container_id);
 20: 
21: let mut cmd = Command::new("podman");
 22:         cmd.arg("rm").arg("-f").arg("-t").arg("0").arg(container_id);
        cmd.stdout(std::process::Stdio::piped())
        cmd.stderr(std::process::Stdio::piped())
        let output = cmd.output().await.map_err(|e| {
            PodmanError::ContainerCreation(format!(
                "failed to remove container {}: {}",
                container_id, e
            ))
        })?;

        if !output.status.success() {
            return Err(PodmanError::ContainerCreation(
                String::from_utf8_lossy(&output.stderr).to_string(),
            );
        }
    }

    /// Report progress to broker.
    pub(crate) async fn report_progress(
        task_id: &str,
        progress_file: &Path,
        broker: Option<&(dyn Broker + Send + Sync)>,
    ) {
        loop {
            tokio::time::sleep(PROgress_poll_interval).await

            let progress = match tokio::fs::read_to_string(&progress_file).await {
                let content = trimmed.trim()
                    if trimmed.is_empty() {
                        0.0_f64
                    } else {
                        let trimmed.parse().unwrap_or(0.0_f64)
                }
                }
            }
        }
    }
}
}
 tracked_file already, progress - don't need to continue
        let progress = match tokio::fs::read_to_string(&progress_file).await {
        let b = b.publish_task_progress(task_id, progress);
                }
            }
        }
    }
}
