//! Twerk Engine - Provider registration for broker and datastore

use std::collections::HashMap;
use twerk_infrastructure::broker::Broker;
use twerk_infrastructure::datastore::Datastore;

/// Provider registry for broker and datastore implementations
pub struct ProviderRegistry {
    ds_providers: HashMap<String, Box<dyn Datastore + Send + Sync>>,
    broker_providers: HashMap<String, Box<dyn Broker + Send + Sync>>,
}

impl ProviderRegistry {
    pub fn new() -> Self {
        Self {
            ds_providers: HashMap::new(),
            broker_providers: HashMap::new(),
        }
    }

    /// Register a datastore provider
    pub fn register_datastore(&mut self, name: &str, provider: Box<dyn Datastore + Send + Sync>) {
        let name = name.to_string();
        if !self.ds_providers.contains_key(&name) {
            self.ds_providers.insert(name, provider);
        }
    }

    /// Register a broker provider
    pub fn register_broker(&mut self, name: &str, provider: Box<dyn Broker + Send + Sync>) {
        let name = name.to_string();
        if !self.broker_providers.contains_key(&name) {
            self.broker_providers.insert(name, provider);
        }
    }

    /// Get a registered datastore provider
    pub fn get_datastore(&self, name: &str) -> Option<&Box<dyn Datastore + Send + Sync>> {
        self.ds_providers.get(name)
    }

    /// Get a registered broker provider
    pub fn get_broker(&self, name: &str) -> Option<&Box<dyn Broker + Send + Sync>> {
        self.broker_providers.get(name)
    }

    /// Check if a datastore provider exists
    pub fn has_datastore(&self, name: &str) -> bool {
        self.ds_providers.contains_key(name)
    }

    /// Check if a broker provider exists
    pub fn has_broker(&self, name: &str) -> bool {
        self.broker_providers.contains_key(name)
    }
}

impl Default for ProviderRegistry {
    fn default() -> Self {
        Self::new()
    }
}
