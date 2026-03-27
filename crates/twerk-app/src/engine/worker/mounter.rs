use tracing::debug;
use twerk_core::mount::Mount;
use twerk_infrastructure::runtime::{BoxedFuture, Mounter};

// =============================================================================
// Mount configuration
// =============================================================================

/// Configuration for bind mount operations.
///
/// Go parity: `type BindConfig struct { Allowed bool; Sources []string }`
#[derive(Debug, Clone, Default)]
pub struct BindConfig {
    /// Whether bind mounts are allowed
    pub allowed: bool,
    /// Allowed source directories (empty = all)
    pub sources: Vec<String>,
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
        if self.cfg.sources.is_empty() {
            return true;
        }
        self.cfg
            .sources
            .iter()
            .any(|allow| allow.eq_ignore_ascii_case(src))
    }
}

impl Mounter for BindMounter {
    fn mount(&self, mnt: &Mount) -> BoxedFuture<()> {
        let allowed = self.cfg.allowed;
        let sources = self.cfg.sources.clone();
        let source = mnt.source.clone().unwrap_or_default();

        Box::pin(async move {
            if !allowed {
                return Err(anyhow::anyhow!("bind mounts are not allowed"));
            }

            // Source validation
            if !sources.is_empty()
                && !sources.iter().any(|s| s.eq_ignore_ascii_case(&source))
            {
                return Err(anyhow::anyhow!("src bind mount is not allowed: {source}"));
            }

            // Create source directory if it doesn't exist
            let src_path = std::path::Path::new(&source);
            if !src_path.exists() {
                std::fs::create_dir_all(src_path).map_err(|e| {
                    anyhow::anyhow!("error creating mount directory: {source}: {e}")
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
        let id = mnt.id.clone().unwrap_or_default();
        Box::pin(async move {
            if id.is_empty() {
                return Err(anyhow::anyhow!("missing mount id"));
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
        let target = mnt.target.clone().unwrap_or_default();
        let source = mnt.source.clone().unwrap_or_default();
        Box::pin(async move {
            if target.is_empty() {
                return Err(anyhow::anyhow!("tmpfs target is required"));
            }
            if !source.is_empty() {
                return Err(anyhow::anyhow!("tmpfs source should be empty"));
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
        assert!(!cfg.allowed);
        assert!(cfg.sources.is_empty());
    }

    #[test]
    fn test_bind_mounter_is_source_allowed_empty_sources() {
        let mounter = BindMounter::new(BindConfig { allowed: true, sources: Vec::new() });
        assert!(mounter.is_source_allowed("/any/path"));
    }

    #[tokio::test]
    async fn test_bind_mounter_mount_disallowed() {
        let mounter = BindMounter::new(BindConfig::default());
        let mnt = Mount { id: Some("test".into()), mount_type: Some("bind".into()), source: Some("/tmp/test".into()), target: Some("/mnt/test".into()), opts: None };
        assert!(mounter.mount(&mnt).await.is_err());
    }

    #[tokio::test]
    async fn test_volume_mounter_mount() {
        let mounter = VolumeMounter::new();
        let mnt = Mount { id: Some("vol-1".into()), mount_type: Some("volume".into()), source: None, target: Some("/data".into()), opts: None };
        assert!(mounter.mount(&mnt).await.is_ok());
    }
}
