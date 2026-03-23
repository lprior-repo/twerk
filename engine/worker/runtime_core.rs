
/// Mock runtime for placeholder implementation.
#[derive(Debug)]
pub struct MockRuntime;

impl RuntimeTrait for MockRuntime {
    fn run(
        &self,
        _ctx: std::sync::Arc<tokio::sync::RwLock<()>>,
        _task: &mut tork::task::Task,
    ) -> tork::runtime::BoxedFuture<()> {
        Box::pin(async { Ok(()) })
    }

    fn health_check(&self) -> tork::runtime::BoxedFuture<()> {
        Box::pin(async { Ok(()) })
    }
}

// =============================================================================
// No-op worker
// =============================================================================

/// No-op worker implementation for placeholder.
#[derive(Debug)]
pub struct NoOpWorker;

impl Worker for NoOpWorker {
    fn start(&self) -> BoxedFuture<()> {
        Box::pin(async { Ok(()) })
    }

    fn stop(&self) -> BoxedFuture<()> {
        Box::pin(async { Ok(()) })
    }
}

// =============================================================================
// Runtime configuration
// =============================================================================

/// Configuration for runtime initialization.
///
/// Go parity: reads from conf.StringDefault, conf.Bool, conf.Strings etc.
#[derive(Debug, Clone)]
pub struct RuntimeConfig {
    /// Runtime type (docker, shell, podman)
    pub runtime_type: String,
    /// Docker-specific: privileged mode
    pub docker_privileged: bool,
    /// Docker-specific: image TTL in seconds
    pub docker_image_ttl_secs: u64,
    /// Docker-specific: verify images
    pub docker_image_verify: bool,
    /// Docker-specific: config file path
    pub docker_config: String,
    /// Shell-specific: command
    pub shell_cmd: Vec<String>,
    /// Shell-specific: UID
    pub shell_uid: String,
    /// Shell-specific: GID
    pub shell_gid: String,
    /// Podman-specific: privileged mode
    pub podman_privileged: bool,
    /// Podman-specific: host network
    pub podman_host_network: bool,
    /// Bind mount config
    pub bind_allowed: bool,
    /// Bind mount allowed sources
    pub bind_sources: Vec<String>,
    /// Host environment variable specs for middleware
    pub hostenv_vars: Vec<String>,
}

impl Default for RuntimeConfig {
    fn default() -> Self {
        Self {
            runtime_type: runtime_type::DEFAULT.to_string(),
            docker_privileged: false,
            docker_image_ttl_secs: 72 * 60 * 60,
            docker_image_verify: false,
            docker_config: String::new(),
            shell_cmd: vec!["bash".to_string(), "-c".to_string()],
            shell_uid: "-".to_string(),
            shell_gid: "-".to_string(),
            podman_privileged: false,
            podman_host_network: false,
            bind_allowed: false,
            bind_sources: Vec::new(),
            hostenv_vars: Vec::new(),
        }
    }
}

/// Reads runtime configuration from environment variables.
///
/// Go parity: reads from conf.StringDefault, conf.Bool, conf.Strings
pub fn read_runtime_config() -> RuntimeConfig {
    let runtime_type = config_string_default("runtime.type", runtime_type::DEFAULT);

    RuntimeConfig {
        runtime_type: runtime_type.clone(),
        docker_privileged: config_bool("runtime.docker.privileged"),
        docker_image_verify: config_bool("runtime.docker.image.verify"),
        docker_config: config_string_default("runtime.docker.config", ""),
        shell_cmd: config_strings("runtime.shell.cmd"),
        shell_uid: config_string_default("runtime.shell.uid", "-"),
        shell_gid: config_string_default("runtime.shell.gid", "-"),
        podman_privileged: config_bool("runtime.podman.privileged"),
        podman_host_network: config_bool("runtime.podman.host.network"),
        bind_allowed: config_bool("mounts.bind.allowed"),
        bind_sources: config_strings("mounts.bind.sources"),
        hostenv_vars: config_strings("middleware.task.hostenv.vars"),
        ..Default::default()
    }
}

// =============================================================================