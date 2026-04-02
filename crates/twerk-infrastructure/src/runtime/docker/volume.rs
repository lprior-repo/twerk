//! Volume mount support following functional-rust conventions.
//!
//! # Architecture
//!
//! - **Data**: `VolumeMounter` holds Docker client
//! - **Calc**: Pure volume name generation
//! - **Actions**: Docker API calls at boundary

use std::sync::Arc;
use tokio::sync::RwLock;

use bollard::models::VolumeCreateRequest;
use bollard::query_parameters::{ListVolumesOptions, RemoveVolumeOptions};
use thiserror::Error;
use twerk_core::mount::Mount;
use twerk_core::uuid::new_uuid;

/// Errors from volume mount operations.
#[derive(Debug, Error)]
pub enum VolumeMounterError {
    #[error("failed to create Docker client: {0}")]
    ClientCreate(String),

    #[error("failed to create volume: {0}")]
    VolumeCreate(String),

    #[error("failed to list volumes: {0}")]
    VolumeList(String),

    #[error("failed to remove volume: {0}")]
    VolumeRemove(String),

    #[error("unknown volume: {0}")]
    UnknownVolume(String),
}

/// Volume mounter for Docker volumes.
#[derive(Debug)]
pub struct VolumeMounter {
    /// Docker client.
    client: Arc<RwLock<bollard::Docker>>,
}

impl VolumeMounter {
    /// Creates a new volume mounter.
    ///
    /// # Errors
    ///
    /// Returns `VolumeMounterError` if the Docker client cannot be created.
    #[allow(clippy::unused_async)]
    pub async fn new() -> Result<Self, VolumeMounterError> {
        let client = bollard::Docker::connect_with_local_defaults()
            .map_err(|e| VolumeMounterError::ClientCreate(e.to_string()))?;

        Ok(Self {
            client: Arc::new(RwLock::new(client)),
        })
    }

    /// Creates a new volume mounter with an existing client.
    #[must_use]
    pub fn with_client(client: bollard::Docker) -> Self {
        Self {
            client: Arc::new(RwLock::new(client)),
        }
    }

    /// Mounts a volume mount.
    ///
    /// Generates a unique volume name and creates the volume in Docker.
    /// Returns a new Mount with the source populated (functional style).
    ///
    /// # Errors
    ///
    /// Returns `VolumeMounterError` if the volume cannot be created.
    pub async fn mount(&self, mnt: &Mount) -> Result<Mount, VolumeMounterError> {
        // Generate a unique name for the volume
        let name = new_uuid();

        // Get the Docker client
        let client = self.client.read().await;

        // Create the volume (we don't need the response since we use the generated name)
        let _volume = client
            .create_volume(VolumeCreateRequest {
                name: Some(name.clone()),
                driver: Some("local".to_string()),
                driver_opts: mnt.opts.clone(),
                labels: Some(std::collections::HashMap::new()),
                cluster_volume_spec: None,
            })
            .await
            .map_err(|e| VolumeMounterError::VolumeCreate(e.to_string()))?;

        // Return new Mount with source populated (matching Go behavior:
        // mn.Source = uuid.NewUUID())
        Ok(Mount {
            id: mnt.id.clone(),
            mount_type: mnt.mount_type.clone(),
            source: Some(name),
            target: mnt.target.clone(),
            opts: mnt.opts.clone(),
        })
    }

    /// Unmounts a volume mount.
    ///
    /// Removes the volume from Docker.
    ///
    /// # Errors
    ///
    /// Returns `VolumeMounterError` if the volume cannot be removed.
    pub async fn unmount(&self, mnt: &Mount) -> Result<(), VolumeMounterError> {
        let source = mnt
            .source
            .as_ref()
            .ok_or_else(|| VolumeMounterError::UnknownVolume("no source".to_string()))?;

        let client = self.client.read().await;

        // List volumes to verify it exists
        let volumes = client
            .list_volumes(Some(ListVolumesOptions {
                filters: Some(
                    vec![("name".to_string(), vec![source.clone()])]
                        .into_iter()
                        .collect(),
                ),
            }))
            .await
            .map_err(|e| VolumeMounterError::VolumeList(e.to_string()))?;

        if volumes.volumes.as_ref().is_none_or(Vec::is_empty) {
            return Err(VolumeMounterError::UnknownVolume(source.clone()));
        }

        // Remove the volume
        client
            .remove_volume(source, Some(RemoveVolumeOptions { force: true }))
            .await
            .map_err(|e| VolumeMounterError::VolumeRemove(e.to_string()))?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use twerk_core::mount::mount_type;

    // Note: These tests require a running Docker daemon
    // They are marked as ignored by default

    #[tokio::test]
    async fn test_create_volume() {
        let mounter = VolumeMounter::new().await.expect("should create mounter");

        let mnt = Mount::new(mount_type::VOLUME, "/somevol");

        let result = mounter.mount(&mnt).await;
        assert!(result.is_ok());
        let mounted = result.expect("should have mounted volume");
        assert!(mounted.source.is_some());
    }
}
