//! Expression evaluation for task conditions.
//!
//! This module provides validation of expressions used in task conditions
//! like `Task.If` and `Job.Output`.

use regex::Regex;
use std::sync::LazyLock;
use thiserror::Error;

/// Errors that can occur during expression validation.
#[derive(Debug, Error)]
pub enum EvalError {
    #[error("error parsing expression '{0}': {1}")]
    ExpressionError(String, String),

    #[error("invalid expression: {0}")]
    InvalidExpression(String),
}

/// Regular expression to match {{ ... }} template expressions.
static TEMPLATE_REGEX: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"\{\{\s*(.+?)\s*\}\}").expect("invalid regex"));

/// Regular expression to detect empty {{ }} templates.
static EMPTY_TEMPLATE_REGEX: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^\s*\{\{\s*\}\}\s*$").expect("invalid regex"));

/// Sanitizes an expression by removing surrounding {{ }} if present.
fn sanitize_expr(expr: &str) -> String {
    let trimmed = expr.trim();
    TEMPLATE_REGEX
        .captures(trimmed)
        .map_or_else(|| trimmed.to_string(), |caps| caps[1].trim().to_string())
}

/// Checks if an expression is valid (can be parsed without error).
/// Returns true if the expression is syntactically correct.
/// Uses basic pattern matching to validate expression syntax.
#[allow(clippy::needless_return)]
pub fn valid_expr(expr_str: &str) -> bool {
    let trimmed = expr_str.trim();

    // Check for empty or whitespace-only input
    if trimmed.is_empty() {
        return false;
    }

    // Check for empty template {{ }}
    if EMPTY_TEMPLATE_REGEX.is_match(trimmed) {
        return false;
    }

    let sanitized = sanitize_expr(expr_str);
    if sanitized.is_empty() {
        return false;
    }

    // Basic syntax validation:
    // 1. Check for balanced parentheses
    // 2. Check for obviously invalid characters

    let parens = sanitized.chars().fold(0i32, |acc, c| match c {
        '(' => acc + 1,
        ')' => acc - 1,
        _ => acc,
    });
    if parens != 0 {
        return false;
    }

    // Check for invalid characters (basic ASCII validation)
    for c in sanitized.chars() {
        if !c.is_ascii_graphic() && !c.is_whitespace() && c != '_' && c != '.' {
            return false;
        }
    }

    true
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_valid_expr_true() {
        assert!(valid_expr("1 == 1"));
        assert!(valid_expr("{{1+1}}"));
        assert!(valid_expr("true and false"));
        assert!(valid_expr("1 + 2 * 3"));
        assert!(valid_expr("inputs.var"));
    }

    #[test]
    fn test_valid_expr_false() {
        assert!(!valid_expr(""));
        assert!(!valid_expr("   "));
        assert!(!valid_expr("{{}}"));
        assert!(!valid_expr("(1 + 2")); // unbalanced
        assert!(!valid_expr("1 + 2)")); // unbalanced
    }

    #[test]
    fn test_sanitize_expr() {
        assert_eq!(sanitize_expr("{{ 1 + 1 }}"), "1 + 1");
        assert_eq!(sanitize_expr("{{inputs.var}}"), "inputs.var");
        assert_eq!(sanitize_expr("randomInt()"), "randomInt()");
        assert_eq!(sanitize_expr("plain text"), "plain text");
    }
}
