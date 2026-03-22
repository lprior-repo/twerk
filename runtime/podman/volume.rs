//! Volume mounter for podman

use std::path::PathBuf;

use anyhow::Context;
use tracing::debug;

use super::{Mount, Mounter};

/// VolumeMounter creates temporary directories for volume mounts
#[derive(Debug)]
pub struct VolumeMounter {
    _phantom: std::marker::PhantomData<()>,
}

impl VolumeMounter {
    pub fn new() -> Self {
        Self {
            _phantom: std::marker::PhantomData,
        }
    }
}

impl Default for VolumeMounter {
    fn default() -> Self {
        Self::new()
    }
}

impl Mounter for VolumeMounter {
    fn mount(&self, mount: &mut Mount) -> Result<(), anyhow::Error> {
        let temp_dir = tempfile::tempdir()
            .context("failed to create temporary directory for volume")?
            .keep();

        // Note: In a real implementation, we'd set permissions properly
        // For now we just set the source
        let mount_source = temp_dir.to_string_lossy().to_string();

        // Set the mount's source to the temp directory path (matching Go behavior)
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
    use super::Mount;
    use super::VolumeMounter;
    use crate::runtime::podman::MountType;
    use crate::runtime::podman::Mounter;

    #[test]
    fn test_create_volume() {
        let vm = VolumeMounter::new();
        let mut mount = Mount {
            id: "test".to_string(),
            mount_type: MountType::Volume,
            source: String::new(),
            target: "/somevol".to_string(),
        };

        let result = vm.mount(&mut mount);
        assert!(result.is_ok());
        assert!(!mount.source.is_empty());
    }
}
