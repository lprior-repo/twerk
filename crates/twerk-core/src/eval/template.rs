//! Template evaluation.
//!
//! Provides functions for evaluating `{{ expression }}` templates
//! and individual expressions within string contexts.

use evalexpr::eval_with_context;
use regex::{Match, Regex};
use std::collections::HashMap;

use super::context::{create_context, eval_value_to_json};
use super::get_template_regex;
use super::transform::sanitize_expr;
use crate::eval::EvalError;

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
///
/// # Errors
/// Returns `EvalError` if the template regex fails to compile or expression evaluation fails.
#[allow(clippy::implicit_hasher)]
pub fn evaluate_template(
    template: &str,
    context: &HashMap<String, serde_json::Value>,
) -> Result<String, EvalError> {
    if template.is_empty() {
        return Ok(String::new());
    }

    let re = get_template_regex()?;
    let matches = collect_template_matches(&re, template);
    if matches.is_empty() {
        return Ok(template.to_string());
    }

    render_template(template, &re, context, &matches).map(|progress| {
        let tail = matches
            .last()
            .map_or("", |last_match| &template[last_match.end()..]);
        progress.buffer + tail
    })
}

#[derive(Default)]
struct RenderProgress {
    buffer: String,
    offset: usize,
}

struct TemplateRenderer<'a> {
    template: &'a str,
    re: &'a Regex,
    context: &'a HashMap<String, serde_json::Value>,
}

fn collect_template_matches<'a>(re: &'a Regex, template: &'a str) -> Vec<Match<'a>> {
    re.find_iter(template).collect()
}

fn render_template(
    template: &str,
    re: &Regex,
    context: &HashMap<String, serde_json::Value>,
    matches: &[Match<'_>],
) -> Result<RenderProgress, EvalError> {
    let renderer = TemplateRenderer {
        template,
        re,
        context,
    };
    matches
        .iter()
        .try_fold(RenderProgress::default(), |progress, matched| {
            renderer.render_match(progress, matched)
        })
}

impl TemplateRenderer<'_> {
    fn render_match(
        &self,
        progress: RenderProgress,
        matched: &Match<'_>,
    ) -> Result<RenderProgress, EvalError> {
        let prefix = template_prefix(self.template, progress.offset, matched.start());
        let replacement = template_replacement(self.re, matched.as_str(), self.context)?;

        Ok(RenderProgress {
            buffer: progress.buffer + &prefix + &replacement,
            offset: matched.end(),
        })
    }
}

fn template_prefix(template: &str, offset: usize, start: usize) -> String {
    if offset < start {
        template[offset..start].to_string()
    } else {
        String::new()
    }
}

fn template_replacement(
    re: &Regex,
    matched: &str,
    context: &HashMap<String, serde_json::Value>,
) -> Result<String, EvalError> {
    let expr_str = extract_match_expression(re, matched)?;

    evaluate_expr(expr_str, context).map(|value| match value {
        serde_json::Value::String(text) => text,
        other => other.to_string(),
    })
}

fn extract_match_expression<'a>(re: &Regex, matched: &'a str) -> Result<&'a str, EvalError> {
    re.captures(matched)
        .and_then(|captures| captures.get(1))
        .map(|capture| capture.as_str())
        .ok_or_else(|| EvalError::InvalidExpression(format!("no capture in match: {matched}")))
}

/// Evaluates a single expression string.
///
/// # Arguments
/// * `expr_str` - The expression to evaluate
/// * `context` - Variable bindings for expression evaluation
///
/// # Returns
/// The result as a JSON value.
///
/// # Errors
/// Returns `EvalError` if expression evaluation fails.
#[allow(clippy::implicit_hasher)]
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
