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
}

#[cfg(test)]
mod proptest_tests {
    use super::*;
    use proptest::prelude::*;

    proptest! {
        #[test]
        fn star_matches_anything(s in ".*") {
            prop_assert!(wildcard_match("*", &s));
        }

        #[test]
        fn empty_pattern_only_matches_empty(s in ".*") {
            prop_assert_eq!(wildcard_match("", &s), s.is_empty());
        }

        #[test]
        fn reflexive_for_literal(s in "[^*]*") {
            prop_assert!(wildcard_match(&s, &s));
        }

        #[test]
        fn star_suffix_matches_prefix(prefix in "[a-zA-Z0-9]+", suffix in ".*") {
            let pattern = format!("{}*", prefix);
            let text = format!("{}{}", prefix, suffix);
            prop_assert!(wildcard_match(&pattern, &text));
        }
    }
}
