use std::collections::HashMap;
use std::sync::Arc;

use twerk_app::engine::coordinator::auth::{BasicAuthConfig, KeyAuthConfig};
use twerk_app::engine::coordinator::limits::{BodyLimitConfig, RateLimitConfig};
use twerk_app::engine::coordinator::middleware::HttpLogConfig;
use twerk_infrastructure::broker::Broker;
use twerk_infrastructure::datastore::Datastore;

use super::trigger_api::{InMemoryTriggerDatastore, TriggerAppState};

#[derive(Clone)]
pub struct Config {
    pub address: String,
    pub enabled: HashMap<String, bool>,
    pub cors_origins: Vec<String>,
    pub basic_auth: Option<BasicAuthConfig>,
    pub key_auth: Option<KeyAuthConfig>,
    pub rate_limit: Option<RateLimitConfig>,
    pub body_limit: Option<BodyLimitConfig>,
    pub http_log: Option<HttpLogConfig>,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            address: "0.0.0.0:8000".to_string(),
            enabled: HashMap::new(),
            cors_origins: vec![],
            basic_auth: None,
            key_auth: None,
            rate_limit: None,
            body_limit: None,
            http_log: None,
        }
    }
}

#[derive(Clone)]
pub struct AppState {
    pub broker: Arc<dyn Broker>,
    pub ds: Arc<dyn Datastore>,
    pub trigger_state: TriggerAppState,
    pub config: Config,
}

impl AppState {
    #[must_use]
    pub fn new(broker: Arc<dyn Broker>, ds: Arc<dyn Datastore>, config: Config) -> Self {
        Self {
            broker,
            ds,
            trigger_state: TriggerAppState {
                trigger_ds: Arc::new(InMemoryTriggerDatastore::new()),
            },
            config,
        }
    }
}
