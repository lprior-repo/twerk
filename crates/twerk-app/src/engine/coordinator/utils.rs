//! Utility functions for the coordinator module
//!
//! Includes wildcard matching, base64 decoding, password hashing, and body limit parsing.

#![deny(clippy::unwrap_used)]
#![deny(clippy::expect_used)]
#![deny(clippy::panic)]
#![warn(clippy::pedantic)]

pub use twerk_common::wildcard::wildcard_match;

/// Base64 decode helper
pub(crate) fn base64_decode(input: &str) -> Option<String> {
    // Use base64 crate from workspace
    use base64::{engine::general_purpose::STANDARD, Engine};

    match STANDARD.decode(input) {
        Ok(bytes) => String::from_utf8(bytes).ok(),
        Err(_) => None,
    }
}

/// Check password against bcrypt hash
pub(crate) fn check_password_hash(password: &str, hash: &str) -> bool {
    bcrypt::verify(password, hash).is_ok_and(|r| r)
}

/// Parse body limit string like "500K", "1M", "10M" to bytes
pub(crate) fn parse_body_limit(s: &str) -> Option<usize> {
    let s = s.trim();
    if s.is_empty() {
        return None;
    }

    let (num_str, multiplier) = if let Some(stripped) = s.strip_suffix('K') {
        (stripped, 1024)
    } else if let Some(stripped) = s.strip_suffix('M') {
        (stripped, 1024 * 1024)
    } else if let Some(stripped) = s.strip_suffix('G') {
        (stripped, 1024 * 1024 * 1024)
    } else {
        return s.parse().ok();
    };

    match num_str.parse::<usize>() {
        Ok(num) => num.checked_mul(multiplier),
        Err(_) => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_wildcard_match_exact() {
        assert!(wildcard_match("abc", "abc"));
        assert!(!wildcard_match("abc", "abd"));
    }

    #[test]
    fn test_wildcard_match_star() {
        assert!(wildcard_match("*", "anything"));
        assert!(wildcard_match("a*c", "abc"));
        assert!(wildcard_match("a*c", "aXXc"));
        assert!(!wildcard_match("a*c", "aXXd"));
    }

    #[test]
    fn test_wildcard_match_empty() {
        assert!(wildcard_match("", ""));
        assert!(!wildcard_match("", "a"));
        assert!(!wildcard_match("a", ""));
    }

    #[test]
    fn test_wildcard_match_multiple_stars() {
        assert!(wildcard_match("*:*", "foo:bar"));
        assert!(wildcard_match("a*b*c", "axbxc"));
    }

    #[test]
    fn test_parse_body_limit() {
        assert_eq!(parse_body_limit("500K"), Some(500 * 1024));
        assert_eq!(parse_body_limit("1M"), Some(1024 * 1024));
        assert_eq!(parse_body_limit("1G"), Some(1024 * 1024 * 1024));
        assert_eq!(parse_body_limit("500"), Some(500));
    }

    #[test]
    fn test_parse_body_limit_edge_cases() {
        assert_eq!(parse_body_limit(""), None);
        assert_eq!(parse_body_limit("invalid"), None);
        assert_eq!(parse_body_limit("K"), None); // no number
    }
}
