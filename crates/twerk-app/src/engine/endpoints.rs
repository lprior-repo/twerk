//! Twerk Engine - Endpoint registration and routing

use super::types::EndpointHandler;
use std::collections::HashMap;

#[derive(Debug, Clone)]
pub enum PathPattern {
    Param(String),
    Wildcard,
}

#[derive(Clone)]
pub struct RouteMatch {
    pub handler: EndpointHandler,
    pub params: HashMap<String, String>,
}

pub struct PatternEntry {
    pub method: String,
    pub path_pattern: PathPattern,
    pub path_template: String,
    pub handler: EndpointHandler,
}

fn is_pattern(path: &str) -> bool {
    path.contains(':') || path.ends_with('*')
}

fn match_pattern(template: &str, path: &str) -> Option<HashMap<String, String>> {
    if let Some(prefix) = template.strip_suffix('*') {
        if path.starts_with(prefix) {
            return Some(HashMap::new());
        }
        return None;
    }
    let template_parts: Vec<&str> = template.split('/').collect();
    let path_parts: Vec<&str> = path.split('/').collect();
    if template_parts.len() != path_parts.len() {
        return None;
    }
    let mut params = HashMap::new();
    for (t, p) in template_parts.iter().zip(path_parts.iter()) {
        if let Some(stripped) = t.strip_prefix(':') {
            params.insert(stripped.to_string(), p.to_string());
        } else if *t != *p {
            return None;
        }
    }
    Some(params)
}

/// HTTP endpoint registry
pub struct EndpointRegistry {
    endpoints: HashMap<String, EndpointHandler>,
    patterns: Vec<PatternEntry>,
}

impl EndpointRegistry {
    pub fn new() -> Self {
        Self {
            endpoints: HashMap::new(),
            patterns: Vec::new(),
        }
    }

    /// Register an endpoint handler
    pub fn register(&mut self, method: &str, path: &str, handler: EndpointHandler) {
        let key = format!("{} {}", method, path);
        if is_pattern(path) {
            let path_pattern = if path.ends_with('*') {
                PathPattern::Wildcard
            } else {
                PathPattern::Param(
                    path.split(':')
                        .nth(1)
                        .map_or_else(String::new, |s| s.to_string()),
                )
            };
            self.patterns.push(PatternEntry {
                method: method.to_string(),
                path_pattern,
                path_template: path.to_string(),
                handler,
            });
        } else {
            self.endpoints.insert(key, handler);
        }
    }

    /// Get an endpoint handler by method and path (exact match only)
    pub fn get(&self, method: &str, path: &str) -> Option<&EndpointHandler> {
        let key = format!("{} {}", method, path);
        self.endpoints.get(&key)
    }

    /// Get an endpoint handler by method and path, with pattern matching support
    pub fn get_with_pattern(&self, method: &str, path: &str) -> Option<RouteMatch> {
        if let Some(handler) = self.get(method, path) {
            return Some(RouteMatch {
                handler: handler.clone(),
                params: HashMap::new(),
            });
        }
        for entry in &self.patterns {
            if entry.method == method {
                if let Some(params) = match_pattern(&entry.path_template, path) {
                    return Some(RouteMatch {
                        handler: entry.handler.clone(),
                        params,
                    });
                }
            }
        }
        None
    }

    /// Get all registered endpoints
    pub fn iter(&self) -> impl Iterator<Item = (&String, &EndpointHandler)> {
        self.endpoints.iter()
    }

    /// Check if an endpoint exists
    pub fn contains(&self, method: &str, path: &str) -> bool {
        let key = format!("{} {}", method, path);
        self.endpoints.contains_key(&key)
            || self
                .patterns
                .iter()
                .any(|p| p.method == method && match_pattern(&p.path_template, path).is_some())
    }
}

impl Default for EndpointRegistry {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]
    use super::*;
    use std::sync::Arc;

    fn dummy_handler() -> EndpointHandler {
        use axum::http::request::Parts;
        use axum::response::Response;
        use bytes::Bytes;
        use futures_util::FutureExt;
        Arc::new(|_req: Parts, _body: Bytes| {
            async move { Response::builder().status(200).body("".into()).unwrap() }.boxed()
        })
    }

    #[test]
    fn test_exact_match_takes_priority() {
        let mut registry = EndpointRegistry::new();
        registry.register("GET", "/jobs", dummy_handler());
        registry.register("GET", "/jobs/:id", dummy_handler());
        assert!(registry.get("GET", "/jobs").is_some());
    }

    #[test]
    fn test_named_param_pattern_matches() {
        let mut registry = EndpointRegistry::new();
        registry.register("GET", "/jobs/:id", dummy_handler());
        let result = registry.get_with_pattern("GET", "/jobs/123");
        assert!(result.is_some());
        let match_result = result.unwrap();
        assert_eq!(match_result.params.get("id"), Some(&"123".to_string()));
    }

    #[test]
    fn test_wildcard_pattern_matches() {
        let mut registry = EndpointRegistry::new();
        registry.register("GET", "/jobs/*", dummy_handler());
        let result = registry.get_with_pattern("GET", "/jobs/all");
        assert!(result.is_some());
    }

    #[test]
    fn test_no_match_returns_none() {
        let registry = EndpointRegistry::new();
        let result = registry.get_with_pattern("GET", "/nonexistent");
        assert!(result.is_none());
    }

    #[test]
    fn test_conflicting_patterns_detected() {
        let mut registry = EndpointRegistry::new();
        registry.register("GET", "/jobs/:id", dummy_handler());
        registry.register("GET", "/jobs/*", dummy_handler());
        assert!(registry.contains("GET", "/jobs/123"));
        assert!(registry.contains("GET", "/jobs/all"));
    }

    #[test]
    fn test_contains_with_pattern() {
        let mut registry = EndpointRegistry::new();
        registry.register("GET", "/jobs/:id", dummy_handler());
        assert!(registry.contains("GET", "/jobs/123"));
        assert!(!registry.contains("GET", "/jobs"));
    }

    #[test]
    fn test_multiple_params() {
        let mut registry = EndpointRegistry::new();
        registry.register("GET", "/orgs/:org/jobs/:job", dummy_handler());
        let result = registry.get_with_pattern("GET", "/orgs/acme/jobs/456");
        assert!(result.is_some());
        let match_result = result.unwrap();
        assert_eq!(match_result.params.get("org"), Some(&"acme".to_string()));
        assert_eq!(match_result.params.get("job"), Some(&"456".to_string()));
    }
}
