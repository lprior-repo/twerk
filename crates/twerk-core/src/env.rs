//! Environment variable helpers.

// ── Timeout/env reading ────────────────────────────────────────────────────────

/// Formats a key-value pair as an environment variable string "key=value".
///
/// # Examples
///
/// ```
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
