//! Probe support for Podman runtime

use std::time::Duration;

use tokio::process::Command;
use tracing::debug;

use super::errors::PodmanError;
use super::types::{PodmanRuntime, Probe};

#[allow(dead_code)]
impl PodmanRuntime {
    /// Get the host port for a container port mapping
    pub(crate) async fn get_host_port(container_id: &str, container_port: i64) -> Result<u16, PodmanError> {
        let port_format = format!(
            "{{{{(index (index .NetworkSettings.Ports \"{}/tcp\") 0).HostPort}}}}",
            container_port
        );
        let mut cmd = Command::new("podman");
        cmd.arg("inspect")
            .arg("--format")
            .arg(&port_format)
            .arg(container_id);

        let output = cmd
            .output()
            .await
            .map_err(|e| PodmanError::ContainerInspect(e.to_string()))?;

        let port_str = String::from_utf8_lossy(&output.stdout).trim().to_string();

        port_str
            .parse::<u16>()
            .map_err(|e| PodmanError::ProbeFailed(format!("failed to parse host port: {}", e)))
    }

    /// Probe a container using HTTP health check
    pub(crate) async fn probe_container(&self, host_port: &u16, probe: &Probe) -> Result<(), PodmanError> {
        let path = if probe.path.is_empty() {
            "/".to_string()
        } else {
            probe.path.clone()
        };

        let timeout_str = if probe.timeout.is_empty() {
            "1m".to_string()
        } else {
            probe.timeout.clone()
        };

        let timeout = Self::parse_duration(&timeout_str)
            .map_err(|e| PodmanError::ProbeTimeout(format!("invalid probe timeout: {}", e)))?;

        let url = format!("http://127.0.0.1:{}{}", host_port, path);
        debug!("probing container at {}", url);

        let probe_start = tokio::time::Instant::now();
        let mut interval = tokio::time::interval(Duration::from_secs(1));

        loop {
            interval.tick().await;

            if probe_start.elapsed() > timeout {
                return Err(PodmanError::ProbeTimeout(timeout_str));
            }

            match Self::http_get(&url).await {
                Ok(true) => {
                    debug!("probe succeeded for {}", url);
                    return Ok(());
                }
                Ok(false) => {
                    debug!("probe returned non-200, retrying...");
                    continue;
                }
                Err(e) => {
                    debug!("probe failed: {}, retrying...", e);
                    continue;
                }
            }
        }
    }

    /// Perform HTTP GET request to check container health
    async fn http_get(url: &str) -> Result<bool, String> {
        let mut cmd = Command::new("curl");
        cmd.arg("-s")
            .arg("-o")
            .arg("/dev/null")
            .arg("-w")
            .arg("%{http_code}")
            .arg("--connect-timeout")
            .arg("3")
            .arg("--max-time")
            .arg("3")
            .arg(url);

        let output = cmd
            .output()
            .await
            .map_err(|e| format!("curl failed: {}", e))?;

        let status_code = String::from_utf8_lossy(&output.stdout).trim().to_string();
        Ok(status_code == "200")
    }
}
