//! Network operations for Docker runtime.

use std::time::Duration;
use bollard::network::CreateNetworkOptions;
use bollard::Docker;
use crate::runtime::docker::error::DockerError;

pub async fn create_network(client: &Docker) -> Result<String, DockerError> {
    let id = uuid::Uuid::new_v4().to_string();
    let options = CreateNetworkOptions {
        name: id.clone(),
        driver: "bridge".to_string(),
        check_duplicate: true,
        ..Default::default()
    };
    let response = client.create_network(options).await
        .map_err(|e| DockerError::NetworkCreate(e.to_string()))?;
    Ok(response.id)
}

pub async fn remove_network(client: &Docker, network_id: &str) {
    use tokio::time::sleep;
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
                sleep(delay).await;
                delay *= 2;
            }
        }
    }
}
