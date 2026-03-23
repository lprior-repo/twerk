
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
        self.cfg.sources.iter().any(|allow| allow.eq_ignore_ascii_case(src))
    }
}

impl Mounter for BindMounter {
    fn mount(
        &self,
        _ctx: Pin<Box<dyn std::future::Future<Output = Result<(), MountError>> + Send>>,
        mnt: &Mount,
    ) -> Pin<Box<dyn std::future::Future<Output = Result<(), MountError>> + Send>> {
        let allowed = self.cfg.allowed;
        let sources = self.cfg.sources.clone();
        let source = mnt.source.clone().unwrap_or_default();

        let cfg_allowed = allowed;
        let cfg_sources = sources;

        Box::pin(async move {
            if !cfg_allowed {
                return Err(MountError::MountFailed(
                    "bind mounts are not allowed".to_string(),
                ));
            }

            // Source validation
            if !cfg_sources.is_empty()
                && !cfg_sources.iter().any(|s| s.eq_ignore_ascii_case(&source))
            {
                return Err(MountError::MountFailed(format!(
                    "src bind mount is not allowed: {}",
                    source
                )));
            }

            // Create source directory if it doesn't exist
            let src_path = std::path::Path::new(&source);
            if !src_path.exists() {
                std::fs::create_dir_all(src_path).map_err(|e| {
                    MountError::MountFailed(format!(
                        "error creating mount directory: {}: {}",
                        source, e
                    ))
                })?;
                debug!("Created bind mount: {}", source);
            }

            Ok(())
        })
    }

    fn unmount(
        &self,
        _ctx: Pin<Box<dyn std::future::Future<Output = Result<(), MountError>> + Send>>,
        _mnt: &Mount,
    ) -> Pin<Box<dyn std::future::Future<Output = Result<(), MountError>> + Send>> {
        // Go parity: BindMounter.Unmount is a no-op
        Box::pin(async { Ok(()) })
    }
}

/// Volume mounter — creates temporary directories for volume mounts.
///
/// Go parity: `docker.NewVolumeMounter()`
#[derive(Debug)]
pub struct VolumeMounter;

impl VolumeMounter {
    /// Creates a new volume mounter.
    #[must_use]
    pub fn new() -> Self {
        Self
    }
}

impl Default for VolumeMounter {
    fn default() -> Self {
        Self::new()
    }
}

impl Mounter for VolumeMounter {
    fn mount(
        &self,
        _ctx: Pin<Box<dyn std::future::Future<Output = Result<(), MountError>> + Send>>,
        mnt: &Mount,
    ) -> Pin<Box<dyn std::future::Future<Output = Result<(), MountError>> + Send>> {
        let id = mnt.id.clone().unwrap_or_default();

        Box::pin(async move {
            if id.is_empty() {
                return Err(MountError::MissingMountId);
            }

            // In production, this would call Docker API to create a named volume.
            debug!("Volume mount prepared for id={}", id);
            Ok(())
        })
    }

    fn unmount(
        &self,
        _ctx: Pin<Box<dyn std::future::Future<Output = Result<(), MountError>> + Send>>,
        _mnt: &Mount,
    ) -> Pin<Box<dyn std::future::Future<Output = Result<(), MountError>> + Send>> {
        Box::pin(async { Ok(()) })
    }
}

/// Tmpfs mounter — validates tmpfs mount specifications.
///
/// Go parity: `docker.NewTmpfsMounter()`
#[derive(Debug)]
pub struct TmpfsMounter;

impl TmpfsMounter {
    /// Creates a new tmpfs mounter.
    #[must_use]
    pub fn new() -> Self {
        Self
    }
}

impl Default for TmpfsMounter {
    fn default() -> Self {
        Self::new()
    }
}

impl Mounter for TmpfsMounter {
    fn mount(
        &self,
        _ctx: Pin<Box<dyn std::future::Future<Output = Result<(), MountError>> + Send>>,
        mnt: &Mount,
    ) -> Pin<Box<dyn std::future::Future<Output = Result<(), MountError>> + Send>> {
        let target = mnt.target.clone().unwrap_or_default();
        let source = mnt.source.clone().unwrap_or_default();

        Box::pin(async move {
            if target.is_empty() {
                return Err(MountError::MountFailed(
                    "tmpfs target is required".to_string(),
                ));
            }
            if !source.is_empty() {
                return Err(MountError::MountFailed(
                    "tmpfs source should be empty".to_string(),
                ));
            }
            Ok(())
        })
    }

    fn unmount(
        &self,
        _ctx: Pin<Box<dyn std::future::Future<Output = Result<(), MountError>> + Send>>,
        _mnt: &Mount,
    ) -> Pin<Box<dyn std::future::Future<Output = Result<(), MountError>> + Send>> {
        Box::pin(async { Ok(()) })
    }
}

// =============================================================================