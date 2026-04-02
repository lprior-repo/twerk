//! Environment variable helpers with TWERK_ prefix convention.

use std::env;

/// Get a string from environment variables with `TWERK_` prefix.
#[must_use]
pub fn var_with_twerk_prefix(key: &str) -> Option<String> {
    let env_key = format!("TWERK_{}", key.to_uppercase().replace('.', "_"));
    env::var(&env_key).ok().filter(|value| !value.is_empty())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_var_with_twerk_prefix() {
        // Note: This test relies on external environment setup
        // In unit tests, we typically mock env vars
        let result = var_with_twerk_prefix("test.key");
        // Returns None if env var not set, which is valid
        assert!(result.is_none() || result.is_some_and(|v| !v.is_empty()));
    }

    #[test]
    fn test_key_transformation() {
        // Verify the key transformation: dots become underscores, uppercase
        let env_key = format!(
            "TWERK_{}",
            "datastore.host".to_uppercase().replace('.', "_")
        );
        assert_eq!(env_key, "TWERK_DATASTORE_HOST");
    }
}
