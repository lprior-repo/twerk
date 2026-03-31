//! Template evaluation.
//!
//! Provides functions for evaluating `{{ expression }}` templates
//! and individual expressions within string contexts.

use evalexpr::eval_with_context;
use regex::Regex;
use std::collections::HashMap;

use super::context::{create_context, eval_value_to_json};
use super::transform::sanitize_expr;
use crate::eval::EvalError;

fn get_template_regex() -> Result<Regex, EvalError> {
    Regex::new(r"\{\{\s*(.+?)\s*\}\}")
        .map_err(|e| EvalError::CompileError("template_regex".into(), e.to_string()))
}

/// Evaluates a template string containing `{{ expression }}` placeholders.
///
/// # Arguments
/// * `template` - String with optional `{{ expr }}` placeholders
/// * `context` - Variable bindings for expression evaluation
///
/// # Returns
/// The template with all placeholders replaced by their evaluated values.
///
/// # Example
/// - Input: `"Hello {{ name }}!"` with context `{"name": "World"}`
/// - Output: `"Hello World!"`
pub fn evaluate_template(
    template: &str,
    context: &HashMap<String, serde_json::Value>,
) -> Result<String, EvalError> {
    if template.is_empty() {
        return Ok(String::new());
    }

    let re = get_template_regex()?;
    let matches = re.find_iter(template).collect::<Vec<_>>();

    if matches.is_empty() {
        return Ok(template.to_string());
    }

    let result = matches
        .iter()
        .try_fold((String::new(), 0usize), |(buf, loc), m| {
            let start_tag = m.start();
            let end_tag = m.end();
            // Copy text before this match
            let prefix = if loc < start_tag {
                template[loc..start_tag].to_string()
            } else {
                String::new()
            };

            // Extract the expression from the capture group
            let caps = re.captures(m.as_str()).ok_or_else(|| {
                EvalError::InvalidExpression(format!("no capture in match: {}", m.as_str()))
            })?;
            let expr_str = &caps[1];

            // Evaluate the expression
            let val = evaluate_expr(expr_str, context)?;
            let replacement = match &val {
                serde_json::Value::String(s) => s.clone(),
                other => other.to_string(),
            };

            Ok((buf + &prefix + &replacement, end_tag))
        })?;

    // Append any trailing text after the last match
    let output = match matches.last() {
        Some(last_match) => {
            let tail = &template[last_match.end()..];
            result.0 + tail
        }
        None => result.0,
    };

    Ok(output)
}

/// Evaluates a single expression string.
///
/// # Arguments
/// * `expr_str` - The expression to evaluate
/// * `context` - Variable bindings for expression evaluation
///
/// # Returns
/// The result as a JSON value.
pub fn evaluate_expr(
    expr_str: &str,
    context: &HashMap<String, serde_json::Value>,
) -> Result<serde_json::Value, EvalError> {
    let sanitized = sanitize_expr(expr_str);
    if sanitized.is_empty() {
        return Ok(serde_json::Value::Null);
    }

    let ctx = create_context(context)?;

    let result = eval_with_context(&sanitized, &ctx)
        .map_err(|e| EvalError::ExpressionError(sanitized.clone(), e.to_string()))?;

    Ok(eval_value_to_json(&result))
}

/// Checks if an expression is syntactically valid.
///
/// # Arguments
/// * `expr` - The expression to validate
///
/// # Returns
/// `true` if the expression can be parsed and evaluated, `false` otherwise.
#[must_use]
pub fn valid_expr(expr: &str) -> bool {
    let sanitized = sanitize_expr(expr);
    if sanitized.is_empty() {
        return false;
    }
    let context = HashMap::new();
    evaluate_expr(&sanitized, &context).is_ok()
}
