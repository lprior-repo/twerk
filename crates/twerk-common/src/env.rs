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
        let key = "test.key";
        let env_key = format!("TWERK_{}", key.to_uppercase().replace('.', "_"));
        let test_value = "test_value_123";
        env::set_var(&env_key, test_value);
        let result = var_with_twerk_prefix(key);
        assert!(result.is_some());
        assert_eq!(result.unwrap(), test_value);
        env::remove_var(&env_key);
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
