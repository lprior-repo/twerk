//! Environment variable helpers.

// ── Timeout/env reading ────────────────────────────────────────────────────────

/// Formats a key-value pair as an environment variable string "key=value".
///
/// # Examples
///
/// ```
/// use twerk_core::env::format_kv;
/// assert_eq!(format_kv("PORT", "8080"), "PORT=8080");
/// ```
#[must_use]
pub fn format_kv(k: &str, v: &str) -> String {
    format!("{k}={v}")
}

/// Reads a u64 timeout value from the environment.
///
/// Returns `default` if the env var is not set or cannot be parsed.
#[must_use]
pub fn read_timeout_env(key: &str, default: u64) -> u64 {
    std::env::var(key)
        .ok()
        .and_then(|v| v.parse().ok())
        .map_or(default, |v| v)
}

/// Reads a boolean cleanup flag from the environment.
///
/// Returns `default` if the env var is not set or has an unrecognized value.
/// Recognized true values: "true", "1" (case-insensitive).
/// All other values are treated as false.
#[must_use]
pub fn read_cleanup_env(key: &str, default: bool) -> bool {
    std::env::var(key)
        .ok()
        .map_or(default, |v| v.eq_ignore_ascii_case("true") || v == "1")
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use std::env;

    // Unique key prefix to avoid collisions between parallel tests
    fn test_key(name: &str) -> String {
        format!("TWERK_CORE_TEST_ENV_{name}")
    }

    fn remove(key: &str) {
        env::remove_var(key);
    }

    fn set(key: &str, val: &str) {
        env::set_var(key, val);
    }

    // ── read_timeout_env ────────────────────────────────────────────────────

    #[test]
    fn read_timeout_env_returns_default_when_unset() {
        let key = test_key("TIMEOUT_UNSET");
        remove(&key);
        let val = read_timeout_env(&key, 42);
        assert_eq!(val, 42, "must return default when env var is not set");
    }

    #[test]
    fn read_timeout_env_parses_valid_u64() {
        let key = test_key("TIMEOUT_VALID");
        remove(&key);
        set(&key, "99");
        let val = read_timeout_env(&key, 0);
        assert_eq!(val, 99, "must parse '99' as u64 99");
        remove(&key);
    }

    #[test]
    fn read_timeout_env_returns_default_on_invalid() {
        let key = test_key("TIMEOUT_INVALID");
        remove(&key);
        set(&key, "not_a_number");
        let val = read_timeout_env(&key, 77);
        assert_eq!(
            val, 77,
            "must return default when env var is not a valid u64"
        );
        remove(&key);
    }

    #[test]
    fn read_timeout_env_parses_zero() {
        let key = test_key("TIMEOUT_ZERO");
        remove(&key);
        set(&key, "0");
        let val = read_timeout_env(&key, 100);
        // Mutation replaces with 0 — default is 100 so if this returns 0 it came from parsing
        assert_eq!(val, 0, "must parse '0' as u64 0, not return default");
        remove(&key);
    }

    #[test]
    fn read_timeout_env_parses_one() {
        let key = test_key("TIMEOUT_ONE");
        remove(&key);
        set(&key, "1");
        let val = read_timeout_env(&key, 100);
        // Mutation replaces with 1 — default is 100 so if this returns 1 it came from parsing
        assert_eq!(val, 1, "must parse '1' as u64 1, not return default");
        remove(&key);
    }

    #[test]
    fn read_timeout_env_parses_large_value() {
        let key = test_key("TIMEOUT_LARGE");
        remove(&key);
        set(&key, "3600");
        let val = read_timeout_env(&key, 30);
        assert_eq!(val, 3600);
        remove(&key);
    }

    // ── read_cleanup_env ───────────────────────────────────────────────────

    #[test]
    fn read_cleanup_env_returns_default_when_unset_true() {
        let key = test_key("CLEANUP_UNSET_T");
        remove(&key);
        let val = read_cleanup_env(&key, true);
        assert!(val, "must return default true when env var is not set");
    }

    #[test]
    fn read_cleanup_env_returns_default_when_unset_false() {
        let key = test_key("CLEANUP_UNSET_F");
        remove(&key);
        let val = read_cleanup_env(&key, false);
        assert!(!val, "must return default false when env var is not set");
    }

    #[test]
    fn read_cleanup_env_true_literal() {
        let key = test_key("CLEANUP_TRUE");
        remove(&key);
        set(&key, "true");
        let val = read_cleanup_env(&key, false);
        // default is false; if true, it came from parsing "true"
        assert!(val, "must return true for 'true'");
        remove(&key);
    }

    #[test]
    fn read_cleanup_env_one_literal() {
        let key = test_key("CLEANUP_ONE");
        remove(&key);
        set(&key, "1");
        let val = read_cleanup_env(&key, false);
        // default is false; if true, it came from parsing "1"
        assert!(val, "must return true for '1'");
        remove(&key);
    }

    #[test]
    fn read_cleanup_env_case_insensitive_true() {
        let key = test_key("CLEANUP_CASE");
        remove(&key);
        set(&key, "TRUE");
        let val = read_cleanup_env(&key, false);
        assert!(val, "must return true for 'TRUE' (case-insensitive)");
        remove(&key);
    }

    #[test]
    fn read_cleanup_env_case_insensitive_true_mixed() {
        let key = test_key("CLEANUP_MIXED");
        remove(&key);
        set(&key, "True");
        let val = read_cleanup_env(&key, false);
        assert!(val, "must return true for 'True' (case-insensitive)");
        remove(&key);
    }

    #[test]
    fn read_cleanup_env_false_for_unrecognized() {
        let key = test_key("CLEANUP_GARBAGE");
        remove(&key);
        set(&key, "yes");
        let val = read_cleanup_env(&key, false);
        assert!(!val, "must return false for unrecognized value 'yes'");
        remove(&key);
    }

    #[test]
    fn read_cleanup_env_false_for_zero() {
        let key = test_key("CLEANUP_ZERO");
        remove(&key);
        set(&key, "0");
        let val = read_cleanup_env(&key, false);
        assert!(!val, "must return false for '0' (only '1' is true)");
        remove(&key);
    }

    #[test]
    fn read_cleanup_env_false_for_empty() {
        let key = test_key("CLEANUP_EMPTY");
        remove(&key);
        set(&key, "");
        let val = read_cleanup_env(&key, false);
        assert!(!val, "must return false for empty string");
        remove(&key);
    }

    #[test]
    fn read_cleanup_env_uses_both_conditions() {
        // This test catches || replaced with &&
        // If "true" || "1" becomes "true" && "1", value "true" would return false
        // and value "1" would also return false since "true" != "1"
        let key = test_key("CLEANUP_OR_CHECK");
        remove(&key);

        // "true" must satisfy the first condition (eq_ignore_ascii_case)
        set(&key, "true");
        assert!(
            read_cleanup_env(&key, false),
            "'true' must match via eq_ignore_ascii_case"
        );
        remove(&key);

        // "1" must satisfy the second condition (==)
        set(&key, "1");
        assert!(read_cleanup_env(&key, false), "'1' must match via ==");
        remove(&key);
    }

    #[test]
    fn read_cleanup_env_eq_not_neq() {
        // Catches == replaced with !=:
        // If == becomes !=, then "1" != "1" is true, so read_cleanup_env returns true
        // BUT also "true" != "1" is true, so that also returns true — no difference.
        // However, "false" != "1" is true, so "false" would incorrectly return true.
        let key = test_key("CLEANUP_NEQ_CHECK");
        remove(&key);
        set(&key, "false");
        let val = read_cleanup_env(&key, false);
        assert!(!val, "'false' must not match '1' via ==");
        remove(&key);
    }

    // ── format_kv ──────────────────────────────────────────────────────────

    #[test]
    fn format_kv_basic() {
        assert_eq!(format_kv("PORT", "8080"), "PORT=8080");
    }
}
