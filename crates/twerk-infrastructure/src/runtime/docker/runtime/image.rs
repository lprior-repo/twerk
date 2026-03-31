//! Image pull, verification, and pruning operations.

use std::sync::Arc;

use bollard::Docker;
use dashmap::DashMap;
use futures_util::StreamExt;
use tokio::sync::RwLock;
use twerk_core::task::Registry;

use bollard::query_parameters::{
    CreateContainerOptions, CreateImageOptions, ListImagesOptions, RemoveContainerOptions,
    RemoveImageOptions,
};

use crate::runtime::docker::auth::{config_path, Config as AuthConfig};
use crate::runtime::docker::config::DockerConfig;
use crate::runtime::docker::error::DockerError;
use crate::runtime::docker::reference::parse as parse_reference;

use super::types::PullRequest;
use std::time::{Duration, Instant};

/// Pulls an image via the serialized pull queue.
pub(super) async fn pull_image(
    pull_tx: &tokio::sync::mpsc::Sender<PullRequest>,
    image: &str,
    registry: Option<&Registry>,
) -> Result<(), DockerError> {
    let (result_tx, result_rx) = tokio::sync::oneshot::channel();
    let request = PullRequest {
        image: image.to_string(),
        registry: registry.cloned(),
        logger: Box::new(std::io::sink()),
        result_tx,
    };
    pull_tx
        .send(request)
        .await
        .map_err(|_| DockerError::ImagePull("pull queue closed".to_string()))?;
    result_rx
        .await
        .map_err(|_| DockerError::ImagePull("pull worker died".to_string()))?
}

/// Internal pull implementation.
pub(super) async fn do_pull_request(
    client: &Docker,
    images: &Arc<DashMap<String, Instant>>,
    config: &DockerConfig,
    image: &str,
    #[allow(unused_variables)] registry: Option<&Registry>,
) -> Result<(), DockerError> {
    // Check cache (respecting TTL)
    if let Some(ts) = images.get(image) {
        if Instant::now().duration_since(*ts) <= config.image_ttl {
            return Ok(());
        }
    }

    // Check local
    let exists = image_exists_locally(client, image).await?;
    if !exists {
        let credentials = get_registry_credentials(config, image).await?;

        let options = CreateImageOptions {
            from_image: Some(image.to_string()),
            ..Default::default()
        };
        let mut stream = client.create_image(Some(options), None, credentials);
        while let Some(result) = stream.next().await {
            match result {
                Ok(_) => {}
                Err(e) => return Err(DockerError::ImagePull(e.to_string())),
            }
        }
    }

    // Verify if enabled (Go parity: verifyImage)
    if config.image_verify {
        if let Err(_e) = verify_image(client, image).await {
            let _ = client
                .remove_image(
                    image,
                    None::<RemoveImageOptions>,
                    None::<bollard::auth::DockerCredentials>,
                )
                .await;
            return Err(DockerError::CorruptedImage(image.to_string()));
        }
    }

    // Cache
    images.insert(image.to_string(), Instant::now());

    Ok(())
}

/// Checks if an image exists locally.
pub(super) async fn image_exists_locally(client: &Docker, name: &str) -> Result<bool, DockerError> {
    let options = ListImagesOptions {
        all: true,
        ..Default::default()
    };
    let image_list = client
        .list_images(Some(options))
        .await
        .map_err(|e| DockerError::ImagePull(e.to_string()))?;
    Ok(image_list
        .iter()
        .any(|img| img.repo_tags.iter().any(|tag| tag == name)))
}

/// Verifies image integrity by creating a test container and removing it.
///
/// Go parity: `verifyImage` — creates container with `cmd: ["true"]`.
pub(super) async fn verify_image(client: &Docker, image: &str) -> Result<(), DockerError> {
    let config = bollard::models::ContainerCreateBody {
        image: Some(image.to_string()),
        cmd: Some(vec!["true".to_string()]),
        ..Default::default()
    };
    let response = client
        .create_container(
            Some(CreateContainerOptions {
                name: None,
                platform: String::new(),
            }),
            config,
        )
        .await
        .map_err(|e| DockerError::ImageVerifyFailed(format!("{}: {}", image, e)))?;

    // Clean up test container
    let _ = client
        .remove_container(
            &response.id,
            Some(RemoveContainerOptions {
                force: true,
                ..Default::default()
            }),
        )
        .await;

    Ok(())
}

/// Gets registry credentials for an image.
pub(super) async fn get_registry_credentials(
    config: &DockerConfig,
    image: &str,
) -> Result<Option<bollard::auth::DockerCredentials>, DockerError> {
    let reference =
        parse_reference(image).map_err(|e| DockerError::ImagePull(e.to_string()))?;

    if reference.domain.is_empty() {
        return Ok(None);
    }

    // Load auth config: config_file takes priority, then config_path, then default path
    let auth_config = match (&config.config_file, &config.config_path) {
        (Some(path), _) | (_, Some(path)) => AuthConfig::load_from_path(path)
            .map_err(|e| DockerError::ImagePull(e.to_string()))?,
        (None, None) => {
            let path = config_path().map_err(|e| DockerError::ImagePull(e.to_string()))?;
            AuthConfig::load_from_path(&path).map_err(|e| DockerError::ImagePull(e.to_string()))?
        }
    };

    let (username, password) = auth_config
        .get_credentials(&reference.domain)
        .map_err(|e| DockerError::ImagePull(e.to_string()))?;

    if username.is_empty() && password.is_empty() {
        return Ok(None);
    }

    Ok(Some(bollard::auth::DockerCredentials {
        username: Some(username),
        password: Some(password),
        ..Default::default()
    }))
}

/// Prunes old images. Go parity: only prunes when no tasks running.
pub(super) async fn prune_images(
    client: &Docker,
    images: &Arc<DashMap<String, Instant>>,
    tasks: &Arc<RwLock<usize>>,
    ttl: Duration,
) {
    if *tasks.read().await > 0 {
        return;
    }

    let now = Instant::now();
    let to_remove: Vec<String> = images
        .iter()
        .filter(|entry| now.duration_since(*entry.value()) > ttl)
        .map(|entry| entry.key().clone())
        .collect();

    for image in to_remove {
        let _ = client
            .remove_image(
                &image,
                None::<RemoveImageOptions>,
                None::<bollard::auth::DockerCredentials>,
            )
            .await;
        images.remove(&image);
        tracing::debug!(image = %image, "pruned image");
    }
}
