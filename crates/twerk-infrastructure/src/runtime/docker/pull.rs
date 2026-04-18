//! Image pull operations for Docker runtime.

use crate::runtime::docker::auth::{config_path, Config as AuthConfig};
use crate::runtime::docker::error::DockerError;
use crate::runtime::docker::reference::parse as parse_reference;
use bollard::auth::DockerCredentials;
use bollard::models::ContainerCreateBody;
use bollard::query_parameters::{
    CreateContainerOptions, CreateImageOptions, RemoveContainerOptions,
};
use bollard::Docker;
use futures_util::StreamExt;
use twerk_core::task::Registry;

/// Checks if a Docker image exists locally.
///
/// # Errors
///
/// Returns `DockerError` if listing images fails.
pub async fn image_exists_locally(client: &Docker, name: &str) -> Result<bool, DockerError> {
    use bollard::query_parameters::ListImagesOptions;
    let options = ListImagesOptions {
        all: true,
        ..Default::default()
    };
    let image_list = client
        .list_images(Some(options))
        .await
        .map_err(|e| DockerError::image_pull(&e))?;
    Ok(image_list
        .iter()
        .any(|img| img.repo_tags.iter().any(|tag| tag == name)))
}

/// Verifies a Docker image by running a container.
///
/// # Errors
///
/// Returns `DockerError` if image verification fails.
pub async fn verify_image(client: &Docker, image: &str) -> Result<(), DockerError> {
    let config = ContainerCreateBody {
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
        .map_err(|e| DockerError::ImageVerifyFailed(format!("{image}: {e}")))?;
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
///
/// # Errors
///
/// Returns `DockerError` if credentials cannot be retrieved.
#[allow(clippy::unused_async)]
pub async fn get_registry_credentials(
    config: &crate::runtime::docker::config::DockerConfig,
    image: &str,
) -> Result<Option<DockerCredentials>, DockerError> {
    let reference = parse_reference(image).map_err(|e| DockerError::image_pull(&e))?;
    if reference.domain.is_empty() {
        return Ok(None);
    }
    let auth_config = match (&config.config_file, &config.config_path) {
        (Some(path), _) | (_, Some(path)) => {
            AuthConfig::load_from_path(path).map_err(|e| DockerError::image_pull(&e))?
        }
        (None, None) => {
            let path = config_path().map_err(|e| DockerError::image_pull(&e))?;
            AuthConfig::load_from_path(&path).map_err(|e| DockerError::image_pull(&e))?
        }
    };
    let (username, password) = auth_config
        .get_credentials(&reference.domain)
        .map_err(|e| DockerError::image_pull(&e))?;
    if username.is_empty() && password.is_empty() {
        return Ok(None);
    }
    Ok(Some(DockerCredentials {
        username: Some(username),
        password: Some(password),
        ..Default::default()
    }))
}

/// Pulls a Docker image from a registry.
///
/// # Errors
///
/// Returns `DockerError` if the image pull fails.
pub async fn pull_image<S: std::hash::BuildHasher>(
    client: &Docker,
    config: &crate::runtime::docker::config::DockerConfig,
    images: &std::sync::Arc<
        tokio::sync::RwLock<std::collections::HashMap<String, std::time::Instant, S>>,
    >,
    image: &str,
    _registry: Option<&Registry>,
) -> Result<(), DockerError> {
    if check_cache(images, image).await.is_some() {
        tracing::debug!(image, "image found in cache");
        return Ok(());
    }

    if image_exists_locally(client, image).await? {
        tracing::debug!(image, "image found locally");
        update_cache(images, image).await;
        return Ok(());
    }

    pull_image_from_registry(client, config, image).await?;

    if config.image_verify {
        verify_and_cleanup_on_failure(client, image).await?;
    }

    update_cache(images, image).await;
    Ok(())
}

async fn check_cache<S: std::hash::BuildHasher>(
    images: &std::sync::Arc<
        tokio::sync::RwLock<std::collections::HashMap<String, std::time::Instant, S>>,
    >,
    image: &str,
) -> Option<std::time::Instant> {
    let cache = images.read().await;
    cache.get(image).copied()
}

async fn update_cache<S: std::hash::BuildHasher>(
    images: &std::sync::Arc<
        tokio::sync::RwLock<std::collections::HashMap<String, std::time::Instant, S>>,
    >,
    image: &str,
) {
    let mut cache = images.write().await;
    cache.insert(image.to_string(), std::time::Instant::now());
}

async fn pull_image_from_registry(
    client: &Docker,
    config: &crate::runtime::docker::config::DockerConfig,
    image: &str,
) -> Result<(), DockerError> {
    tracing::debug!(image, "pulling image");
    let credentials = get_registry_credentials(config, image).await?;
    let options = CreateImageOptions {
        from_image: Some(image.to_string()),
        ..Default::default()
    };
    let mut stream = client.create_image(Some(options), None, credentials);
    while let Some(result) = stream.next().await {
        if let Err(e) = result {
            return Err(DockerError::image_pull(&e));
        }
    }
    Ok(())
}

async fn verify_and_cleanup_on_failure(client: &Docker, image: &str) -> Result<(), DockerError> {
    if let Err(_e) = verify_image(client, image).await {
        use bollard::query_parameters::RemoveImageOptions;
        let _ = client
            .remove_image(image, None::<RemoveImageOptions>, None::<DockerCredentials>)
            .await;
        return Err(DockerError::CorruptedImage(image.to_string()));
    }
    Ok(())
}
