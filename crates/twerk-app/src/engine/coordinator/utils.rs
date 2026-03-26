//! Utility functions for the coordinator module
//!
//! Includes wildcard matching, base64 decoding, password hashing, and body limit parsing.

#![deny(clippy::unwrap_used)]
#![deny(clippy::expect_used)]
#![deny(clippy::panic)]
#![warn(clippy::pedantic)]

/// Matches a string against a wildcard pattern where `*` matches any sequence
pub fn wildcard_match(pattern: &str, s: &str) -> bool {
    if pattern.is_empty() {
        return s.is_empty();
    }
    if pattern == "*" {
        return true;
    }
    if !pattern.contains('*') {
        return pattern == s;
    }

    // Simple DP approach for wildcard matching
    let pattern_chars: Vec<char> = pattern.chars().collect();
    let s_chars: Vec<char> = s.chars().collect();
    let lp = pattern_chars.len();
    let ls = s_chars.len();

    let mut dp = vec![false; (lp + 1) * (ls + 1)];
    dp[0] = true;

    for i in 0..lp {
        let idx = (i + 1) * (ls + 1);
        dp[idx] = if pattern_chars[i] == '*' {
            dp[i * (ls + 1)]
        } else {
            false
        };
    }

    for (i, &pc) in pattern_chars.iter().enumerate() {
        for j in 0..ls {
            let idx = (i + 1) * (ls + 1) + (j + 1);
            dp[idx] = match pc {
                '*' => {
                    dp[i * (ls + 1) + j] || dp[i * (ls + 1) + (j + 1)] || dp[(i + 1) * (ls + 1) + j]
                }
                _ if pc == s_chars[j] => dp[i * (ls + 1) + j],
                _ => false,
            };
        }
    }

    dp[lp * (ls + 1) + ls]
}

/// Base64 decode helper
pub(crate) fn base64_decode(input: &str) -> Option<String> {
    // Use base64 crate from workspace
    use base64::{engine::general_purpose::STANDARD, Engine};

    STANDARD
        .decode(input)
        .ok()
        .and_then(|bytes| String::from_utf8(bytes).ok())
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

    let multiplier = if s.ends_with('K') {
        1024
    } else if s.ends_with('M') {
        1024 * 1024
    } else if s.ends_with('G') {
        1024 * 1024 * 1024
    } else {
        return s.parse().ok();
    };

    let num_str = &s[..s.len() - 1];
    let num: usize = num_str.parse().ok()?;
    num.checked_mul(multiplier)
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
