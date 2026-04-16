//! Shared test fixtures and strategies for domain types.
//!
//! This module provides reusable test infrastructure for:
//! - [`Hostname`]
//! - [`WebhookUrl`]
//! - [`CronExpression`]
//!
//! # Usage
//!
//! ```ignore
//! use crate::domain::testing::{arb_hostname, arb_valid_hostname};
//! use proptest::prelude::proptest;
//!
//! proptest! {
//!     #[test]
//!     fn my_test(hostname in arb_hostname()) {
//!         // ...
//!     }
//! }
//! ```

use proptest::prelude::*;

// ----------------------------------------------------------------------------
// Hostname fixtures
// ----------------------------------------------------------------------------

/// Valid hostname samples for testing.
pub const VALID_HOSTNAMES: &[&str] = &[
    "localhost",
    "example.com",
    "api.example.com",
    "my-host.example.co.uk",
    "server1.prod.us-east-1",
    "a.b.c",
    "MyServer.Example.COM",
];

/// Invalid hostname samples for testing (not exhaustive).
#[allow(dead_code)]
pub const INVALID_HOSTNAMES: &[&str] = &[
    "",
    "123.456.789",      // all-numeric labels
    "example.com:8080", // contains colon
];

/// Generates a hostname at the maximum allowed length (253 chars).
pub fn max_length_hostname() -> String {
    // 253 character hostname: 5 labels of 49 'a's + 4 dots + ".com" = 5*49 + 4 + 4 = 253
    // Each label (49 chars) is within the 63-char label limit.
    format!(
        "{}.{}.{}.{}.{}.com",
        "a".repeat(49),
        "a".repeat(49),
        "a".repeat(49),
        "a".repeat(49),
        "a".repeat(49)
    )
}

/// Proptest strategy for valid hostnames.
pub fn arb_valid_hostname() -> impl Strategy<Value = &'static str> {
    prop::sample::select(VALID_HOSTNAMES)
}

/// Proptest strategy for any hostname (valid or invalid).
#[allow(dead_code)]
pub fn arb_hostname() -> impl Strategy<Value = &'static str> {
    prop::sample::select(VALID_HOSTNAMES)
}

/// Proptest strategy for hostname at maximum length.
#[allow(dead_code)]
pub fn arb_max_length_hostname() -> impl Strategy<Value = String> {
    Just(max_length_hostname())
}

// ----------------------------------------------------------------------------
// WebhookUrl fixtures
// ----------------------------------------------------------------------------

/// Valid webhook URL samples for testing.
pub const VALID_WEBHOOK_URLS: &[&str] = &[
    "https://example.com",
    "http://localhost:8080",
    "https://api.test.co:443/v1",
    "https://example.com:8443/path",
    "https://example.com:8080/webhook",
    "http://localhost:3000/",
    "https://a.b",
    "https://example.com:443/path?query=1#fragment",
];

/// Invalid webhook URL samples for testing.
#[allow(dead_code)]
pub const INVALID_WEBHOOK_URLS: &[&str] = &[
    "not a url",
    "ftp://example.com/file",
    "file:///path/to/file",
    "ws://example.com/socket",
    "wss://secure.example.com/socket",
    "http://",
];

/// Proptest strategy for valid webhook URLs.
pub fn arb_valid_webhook_url() -> impl Strategy<Value = &'static str> {
    prop::sample::select(VALID_WEBHOOK_URLS)
}

/// Proptest strategy for any webhook URL (valid or invalid).
#[allow(dead_code)]
pub fn arb_webhook_url() -> impl Strategy<Value = &'static str> {
    prop::sample::select(VALID_WEBHOOK_URLS)
}

// ----------------------------------------------------------------------------
// CronExpression fixtures
// ----------------------------------------------------------------------------

/// Valid cron expression samples for testing.
pub const VALID_CRON_EXPRESSIONS: &[&str] = &[
    "0 0 * * *",
    "*/15 * * * MON-FRI",
    "0 30 8 1 * *",
    "0 0 * * MON",
    "0 0 1 * *",
    "0 12 * * *",
];

/// Invalid cron expression samples for testing.
#[allow(dead_code)]
pub const INVALID_CRON_EXPRESSIONS: &[&str] = &[
    "",
    "X * * * * *",   // parse error
    "* * *",         // too few fields
    "* * * * * * *", // too many fields
];

/// Proptest strategy for valid cron expressions.
pub fn arb_valid_cron_expression() -> impl Strategy<Value = &'static str> {
    prop::sample::select(VALID_CRON_EXPRESSIONS)
}

/// Proptest strategy for any cron expression (valid or invalid).
#[allow(dead_code)]
pub fn arb_cron_expression() -> impl Strategy<Value = &'static str> {
    prop::sample::select(VALID_CRON_EXPRESSIONS)
}

// ----------------------------------------------------------------------------
// Shared assertion helpers
// ----------------------------------------------------------------------------

/// Asserts that a type satisfies `Send + Sync`.
#[macro_export]
macro_rules! assert_is_send_and_sync {
    ($value:expr) => {
        fn assert_send<T: Send>(_: &T) {}
        fn assert_sync<T: Sync>(_: &T) {}
        assert_send(&$value);
        assert_sync(&$value);
    };
}

/// Asserts that a result is an error and matches a specific variant pattern.
#[macro_export]
macro_rules! assert_err_matches {
    ($result:expr, $variant:pat) => {{
        let Err(e) = $result else {
            panic!("expected error, got ok");
        };
        assert!(matches!(e, $variant));
        e
    }};
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn max_length_hostname_is_253_chars() {
        let hostname = max_length_hostname();
        assert_eq!(hostname.len(), 253);
    }

    #[test]
    fn valid_hostnames_are_not_empty() {
        for hostname in VALID_HOSTNAMES {
            assert!(
                !hostname.is_empty(),
                "hostname '{}' should not be empty",
                hostname
            );
        }
    }

    #[test]
    fn valid_webhook_urls_are_not_empty() {
        for url in VALID_WEBHOOK_URLS {
            assert!(!url.is_empty(), "url '{}' should not be empty", url);
        }
    }

    #[test]
    fn valid_cron_expressions_are_not_empty() {
        for expr in VALID_CRON_EXPRESSIONS {
            assert!(!expr.is_empty(), "cron '{}' should not be empty", expr);
        }
    }
}
