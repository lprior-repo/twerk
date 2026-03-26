//! Twerk Engine - Endpoint registration and routing

use super::types::EndpointHandler;
use std::collections::HashMap;

/// HTTP endpoint registry
pub struct EndpointRegistry {
    endpoints: HashMap<String, EndpointHandler>,
}

impl EndpointRegistry {
    pub fn new() -> Self {
        Self {
            endpoints: HashMap::new(),
        }
    }

    /// Register an endpoint handler
    pub fn register(&mut self, method: &str, path: &str, handler: EndpointHandler) {
        let key = format!("{} {}", method, path);
        self.endpoints.insert(key, handler);
    }

    /// Get an endpoint handler by method and path
    pub fn get(&self, method: &str, path: &str) -> Option<&EndpointHandler> {
        let key = format!("{} {}", method, path);
        self.endpoints.get(&key)
    }

    /// Get all registered endpoints
    pub fn iter(&self) -> impl Iterator<Item = (&String, &EndpointHandler)> {
        self.endpoints.iter()
    }

    /// Check if an endpoint exists
    pub fn contains(&self, method: &str, path: &str) -> bool {
        let key = format!("{} {}", method, path);
        self.endpoints.contains_key(&key)
    }
}

impl Default for EndpointRegistry {
    fn default() -> Self {
        Self::new()
    }
}
