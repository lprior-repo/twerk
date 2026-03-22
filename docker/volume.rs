//! Volume mount support following functional-rust conventions.
//!
//! # Architecture
//!
//! - **Data**: `VolumeMounter` holds Docker client
//! - **Calc**: Pure volume name generation
//! - **Actions**: Docker API calls at boundary

use std::sync::Arc;
use tokio::sync::RwLock;

use crate::docker::tork::Mount;
use thiserror::Error;

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
    ///
    /// # Errors
    ///
    /// Returns `VolumeMounterError` if the volume cannot be created.
    pub async fn mount(&self, mnt: &mut Mount) -> Result<(), VolumeMounterError> {
        // Generate a unique name for the volume
        let name = uuid::Uuid::new_v4().to_string();

        // Get the Docker client
        let client = self.client.read().await;

        // Create the volume
        let volume = client.create_volume(
            bollard::volume::CreateVolumeOptions {
                name: name.clone(),
                driver: "local".to_string(),
                driver_opts: mnt.opts.iter()
                    .map(|(k, v)| (k.clone(), v.clone()))
                    .collect::<std::collections::HashMap<String, String>>(),
                labels: std::collections::HashMap::new(),
            }
        ).await
            .map_err(|e| VolumeMounterError::VolumeCreate(e.to_string()))?;

        // Populate mnt.source with the generated volume path (matching Go behavior)
        mnt.source = Some(volume.mountpoint);

        Ok(())
    }

    /// Unmounts a volume mount.
    ///
    /// Removes the volume from Docker.
    ///
    /// # Errors
    ///
    /// Returns `VolumeMounterError` if the volume cannot be removed.
    pub async fn unmount(&self, mnt: &Mount) -> Result<(), VolumeMounterError> {
        let source = mnt.source.as_ref()
            .ok_or_else(|| VolumeMounterError::UnknownVolume("no source".to_string()))?;

        let client = self.client.read().await;

        // List volumes to verify it exists
        let volumes = client.list_volumes(
            Some(bollard::volume::ListVolumesOptions {
                filters: vec![("name".to_string(), vec![source.clone()])]
                    .into_iter()
                    .collect(),
            })
        ).await
            .map_err(|e| VolumeMounterError::VolumeList(e.to_string()))?;

        if volumes.volumes.as_ref().is_none_or(|v| v.is_empty()) {
            return Err(VolumeMounterError::UnknownVolume(source.clone()));
        }

        // Remove the volume
        client.remove_volume(
            source,
            Some(bollard::volume::RemoveVolumeOptions {
                force: true,
            })
        ).await
            .map_err(|e| VolumeMounterError::VolumeRemove(e.to_string()))?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::docker::tork::mount_type;

    // Note: These tests require a running Docker daemon
    // They are marked as ignored by default

    #[tokio::test]
    #[ignore]
    async fn test_create_volume() {
        let mounter = VolumeMounter::new().await.expect("should create mounter");

        let mut mnt = Mount::new(mount_type::VOLUME, "/somevol");

        let result = mounter.mount(&mut mnt).await;
        assert!(result.is_ok());
        assert!(mnt.source.is_some());
    }
}
