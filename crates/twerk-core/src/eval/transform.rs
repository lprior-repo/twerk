//! Expression sanitization and operator transformation.
//!
//! Provides utilities for preparing expressions before evaluation:
//! - Removing template braces
//! - Transforming `and`/`or` to `&&`/`||`

/// Sanitizes an expression by removing template braces and transforming operators.
///
/// # Example
/// - Input: `"{{ foo and bar }}"`
/// - Output: `"foo && bar"`
#[must_use]
pub fn sanitize_expr(expr: &str) -> String {
    let trimmed = expr.trim();
    let without_braces = trimmed
        .strip_prefix("{{")
        .and_then(|value| value.strip_suffix("}}"))
        .map_or_else(
            || trimmed.to_string(),
            |contents| contents.trim().to_string(),
        );
    transform_operators(&without_braces)
}

/// Transforms `and`/`or` operators to `&&`/`||` for evalexpr compatibility.
///
/// # Example
/// - Input: `"a and b or c"`
/// - Output: `"a && b || c"`
#[must_use]
pub fn transform_operators(expr: &str) -> String {
    expr.replace(" and ", " && ").replace(" or ", " || ")
}

#[cfg(test)]
mod proptest_tests {
    use super::*;
    use proptest::prelude::*;

    proptest! {
        #[test]
        fn sanitize_expr_idempotent(s in ".{0,50}") {
            let first = sanitize_expr(&s);
            let second = sanitize_expr(&first);
            prop_assert_eq!(first, second);
        }

        #[test]
        fn transform_operators_idempotent(s in ".{0,50}") {
            let first = transform_operators(&s);
            let second = transform_operators(&first);
            prop_assert_eq!(first, second);
        }

        #[test]
        fn transform_and_preserves_non_operators(s in "[a-z]{1,10}") {
            let input = format!("{} and {}", s, s);
            let result = transform_operators(&input);
            prop_assert!(result.contains("&&"));
            prop_assert!(!result.contains(" and "));
            prop_assert!(result.contains(&s));
        }
    }
}
