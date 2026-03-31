//! Expression sanitization and operator transformation.
//!
//! Provides utilities for preparing expressions before evaluation:
//! - Removing template braces
//! - Transforming `and`/`or` to `&&`/`||`

use regex::Regex;

use crate::eval::EvalError;

fn get_template_regex() -> Result<Regex, EvalError> {
    Regex::new(r"\{\{\s*(.+?)\s*\}\}")
        .map_err(|e| EvalError::CompileError("template_regex".into(), e.to_string()))
}

/// Sanitizes an expression by removing template braces and transforming operators.
///
/// # Example
/// - Input: `"{{ foo and bar }}"`
/// - Output: `"foo && bar"`
#[must_use]
pub fn sanitize_expr(expr: &str) -> String {
    let trimmed = expr.trim();
    let Ok(re) = get_template_regex() else {
        return trimmed.to_string();
    };
    let without_braces = re
        .captures(trimmed)
        .map_or_else(|| trimmed.to_string(), |caps| caps[1].trim().to_string());
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
