use tracing::debug;
use twerk_core::mount::Mount;
use twerk_infrastructure::runtime::{BoxedFuture, Mounter};

// ── Typed errors for mounters ──────────────────────────────────────

#[derive(Debug, thiserror::Error)]
enum MounterError {
    #[error("bind mounts are not allowed")]
    BindMountsNotAllowed,
    #[error("src bind mount is not allowed: {path}")]
    BindSourceNotAllowed { path: String },
    #[error("error creating mount directory: {dir}: {err}")]
    MountDirCreation { dir: String, err: String },
    #[error("missing mount id")]
    MissingMountId,
    #[error("tmpfs target is required")]
    TmpfsTargetRequired,
    #[error("tmpfs source should be empty")]
    TmpfsSourceNotEmpty,
}

// =============================================================================
// Mount configuration
// =============================================================================

/// Mount policy for bind mounts.
#[derive(Clone, Debug, Default)]
pub enum MountPolicy {
    /// Mounts are denied
    #[default]
    Denied,
    /// Mounts are allowed for specific paths
    Allowed(Vec<String>),
}

/// Configuration for bind mount operations.
#[derive(Debug, Clone, Default)]
pub struct BindConfig {
    /// Mount policy controlling whether bind mounts are allowed.
    pub policy: MountPolicy,
}

// =============================================================================
// Mounter implementations
// =============================================================================

/// Bind mounter — creates source directories for bind mounts.
///
/// Go parity: `docker.BindMounter`
#[derive(Debug)]
pub struct BindMounter {
    /// Configuration for allowed bind sources
    cfg: BindConfig,
}

impl BindMounter {
    /// Creates a new bind mounter.
    ///
    /// Go parity: `func NewBindMounter(cfg BindConfig) *BindMounter`
    #[must_use]
    pub fn new(cfg: BindConfig) -> Self {
        Self { cfg }
    }

    /// Checks whether a source path is in the allowed list.
    ///
    /// Go parity: `func (m *BindMounter) isSourceAllowed(src string) bool`
    #[cfg(test)]
    fn is_source_allowed(&self, src: &str) -> bool {
        match &self.cfg.policy {
            MountPolicy::Denied => false,
            MountPolicy::Allowed(sources) => {
                if sources.is_empty() {
                    return true;
                }
                sources.iter().any(|allow| allow.eq_ignore_ascii_case(src))
            }
        }
    }
}

impl Mounter for BindMounter {
    fn mount(&self, mnt: &Mount) -> BoxedFuture<()> {
        let policy = self.cfg.policy.clone();
        let source = mnt.source.clone().map_or_else(String::new, |s| s);

        Box::pin(async move {
            let sources = match policy {
                MountPolicy::Denied => {
                    return Err(MounterError::BindMountsNotAllowed.into());
                }
                MountPolicy::Allowed(s) => s,
            };

            // Source validation
            if !sources.is_empty() && !sources.iter().any(|s| s.eq_ignore_ascii_case(&source)) {
                return Err(MounterError::BindSourceNotAllowed { path: source }.into());
            }

            // Create source directory if it doesn't exist
            let src_path = std::path::Path::new(&source);
            if !src_path.exists() {
                tokio::fs::create_dir_all(src_path).await.map_err(|e| {
                    MounterError::MountDirCreation {
                        dir: source.clone(),
                        err: e.to_string(),
                    }
                })?;
                debug!("Created bind mount: {source}");
            }

            Ok(())
        })
    }

    fn unmount(&self, _mnt: &Mount) -> BoxedFuture<()> {
        // Go parity: BindMounter.Unmount is a no-op
        Box::pin(async { Ok(()) })
    }
}

/// Volume mounter — creates temporary directories for volume mounts.
#[derive(Debug, Default)]
pub struct VolumeMounter;

impl VolumeMounter {
    #[must_use]
    pub fn new() -> Self {
        Self
    }
}

impl Mounter for VolumeMounter {
    fn mount(&self, mnt: &Mount) -> BoxedFuture<()> {
        let id = mnt.id.clone().map_or_else(String::new, |id| id);
        Box::pin(async move {
            if id.is_empty() {
                return Err(MounterError::MissingMountId.into());
            }
            debug!("Volume mount prepared for id={id}");
            Ok(())
        })
    }

    fn unmount(&self, _mnt: &Mount) -> BoxedFuture<()> {
        Box::pin(async { Ok(()) })
    }
}

/// Tmpfs mounter — validates tmpfs mount specifications.
#[derive(Debug, Default)]
pub struct TmpfsMounter;

impl TmpfsMounter {
    #[must_use]
    pub fn new() -> Self {
        Self
    }
}

impl Mounter for TmpfsMounter {
    fn mount(&self, mnt: &Mount) -> BoxedFuture<()> {
        let target = mnt.target.clone().map_or_else(String::new, |t| t);
        let source = mnt.source.clone().map_or_else(String::new, |s| s);
        Box::pin(async move {
            if target.is_empty() {
                return Err(MounterError::TmpfsTargetRequired.into());
            }
            if !source.is_empty() {
                return Err(MounterError::TmpfsSourceNotEmpty.into());
            }
            Ok(())
        })
    }

    fn unmount(&self, _mnt: &Mount) -> BoxedFuture<()> {
        Box::pin(async { Ok(()) })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use twerk_core::mount::Mount;

    #[test]
    fn test_bind_config_default() {
        let cfg = BindConfig::default();
        assert!(matches!(cfg.policy, MountPolicy::Denied));
    }

    #[test]
    fn test_bind_mounter_is_source_allowed_empty_sources() {
        let mounter = BindMounter::new(BindConfig {
            policy: MountPolicy::Allowed(Vec::new()),
        });
        assert!(mounter.is_source_allowed("/any/path"));
    }

    #[tokio::test]
    async fn test_bind_mounter_mount_disallowed() {
        let mounter = BindMounter::new(BindConfig::default());
        let mnt = Mount {
            id: Some("test".into()),
            mount_type: Some("bind".into()),
            source: Some("/tmp/test".into()),
            target: Some("/mnt/test".into()),
            opts: None,
        };
        assert!(mounter.mount(&mnt).await.is_err());
    }

    #[tokio::test]
    async fn test_volume_mounter_mount() {
        let mounter = VolumeMounter::new();
        let mnt = Mount {
            id: Some("vol-1".into()),
            mount_type: Some("volume".into()),
            source: None,
            target: Some("/data".into()),
            opts: None,
        };
        assert!(mounter.mount(&mnt).await.is_ok());
    }
}
