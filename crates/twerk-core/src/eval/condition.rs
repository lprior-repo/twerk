//! Condition evaluation.
//!
//! Provides functions for evaluating boolean conditions in job and task contexts.

use evalexpr::eval_with_context;
use std::collections::HashMap;

use super::context::{create_context, eval_value_to_json};
use super::transform::{sanitize_expr, transform_operators};

/// Evaluates a condition expression against a job summary.
///
/// # Arguments
/// * `expr` - The condition expression (e.g., `"job_state == 'COMPLETED'"`)
/// * `summary` - The job summary providing context variables
///
/// # Context Variables
/// - `job_state` - The current job state
/// - `job_id` - The job ID (or empty string if none)
/// - `job_name` - The job name (if set)
/// - `job_error` - The job error message (if set)
///
/// # Returns
/// - `Ok(true)` if the condition evaluates to true
/// - `Ok(false)` if the condition evaluates to false
/// - `Err` if evaluation fails
///
/// # Errors
/// Returns an error if the expression cannot be evaluated or doesn't produce a boolean.
pub fn evaluate_condition(expr: &str, summary: &crate::job::JobSummary) -> Result<bool, String> {
    let mut context = HashMap::new();
    context.insert(
        "job_state".to_string(),
        serde_json::Value::String(summary.state.to_string().to_lowercase()),
    );
    context.insert(
        "job_id".to_string(),
        serde_json::json!(summary.id.as_deref().map_or("", |id| id)),
    );
    if let Some(name) = &summary.name {
        context.insert(
            "job_name".to_string(),
            serde_json::Value::String(name.clone()),
        );
    }
    if let Some(error) = &summary.error {
        context.insert(
            "job_error".to_string(),
            serde_json::Value::String(error.clone()),
        );
    }

    let sanitized = sanitize_expr(expr);
    if sanitized.is_empty() {
        return Ok(true); // Empty expression is treated as true (no condition)
    }

    let transformed = transform_operators(&sanitized);
    let ctx = create_context(&context).map_err(|e| e.to_string())?;

    match eval_with_context(&transformed, &ctx) {
        Ok(val) => {
            let json_val = eval_value_to_json(&val);
            match json_val.as_bool() {
                Some(b) => Ok(b),
                None => Err(format!(
                    "expression did not evaluate to a boolean, got: {json_val}"
                )),
            }
        }
        Err(e) => Err(format!("expression evaluation failed: {e}")),
    }
}

/// Evaluates a condition expression against task and job summaries.
///
/// # Arguments
/// * `expr` - The condition expression
/// * `task_summary` - The task summary providing task context
/// * `job_summary` - The job summary providing job context
///
/// # Context Variables
/// - `job_state` - The current job state
/// - `job_id` - The job ID (or empty string if none)
/// - `task_state` - The current task state
/// - `task_id` - The task ID (or empty string if none)
///
/// # Returns
/// - `Ok(true)` if the condition evaluates to true
/// - `Ok(false)` if the condition evaluates to false
/// - `Err` if evaluation fails
///
/// # Errors
/// Returns an error if the expression cannot be evaluated or doesn't produce a boolean.
pub fn evaluate_task_condition(
    expr: &str,
    task_summary: &crate::task::TaskSummary,
    job_summary: &crate::job::JobSummary,
) -> Result<bool, String> {
    let mut context = HashMap::new();

    // Flattened job context
    context.insert(
        "job_state".to_string(),
        serde_json::Value::String(job_summary.state.to_string().to_lowercase()),
    );
    context.insert(
        "job_id".to_string(),
        serde_json::json!(job_summary.id.as_deref().map_or("", |id| id)),
    );

    // Flattened task context
    context.insert(
        "task_state".to_string(),
        serde_json::Value::String(task_summary.state.to_string().to_lowercase()),
    );
    context.insert(
        "task_id".to_string(),
        serde_json::json!(task_summary.id.as_deref().map_or("", |id| id)),
    );

    let sanitized = sanitize_expr(expr);
    if sanitized.is_empty() {
        return Ok(true); // Empty expression is treated as true (no condition)
    }

    let transformed = transform_operators(&sanitized);
    let ctx = create_context(&context).map_err(|e| e.to_string())?;

    match eval_with_context(&transformed, &ctx) {
        Ok(val) => {
            let json_val = eval_value_to_json(&val);
            match json_val.as_bool() {
                Some(b) => Ok(b),
                None => Err(format!(
                    "expression did not evaluate to a boolean, got: {json_val}"
                )),
            }
        }
        Err(e) => Err(format!("expression evaluation failed: {e}")),
    }
}
