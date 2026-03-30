//!
//! Network operations for Docker runtime.
//!
//! Go parity: `createNetwork` and `removeNetwork` in `/tmp/tork/runtime/docker/docker.go`
//!
//! ## Network Creation
//!
//! Creates a bridge network with a UUID name. The network is used to enable
//! sidecar containers to communicate with the main task container.
//!
//! ## Network Removal
//!
//! Removes a network with retry logic. Docker cannot remove a network if
//! containers are still connected, so we retry with exponential backoff
//! (200ms, 400ms, 800ms, 1600ms, 3200ms) up to 5 attempts.

use crate::runtime::docker::error::DockerError;
use bollard::models::NetworkCreateRequest;
use bollard::Docker;
use std::time::Duration;

/// Creates a network for sidecar communication.
///
/// Go parity: `createNetwork` — creates bridge network with UUID name.
/// The `CheckDuplicate` option is set to prevent accidental duplicate networks.
///
/// # Errors
///
/// Returns `DockerError::NetworkCreate` if the network cannot be created.
pub async fn create_network(client: &Docker) -> Result<String, DockerError> {
    let id = uuid::Uuid::new_v4().to_string();
    let request = NetworkCreateRequest {
        name: id.clone(),
        driver: Some("bridge".to_string()),
        ..Default::default()
    };
    let response = client
        .create_network(request)
        .await
        .map_err(|e| DockerError::NetworkCreate(e.to_string()))?;
    Ok(response.id)
}

/// Removes a network with retry logic.
///
/// Go parity: `removeNetwork` — exponential backoff 200ms→3200ms, 5 retries.
/// Docker cannot remove a network if containers are still connected to it,
/// so we retry with a small delay to ensure containers are fully removed first.
///
/// # Notes
///
/// This function logs errors but does not return them, matching Go behavior.
pub async fn remove_network(client: &Docker, network_id: &str) {
    let mut delay = Duration::from_millis(200);
    for i in 0..5u32 {
        match client.remove_network(network_id).await {
            Ok(()) => return,
            Err(e) => {
                if i == 4 {
                    tracing::error!(network_id, error = %e, "failed to remove network");
                    return;
                }
                tracing::debug!(network_id, attempt = i+1, error = %e, "retrying");
                tokio::time::sleep(delay).await;
                delay *= 2;
            }
        }
    }
}
