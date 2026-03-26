//! Bind mount support following functional-rust conventions.
//!
//! # Architecture
//!
//! - **Data**: `BindMounter` holds configuration and mount state
//! - **Calc**: Pure source path validation logic
//! - **Actions**: Directory creation pushed to boundary

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, RwLock};

use thiserror::Error;
use twerk_core::mount::Mount;

/// Configuration for bind mounter.
#[derive(Debug, Clone, Default)]
pub struct BindConfig {
    /// Whether bind mounts are allowed.
    pub allowed: bool,
    /// List of allowed source paths.
    pub sources: Vec<String>,
}

/// Errors from bind mount operations.
#[derive(Debug, Error)]
pub enum BindMounterError {
    #[error("bind mounts are not allowed")]
    NotAllowed,

    #[error("source bind mount is not allowed: {0}")]
    SourceNotAllowed(String),

    #[error("error creating mount directory: {0}")]
    CreateDirectory(PathBuf, #[source] std::io::Error),

    #[error("error checking directory: {0}")]
    StatDirectory(PathBuf, #[source] std::io::Error),
}

/// Bind mounter for host directory mounts.
#[derive(Debug)]
pub struct BindMounter {
    /// Configuration.
    cfg: BindConfig,
    /// Active mounts state (source → source mapping).
    state: Arc<RwLock<BindMounterState>>,
}

#[derive(Debug, Default)]
struct BindMounterState {
    mounts: HashMap<String, String>,
}

impl BindMounter {
    /// Creates a new bind mounter with the given configuration.
    #[must_use]
    pub fn new(cfg: BindConfig) -> Self {
        Self {
            cfg,
            state: Arc::new(RwLock::new(BindMounterState::default())),
        }
    }
}

impl BindMounter {
    /// Mounts a bind mount.
    ///
    /// # Errors
    ///
    /// Returns `BindMounterError` if the mount cannot be created.
    pub fn mount(&self, mnt: &Mount) -> Result<(), BindMounterError> {
        // Check if bind mounts are allowed
        if !self.cfg.allowed {
            return Err(BindMounterError::NotAllowed);
        }

        // Get source path
        let source = mnt
            .source
            .as_ref()
            .ok_or_else(|| BindMounterError::SourceNotAllowed("no source specified".to_string()))?;

        // Check if source is allowed
        if !self.is_source_allowed(source) {
            return Err(BindMounterError::SourceNotAllowed(source.clone()));
        }

        // Check if already mounted
        {
            let state = self.state.read().map_err(|_| {
                BindMounterError::StatDirectory(
                    PathBuf::from("."),
                    std::io::Error::other("lock poisoned"),
                )
            })?;
            if state.mounts.contains_key(source) {
                return Ok(()); // Already mounted
            }
        }

        // Ensure source directory exists
        let source_path = Path::new(source);
        if !source_path.exists() {
            std::fs::create_dir_all(source_path)
                .map_err(|e| BindMounterError::CreateDirectory(source_path.to_path_buf(), e))?;
        }

        // Record the mount
        {
            let mut state = self.state.write().map_err(|_| {
                BindMounterError::StatDirectory(
                    PathBuf::from("."),
                    std::io::Error::other("lock poisoned"),
                )
            })?;
            state.mounts.insert(source.clone(), source.clone());
        }

        Ok(())
    }

    /// Unmounts a bind mount.
    #[must_use]
    pub fn unmount(&self, _mnt: &Mount) -> Result<(), BindMounterError> {
        // Bind mounts don't need explicit unmounting in the mounter
        Ok(())
    }

    /// Checks if a source path is allowed.
    fn is_source_allowed(&self, src: &str) -> bool {
        if self.cfg.sources.is_empty() {
            return true; // All sources allowed if list is empty
        }

        self.cfg
            .sources
            .iter()
            .any(|allowed| allowed.eq_ignore_ascii_case(src))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use twerk_core::mount::mount_type;

    #[test]
    fn test_bind_mount_not_allowed() {
        let mounter = BindMounter::new(BindConfig {
            allowed: false,
            sources: vec![],
        });

        let mnt = Mount::new(mount_type::BIND, "/tmp").with_source("/tmp");

        let result = mounter.mount(&mnt);
        assert!(result.is_err());
    }

    #[test]
    fn test_bind_mount_source_not_allowed() {
        let mounter = BindMounter::new(BindConfig {
            allowed: true,
            sources: vec!["/tmp".to_string()],
        });

        let mnt = Mount::new(mount_type::BIND, "/other").with_source("/other");

        let result = mounter.mount(&mnt);
        assert!(result.is_err());
    }

    #[test]
    fn test_bind_mount_allowed_source() {
        let mounter = BindMounter::new(BindConfig {
            allowed: true,
            sources: vec!["/tmp".to_string()],
        });

        let mnt = Mount::new(mount_type::BIND, "/tmp").with_source("/tmp");

        let result = mounter.mount(&mnt);
        assert!(result.is_ok());
    }

    #[test]
    fn test_bind_mount_empty_sources_allows_any() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let src = tmp.path().join("sub").to_string_lossy().to_string();

        let mounter = BindMounter::new(BindConfig {
            allowed: true,
            sources: vec![],
        });

        let mnt = Mount::new(mount_type::BIND, &src).with_source(&src);

        let result = mounter.mount(&mnt);
        assert!(result.is_ok());
    }

    #[test]
    fn test_bind_mount_case_insensitive() {
        let mounter = BindMounter::new(BindConfig {
            allowed: true,
            sources: vec!["/TMP".to_string()],
        });

        let mnt = Mount::new(mount_type::BIND, "/tmp").with_source("/tmp");

        let result = mounter.mount(&mnt);
        assert!(result.is_ok());
    }

    #[test]
    fn test_bind_mount_no_source() {
        let mounter = BindMounter::new(BindConfig {
            allowed: true,
            sources: vec![],
        });

        let mnt = Mount::new(mount_type::BIND, "/target");
        // source is None

        let result = mounter.mount(&mnt);
        assert!(result.is_err());
    }

    #[test]
    fn test_bind_mount_idempotent() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let src = tmp.path().to_string_lossy().to_string();

        let mounter = BindMounter::new(BindConfig {
            allowed: true,
            sources: vec![],
        });

        let mnt = Mount::new(mount_type::BIND, &src).with_source(&src);

        assert!(mounter.mount(&mnt).is_ok());
        assert!(mounter.mount(&mnt).is_ok()); // second call should also succeed
    }

    #[test]
    fn test_unmount_is_noop() {
        let mounter = BindMounter::new(BindConfig {
            allowed: true,
            sources: vec![],
        });

        let mnt = Mount::new(mount_type::BIND, "/target").with_source("/target");

        assert!(mounter.unmount(&mnt).is_ok());
    }
}
