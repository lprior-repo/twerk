//! Container health check probing.
//!
//! Provides HTTP health probe functionality for containers with configurable
//! timeout, port, and path.

use crate::runtime::docker::config::{DEFAULT_PROBE_PATH, DEFAULT_PROBE_TIMEOUT};
use crate::runtime::docker::error::DockerError;
use crate::runtime::docker::helpers::{parse_go_duration, port_key};
use bollard::Docker;
use std::time::Duration;
use tokio::time::sleep;

/// Probes a container for readiness via HTTP health check.
///
/// # Errors
/// - `DockerError::ProbeError` if port mapping cannot be found or HTTP client fails
/// - `DockerError::ProbeTimeout` if probe doesn't succeed within timeout
pub async fn probe_container(
    client: &Docker,
    container_id: &str,
    port: u16,
    path: Option<&str>,
    timeout_str: Option<&str>,
) -> Result<(), DockerError> {
    let path = path.map_or(DEFAULT_PROBE_PATH, |s| s);
    let timeout_str = timeout_str.map_or(DEFAULT_PROBE_TIMEOUT, |s| s);

    let timeout = parse_go_duration(timeout_str)
        .map_err(|e| DockerError::ProbeTimeout(format!("invalid timeout: {e}")))?;

    let inspect = client
        .inspect_container(container_id, None)
        .await
        .map_err(|e| DockerError::ContainerInspect(format!("{container_id}: {e}")))?;

    let port_key = port_key(u64::from(port));
    let host_port = inspect
        .network_settings
        .as_ref()
        .and_then(|ns| ns.ports.as_ref())
        .and_then(|ports| ports.get(&port_key))
        .and_then(|opt| opt.as_ref())
        .and_then(|bindings| bindings.first())
        .and_then(|b| b.host_port.as_ref())
        .ok_or_else(|| DockerError::ProbeError(format!("no port found for {container_id}")))?;

    let probe_url = format!("http://localhost:{host_port}{path}");

    let http_client = reqwest::Client::builder()
        .timeout(Duration::from_secs(3))
        .connect_timeout(Duration::from_secs(3))
        .build()
        .map_err(|e| DockerError::ProbeError(format!("HTTP client: {e}")))?;

    let deadline = tokio::time::Instant::now() + timeout;

    loop {
        if tokio::time::Instant::now() >= deadline {
            return Err(DockerError::ProbeTimeout(timeout_str.to_string()));
        }

        match http_client.get(&probe_url).send().await {
            Ok(resp) if resp.status().as_u16() == 200 => return Ok(()),
            Ok(resp) => {
                tracing::debug!(
                    container_id = %container_id,
                    status = resp.status().as_u16(),
                    "probe non-200"
                );
            }
            Err(e) => {
                tracing::debug!(container_id = %container_id, error = %e, "probe failed");
            }
        }

        sleep(Duration::from_secs(1)).await;
    }
}

/// Probes a container if a probe is configured.
///
/// Returns `Ok(())` immediately if probe is `None`.
///
/// # Errors
///
/// Returns `DockerError` if the probe fails.
pub async fn probe_if_configured(
    client: &Docker,
    container_id: &str,
    probe: Option<&twerk_core::task::Probe>,
) -> Result<(), DockerError> {
    let Some(probe) = probe else {
        return Ok(());
    };

    probe_container(
        client,
        container_id,
        u16::try_from(probe.port).unwrap_or(0),
        probe.path.as_deref(),
        probe.timeout.as_deref(),
    )
    .await
}
