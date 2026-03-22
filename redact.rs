//! Redaction utilities.
//!
//! Provides functionality for redacting sensitive information from strings and data structures.

/// The redaction replacement string.
pub const REDACTED_STR: &str = "[REDACTED]";

use std::sync::Arc;

/// A redacter that can redact sensitive information.
pub struct Redacter {
    matchers: Vec<Arc<dyn Matcher>>,
}

impl Redacter {
    /// Create a new redacter with the given matchers.
    pub fn new(matchers: Vec<Arc<dyn Matcher>>) -> Self {
        Self { matchers }
    }

    /// Redact a task log part by replacing secret values in its contents.
    pub fn redact_task_log_part(
        &self,
        part: &mut tork::task::TaskLogPart,
        secrets: &std::collections::HashMap<String, String>,
    ) {
        let mut contents = part.contents.clone().unwrap_or_default();
        for secret in secrets.values() {
            if !secret.is_empty() {
                contents = contents.replace(secret, REDACTED_STR);
            }
        }
        part.contents = Some(contents);
    }

    /// Check if a key should be redacted based on matchers.
    pub fn should_redact(&self, key: &str) -> bool {
        self.matchers.iter().any(|m| m.matches(key))
    }

    /// Redact a map of variables.
    pub fn redact_vars(
        &self,
        vars: &std::collections::HashMap<String, String>,
        secrets: &std::collections::HashMap<String, String>,
    ) -> std::collections::HashMap<String, String> {
        let mut redacted = std::collections::HashMap::new();
        for (k, v) in vars {
            let mut value = v.clone();
            // Check if key matches any matcher
            if self.should_redact(k) {
                value = REDACTED_STR.to_string();
            }
            // Replace secret values in content
            for secret in secrets.values() {
                if !secret.is_empty() && value.contains(secret) {
                    value = value.replace(secret, REDACTED_STR);
                }
            }
            redacted.insert(k.clone(), value);
        }
        redacted
    }
}

/// Trait for matching keys that should be redacted.
pub trait Matcher: Send + Sync {
    /// Check if a key matches this matcher.
    fn matches(&self, key: &str) -> bool;
}

/// Matcher that checks if a key contains a substring (case-insensitive).
#[derive(Clone)]
pub struct ContainsMatcher {
    substring: String,
}

impl ContainsMatcher {
    /// Create a new contains matcher.
    pub fn new(substr: &str) -> Arc<Self> {
        Arc::new(Self {
            substring: substr.to_uppercase(),
        })
    }
}

impl Matcher for ContainsMatcher {
    fn matches(&self, key: &str) -> bool {
        key.to_uppercase().contains(&self.substring)
    }
}

/// Create a matcher that checks if a key contains the given substring (case-insensitive).
pub fn contains_matcher(substr: &str) -> Arc<dyn Matcher> {
    ContainsMatcher::new(substr)
}

/// Matcher that uses wildcard pattern matching.
pub struct WildcardMatcher {
    pattern: String,
}

impl WildcardMatcher {
    /// Create a new wildcard matcher.
    pub fn new(pattern: &str) -> Arc<Self> {
        Arc::new(Self {
            pattern: pattern.to_string(),
        })
    }
}

impl Matcher for WildcardMatcher {
    fn matches(&self, key: &str) -> bool {
        crate::wildcard::match_pattern(&self.pattern, key)
    }
}

/// Create a matcher that uses wildcard pattern matching.
pub fn wildcard_matcher(pattern: &str) -> Arc<dyn Matcher> {
    WildcardMatcher::new(pattern)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_contains_matcher() {
        let matcher = contains_matcher("SECRET");
        assert!(matcher.matches("MY_SECRET_KEY"));
        assert!(matcher.matches("secret"));
        assert!(!matcher.matches("PUBLIC"));
    }

    #[test]
    fn test_redact_vars() {
        let redacter = Redacter::new(vec![
            contains_matcher("SECRET"),
            contains_matcher("PASSWORD"),
        ]);

        let mut vars = std::collections::HashMap::new();
        vars.insert("api_key".to_string(), "secret123".to_string());
        vars.insert("username".to_string(), "admin".to_string());
        vars.insert("password".to_string(), "supersecret".to_string());

        let mut secrets = std::collections::HashMap::new();
        secrets.insert("my_secret".to_string(), "secret123".to_string());

        let redacted = redacter.redact_vars(&vars, &secrets);

        // api_key matches SECRET matcher, so it should be redacted
        assert_eq!(redacted.get("api_key"), Some(&"[REDACTED]".to_string()));
        // username should remain unchanged
        assert_eq!(redacted.get("username"), Some(&"admin".to_string()));
        // password matches PASSWORD matcher
        assert_eq!(redacted.get("password"), Some(&"[REDACTED]".to_string()));
    }

    #[test]
    fn test_redact_task_log_part() {
        let redacter = Redacter::new(vec![]);

        let mut part = tork::task::TaskLogPart {
            id: Some("part-1".to_string()),
            number: 1,
            task_id: Some("task-1".to_string()),
            contents: Some("line with secret123 and more".to_string()),
            created_at: None,
        };

        let mut secrets = std::collections::HashMap::new();
        secrets.insert("my_secret".to_string(), "secret123".to_string());

        redacter.redact_task_log_part(&mut part, &secrets);

        assert_eq!(
            part.contents,
            Some("line with [REDACTED] and more".to_string())
        );
    }
}
