// ----------------------------------------------------------------------------
// Docker Configuration
// ----------------------------------------------------------------------------

use std::sync::Arc;
use std::time::Duration;

/// Default workdir for task files.
pub const DEFAULT_WORKDIR: &str = "/twerk/workdir";

/// Default image TTL (3 days).
pub const DEFAULT_IMAGE_TTL: Duration = Duration::from_secs(72 * 60 * 60);

/// Default probe path.
pub const DEFAULT_PROBE_PATH: &str = "/";

/// Default probe timeout.
pub const DEFAULT_PROBE_TIMEOUT: &str = "1m";

/// Default command when none specified (uses /twerk/entrypoint script).
pub const DEFAULT_CMD: &[&str] = &["/twerk/entrypoint"];

/// Default entrypoint for `run` scripts.
pub const RUN_ENTRYPOINT: &[&str] = &["sh", "-c"];

/// Docker runtime configuration.
#[derive(Clone)]
pub struct DockerConfig {
    /// Docker config file path for registry credentials.
    pub config_file: Option<String>,
    /// Docker config path for registry credentials (alternative to `config_file`).
    pub config_path: Option<String>,
    /// Whether to run containers in privileged mode.
    pub privileged: bool,
    /// Image TTL for pruning.
    pub image_ttl: Duration,
    /// Whether to verify image integrity.
    pub image_verify: bool,
    /// Broker for log shipping and progress.
    pub broker: Option<Arc<dyn crate::broker::Broker>>,
    /// Whether to allow host network mode for containers.
    pub host_network: bool,
}

impl Default for DockerConfig {
    fn default() -> Self {
        Self {
            config_file: None,
            config_path: None,
            privileged: false,
            image_ttl: DEFAULT_IMAGE_TTL,
            image_verify: false,
            broker: None,
            host_network: false,
        }
    }
}

/// Builder for Docker runtime configuration.
#[derive(Default)]
pub struct DockerConfigBuilder {
    config_file: Option<String>,
    config_path: Option<String>,
    privileged: bool,
    image_ttl: Duration,
    image_verify: bool,
    broker: Option<Arc<dyn crate::broker::Broker>>,
    host_network: bool,
}

impl DockerConfigBuilder {
    #[must_use]
    pub fn with_image_ttl(mut self, ttl: Duration) -> Self {
        self.image_ttl = ttl;
        self
    }

    #[must_use]
    pub fn with_privileged(mut self, privileged: bool) -> Self {
        self.privileged = privileged;
        self
    }

    #[must_use]
    pub fn with_image_verify(mut self, verify: bool) -> Self {
        self.image_verify = verify;
        self
    }

    #[must_use]
    pub fn with_config_file(mut self, path: &str) -> Self {
        self.config_file = Some(path.to_string());
        self
    }

    #[must_use]
    pub fn with_host_network(mut self, enabled: bool) -> Self {
        self.host_network = enabled;
        self
    }

    #[must_use]
    pub fn with_broker(mut self, broker: Arc<dyn crate::broker::Broker>) -> Self {
        self.broker = Some(broker);
        self
    }

    #[must_use]
    pub fn with_config_path(mut self, path: &str) -> Self {
        self.config_path = Some(path.to_string());
        self
    }

    #[must_use]
    pub fn build(self) -> DockerConfig {
        DockerConfig {
            config_file: self.config_file,
            config_path: self.config_path,
            privileged: self.privileged,
            image_ttl: self.image_ttl,
            image_verify: self.image_verify,
            broker: self.broker,
            host_network: self.host_network,
        }
    }
}
