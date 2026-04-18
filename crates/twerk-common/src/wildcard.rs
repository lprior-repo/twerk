/// Returns true if the pattern contains wildcard characters (*).
#[must_use]
pub fn is_wild_pattern(pattern: &str) -> bool {
    pattern.contains('*')
}

/// Matches a string against a wildcard pattern where `*` matches any sequence
#[must_use]
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

/// Alias for `wildcard_match` for Go parity.
#[must_use]
pub fn match_pattern(pattern: &str, s: &str) -> bool {
    wildcard_match(pattern, s)
}

/// Match is an alias for `wildcard_match` (Go's `Match` -> Rust's `match`).
#[must_use]
pub fn match_wildcard(pattern: &str, s: &str) -> bool {
    wildcard_match(pattern, s)
}

/// Wrapper for match that takes pattern and string (Go's Match function)
/// Note: using r#match to escape Rust's match keyword
#[must_use]
pub fn r#match(pattern: &str, s: &str) -> bool {
    wildcard_match(pattern, s)
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prop_assert;
    use proptest::prop_assert_eq;
    use proptest::prop_assume;

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

    proptest::proptest! {
        #[test]
        fn test_wildcard_match_deterministic(pattern: String, s: String) {
            let result1 = wildcard_match(&pattern, &s);
            let result2 = wildcard_match(&pattern, &s);
            prop_assert_eq!(result1, result2, "wildcard_match must be deterministic");
        }

        #[test]
        fn test_wildcard_match_reflexivity(s: String) {
            prop_assume!(!s.contains('*'), "s must not contain wildcard character");
            prop_assert!(wildcard_match(&s, &s), "non-wildcard string should match itself");
        }

        #[test]
        fn test_wildcard_match_star_at_boundaries(s: String) {
            let pattern_start = format!("*{}", s);
            let pattern_end = format!("{}*", s);
            let pattern_both = format!("*{}*", s);
            prop_assert!(wildcard_match(&pattern_start, &s), "star at start should match");
            prop_assert!(wildcard_match(&pattern_end, &s), "star at end should match");
            prop_assert!(wildcard_match(&pattern_both, &s), "stars at both boundaries should match");
        }

        #[test]
        fn test_wildcard_match_empty_string(pattern: String) {
            if pattern.is_empty() {
                prop_assert!(wildcard_match(&pattern, ""), "empty pattern should match empty string");
            } else if pattern == "*" {
                prop_assert!(wildcard_match(&pattern, ""), "star pattern should match empty string");
            } else if !pattern.contains('*') {
                prop_assert!(!wildcard_match(&pattern, ""), "non-star pattern should not match empty string");
            }
        }

        #[test]
        fn test_wildcard_match_star_matches_anything(pattern: String, s: String) {
            let star_pattern = "*";
            prop_assert!(wildcard_match(star_pattern, &s), "star should match anything");
            prop_assert!(wildcard_match(star_pattern, ""), "star should match empty string");
        }
    }
}
