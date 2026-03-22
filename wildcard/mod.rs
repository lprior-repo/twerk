//! Wildcard pattern matching
//!
//! Provides glob-style pattern matching where `*` matches any sequence of characters.

#![deny(clippy::unwrap_used)]
#![deny(clippy::expect_used)]
#![deny(clippy::panic)]
#![warn(clippy::pedantic)]
#![forbid(unsafe_code)]

/// The wildcard character used for pattern matching.
const WILDCARD_CHAR: char = '*';

/// Checks if the pattern contains any wildcard characters.
///
/// # Arguments
/// * `pattern` - The pattern string to check
///
/// # Returns
/// `true` if the pattern contains `*`, `false` otherwise
#[must_use]
pub fn is_wild_pattern(pattern: &str) -> bool {
    pattern.contains(WILDCARD_CHAR)
}

/// Matches a string against a wildcard pattern.
///
/// The `*` wildcard matches any sequence of characters (including empty).
///
/// # Arguments
/// * `pattern` - The wildcard pattern
/// * `s` - The string to match against the pattern
///
/// # Returns
/// `true` if the string matches the pattern, `false` otherwise
#[must_use]
pub fn match_pattern(pattern: &str, s: &str) -> bool {
    // Edge case: single wildcard matches everything
    if pattern == WILDCARD_CHAR.to_string() {
        return true;
    }

    // Edge case: empty pattern matches only empty string
    if pattern.is_empty() {
        return s.is_empty();
    }

    // If pattern has no wildcards, do direct comparison
    if !is_wild_pattern(pattern) {
        return pattern == s;
    }

    // Dynamic programming approach
    // Use iterators for char access to avoid index issues
    let pattern_chars: Vec<char> = pattern.chars().collect();
    let s_chars: Vec<char> = s.chars().collect();
    let lp = pattern_chars.len();
    let ls = s_chars.len();

    // Use a flat vector for the DP table for better cache locality
    // dp[i * (ls + 1) + j] = does pattern[0..i] match s[0..j]
    let mut dp = vec![false; (lp + 1) * (ls + 1)];

    // Base case: empty pattern matches empty string
    dp[0] = true;

    // Initialize first column (pattern vs empty string)
    #[allow(clippy::needless_range_loop)]
    for i in 0..lp {
        let idx = (i + 1) * (ls + 1);
        let prev_idx = i * (ls + 1);
        dp[idx] = if pattern_chars[i] == WILDCARD_CHAR {
            dp[prev_idx]
        } else {
            false
        };
    }

    // Fill the DP table
    for i in 0..lp {
        let pc = pattern_chars[i];
        for j in 0..ls {
            let idx = (i + 1) * (ls + 1) + (j + 1);
            let sc = s_chars[j];
            dp[idx] = match pc {
                WILDCARD_CHAR => {
                    let from_match = dp[i * (ls + 1) + j];
                    let from_pattern_star = dp[i * (ls + 1) + (j + 1)];
                    let from_string_star = dp[(i + 1) * (ls + 1) + j];
                    from_match || from_pattern_star || from_string_star
                }
                _ if pc == sc => dp[i * (ls + 1) + j],
                _ => false,
            };
        }
    }

    dp[lp * (ls + 1) + ls]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_wild_pattern() {
        assert!(is_wild_pattern("*"));
        assert!(is_wild_pattern("**"));
        assert!(!is_wild_pattern("."));
        assert!(!is_wild_pattern("a"));
    }

    #[test]
    fn test_match_pattern_basic() {
        assert!(match_pattern("", ""));
        assert!(match_pattern("*", ""));
        assert!(!match_pattern("", "a"));
        assert!(match_pattern("abc", "abc"));
        assert!(!match_pattern("abc", "ac"));
        assert!(!match_pattern("abc", "abd"));
    }

    #[test]
    fn test_match_pattern_with_star() {
        assert!(match_pattern("a*c", "abc"));
        assert!(match_pattern("a*c", "abcbc"));
        assert!(!match_pattern("a*c", "abcbd"));
        assert!(match_pattern("a*b*c", "ajkembbcldkcedc"));
    }

    #[test]
    fn test_match_pattern_dots() {
        assert!(match_pattern("jobs.*", "jobs.completed"));
        assert!(match_pattern("jobs.*", "jobs.long.completed"));
        assert!(!match_pattern("tasks.*", "jobs.completed"));
        assert!(match_pattern("*.completed", "jobs.completed"));
        assert!(!match_pattern("*.completed.thing", "jobs.completed"));
    }
}
