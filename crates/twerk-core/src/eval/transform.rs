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
