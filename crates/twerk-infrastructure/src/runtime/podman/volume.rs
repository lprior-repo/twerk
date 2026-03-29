//! Volume mounter for podman
//!
//! Creates temporary directories for volume mounts with world-writable
//! permissions, matching the Go implementation.

use std::path::PathBuf;

#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;

use super::errors::PodmanError;
use super::types::Mount;

/// VolumeMounter creates temporary directories for volume mounts
#[derive(Debug, Default)]
pub struct VolumeMounter {
    _priv: (),
}

impl VolumeMounter {
    /// Creates a new VolumeMounter
    #[must_use]
    pub fn new() -> Self {
        Self { _priv: () }
    }
}

impl super::types::Mounter for VolumeMounter {
    fn mount(&self, mount: &mut Mount) -> Result<(), PodmanError> {
        let temp_dir =
            tempfile::tempdir().map_err(|e| PodmanError::WorkdirCreation(e.to_string()))?;

        // Set permissions to 0777 (world-writable) matching Go behavior
        let path = temp_dir.path();
        std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o777))
            .map_err(|e| PodmanError::WorkdirCreation(e.to_string()))?;

        // Keep the directory alive beyond this scope
        let kept = temp_dir.keep();
        let mount_source = kept.to_string_lossy().to_string();

        mount.source = mount_source.clone();

        tracing::debug!("Created volume at {}", mount_source);

        Ok(())
    }

    fn unmount(&self, mount: &Mount) -> Result<(), PodmanError> {
        if !mount.source.is_empty() {
            let path = PathBuf::from(&mount.source);
            if path.exists() {
                std::fs::remove_dir_all(&path)
                    .map_err(|e| PodmanError::WorkdirCreation(e.to_string()))?;
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::runtime::podman::types::{MountType, Mounter};

    /// Verifies mount creates a directory, it exists, and unmount removes it.
    #[test]
    fn volume_mount_creates_directory_when_mounted() {
        let vm = VolumeMounter::new();
        let mut mount = Mount {
            id: "test".to_string(),
            mount_type: MountType::Volume,
            source: String::new(),
            target: "/somevol".to_string(),
            opts: None,
        };

        vm.mount(&mut mount).expect("mount should succeed");
        assert_eq!("/somevol", mount.target);
        assert!(!mount.source.is_empty(), "source should be populated");

        // Verify the directory exists
        let metadata = std::fs::metadata(&mount.source);
        assert!(metadata.is_ok(), "mounted directory should exist");

        // Verify the directory is world-writable
        #[cfg(unix)]
        {
            let mode = metadata.expect("metadata ok").permissions().mode();
            assert_eq!(mode & 0o777, 0o777);
        }

        // Unmount should remove the directory
        vm.unmount(&mount).expect("unmount should succeed");

        // Verify directory no longer exists
        let metadata = std::fs::metadata(&mount.source);
        assert!(
            metadata.is_err(),
            "directory should be removed after unmount"
        );
    }

    #[test]
    fn volume_unmount_succeeds_when_path_nonexistent() {
        let vm = VolumeMounter::new();
        let mount = Mount {
            id: "test".to_string(),
            mount_type: super::MountType::Volume,
            source: "/nonexistent/path/that/does/not/exist".to_string(),
            target: "/somevol".to_string(),
            opts: None,
        };

        // Should not error on nonexistent path
        vm.unmount(&mount)
            .expect("unmount of nonexistent path should succeed");
    }

    #[test]
    fn volume_unmount_succeeds_when_source_empty() {
        let vm = VolumeMounter::new();
        let mount = Mount {
            id: "test".to_string(),
            mount_type: super::MountType::Volume,
            source: String::new(),
            target: "/somevol".to_string(),
            opts: None,
        };

        vm.unmount(&mount)
            .expect("unmount with empty source should succeed");
    }

    #[test]
    fn volume_mount_unmount_succeeds_across_multiple_cycles() {
        let vm = VolumeMounter::new();

        for _ in 0..5 {
            let mut mount = Mount {
                id: uuid::Uuid::new_v4().to_string(),
                mount_type: super::MountType::Volume,
                source: String::new(),
                target: "/vol".to_string(),
                opts: None,
            };

            vm.mount(&mut mount).expect("mount should succeed");
            assert!(std::path::Path::new(&mount.source).exists());

            vm.unmount(&mount).expect("unmount should succeed");
            assert!(!std::path::Path::new(&mount.source).exists());
        }
    }
}
