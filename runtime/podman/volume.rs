//! Volume mounter for podman
//!
//! Creates temporary directories for volume mounts with world-writable
//! permissions, matching the Go implementation.

use std::path::PathBuf;

#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;

use anyhow::Context;
use tracing::debug;

use super::{Mount, Mounter};

/// VolumeMounter creates temporary directories for volume mounts
#[derive(Debug, Default)]
pub struct VolumeMounter;

impl VolumeMounter {
    pub fn new() -> Self {
        Self
    }
}

impl Mounter for VolumeMounter {
    fn mount(&self, mount: &mut Mount) -> Result<(), anyhow::Error> {
        let temp_dir =
            tempfile::tempdir().context("failed to create temporary directory for volume")?;

        // Set permissions to 0777 (world-writable) matching Go behavior
        let path = temp_dir.path();
        std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o777))
            .context("failed to chmod temporary directory")?;

        // Keep the directory alive beyond this scope
        let kept = temp_dir.keep();
        let mount_source = kept.to_string_lossy().to_string();

        mount.source = mount_source.clone();

        debug!("Created volume at {}", mount_source);

        Ok(())
    }

    fn unmount(&self, mount: &Mount) -> Result<(), anyhow::Error> {
        if !mount.source.is_empty() {
            let path = PathBuf::from(&mount.source);
            if path.exists() {
                std::fs::remove_dir_all(&path).context("failed to remove volume directory")?;
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::runtime::podman::MountType;

    /// Mirrors Go's TestCreateVolume:
    /// Verifies mount creates a directory, it exists, and unmount removes it.
    #[test]
    fn test_create_volume() {
        let vm = VolumeMounter::new();
        let mut mount = Mount {
            id: "test".to_string(),
            mount_type: MountType::Volume,
            source: String::new(),
            target: "/somevol".to_string(),
            opts: None,
        };

        let result = vm.mount(&mut mount);
        assert!(result.is_ok());
        assert_eq!("/somevol", mount.target);
        assert!(!mount.source.is_empty(), "source should be populated");

        // Verify the directory exists
        let metadata = std::fs::metadata(&mount.source);
        assert!(metadata.is_ok(), "mounted directory should exist");

        // Verify the directory is world-writable
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mode = metadata.expect("metadata ok").permissions().mode();
            assert_eq!(mode & 0o777, 0o777);
        }

        // Unmount should remove the directory
        let result = vm.unmount(&mount);
        assert!(result.is_ok());

        // Verify directory no longer exists
        let metadata = std::fs::metadata(&mount.source);
        assert!(
            metadata.is_err(),
            "directory should be removed after unmount"
        );
    }

    /// Mirrors Go's TestCreateMountVolume:
    /// Verifies mount populates source for a volume mount type.
    #[test]
    fn test_create_mount_volume() {
        let vm = VolumeMounter::new();
        let mut mount = Mount {
            id: "test".to_string(),
            mount_type: MountType::Volume,
            source: String::new(),
            target: "/somevol".to_string(),
            opts: None,
        };

        let result = vm.mount(&mut mount);
        assert!(result.is_ok());

        // Cleanup
        let _ = vm.unmount(&mount);

        assert_eq!("/somevol", mount.target);
        assert!(!mount.source.is_empty());
    }

    #[test]
    fn test_unmount_nonexistent() {
        let vm = VolumeMounter::new();
        let mount = Mount {
            id: "test".to_string(),
            mount_type: MountType::Volume,
            source: "/nonexistent/path/that/does/not/exist".to_string(),
            target: "/somevol".to_string(),
            opts: None,
        };

        // Should not error on nonexistent path
        let result = vm.unmount(&mount);
        assert!(result.is_ok());
    }

    #[test]
    fn test_unmount_empty_source() {
        let vm = VolumeMounter::new();
        let mount = Mount {
            id: "test".to_string(),
            mount_type: MountType::Volume,
            source: String::new(),
            target: "/somevol".to_string(),
            opts: None,
        };

        let result = vm.unmount(&mount);
        assert!(result.is_ok());
    }

    /// Verifies that multiple mount/unmount cycles work correctly.
    #[test]
    fn test_multiple_mount_unmount_cycles() {
        let vm = VolumeMounter::new();

        for _ in 0..5 {
            let mut mount = Mount {
                id: uuid::Uuid::new_v4().to_string(),
                mount_type: MountType::Volume,
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
