//! Expression evaluation module — 100% parity with Go `internal/eval`.
//!
//! Provides template evaluation with `{{ expression }}` syntax and
//! support for built-in functions like `randomInt()` and `sequence()`.
//! Uses the `evalexpr` crate for expression evaluation support.

use evalexpr::{
    eval_with_context, ContextWithMutableFunctions, ContextWithMutableVariables, HashMapContext,
    Value,
};
use regex::Regex;
use std::collections::HashMap;
use thiserror::Error;

/// Errors that can occur during evaluation.
#[derive(Debug, Error, PartialEq)]
pub enum EvalError {
    #[error("error compiling expression '{0}': {1}")]
    CompileError(String, String),

    #[error("error evaluating expression '{0}': {1}")]
    ExpressionError(String, String),

    #[error("invalid expression: {0}")]
    InvalidExpression(String),

    #[error("unsupported function: {0}")]
    UnsupportedFunction(String),
}

fn get_template_regex() -> Result<Regex, EvalError> {
    Regex::new(r"\{\{\s*(.+?)\s*\}\}")
        .map_err(|e| EvalError::CompileError("template_regex".into(), e.to_string()))
}

// ---------------------------------------------------------------------------
// Built-in functions
// ---------------------------------------------------------------------------

fn random_int_fn(args: &Value) -> Result<Value, String> {
    let max_opt = match args.as_tuple() {
        Ok(tuple) => match tuple.len() {
            0 => None,
            1 => Some(
                tuple[0]
                    .as_int()
                    .map_err(|_| "randomInt requires a numeric argument")?,
            ),
            n => return Err(format!("randomInt expects 0 or 1 arguments, got {n}")),
        },
        Err(_) => match args {
            Value::Empty => None,
            Value::Int(n) => Some(*n),
            _ => return Err("randomInt requires a numeric argument".into()),
        },
    };

    match max_opt {
        None | Some(0) => {
            let val = rand::random::<i64>();
            Ok(Value::Int(
                i64::try_from(val.unsigned_abs()).map_or(i64::MAX, |v| v),
            ))
        }
        Some(max) if max > 0 => {
            let val = rand::random::<i64>();
            let val_abs = val.unsigned_abs();
            let result = val_abs % u64::try_from(max).map_or(u64::MAX, |v| v);
            Ok(Value::Int(i64::try_from(result).map_or(i64::MAX, |v| v)))
        }
        Some(_) => Ok(Value::Int(0)),
    }
}

fn sequence_fn(args: &Value) -> Result<Value, String> {
    let tuple = args
        .as_tuple()
        .map_err(|_| "sequence expects tuple arguments".to_string())?;

    if tuple.len() != 2 {
        return Err(format!("sequence expects 2 arguments, got {}", tuple.len()));
    }

    let start = tuple[0]
        .as_int()
        .map_err(|_| "sequence requires numeric arguments".to_string())?;
    let stop = tuple[1]
        .as_int()
        .map_err(|_| "sequence requires numeric arguments".to_string())?;

    let range = if start >= stop {
        Vec::new()
    } else {
        (start..stop).map(Value::Int).collect()
    };

    Ok(Value::Tuple(range))
}

fn from_json_fn(args: &Value) -> Result<Value, String> {
    let s = match args.as_tuple() {
        Ok(tuple) => {
            if tuple.is_empty() {
                return Err("fromJSON requires a string argument".to_string());
            }
            tuple[0].as_string()
        }
        Err(_) => args.as_string(),
    }
    .map_err(|_| "fromJSON requires a string argument".to_string())?;
    let parsed: serde_json::Value =
        serde_json::from_str(&s).map_err(|e| format!("fromJSON parse error: {}", e))?;
    json_to_eval_value(&parsed).map_err(|e| format!("fromJSON conversion error: {}", e))
}

fn split_fn(args: &Value) -> Result<Value, String> {
    let tuple = args
        .as_tuple()
        .map_err(|_| "split expects tuple arguments".to_string())?;

    if tuple.len() != 2 {
        return Err(format!("split expects 2 arguments, got {}", tuple.len()));
    }

    let s = tuple[0]
        .as_string()
        .map_err(|_| "split requires string arguments".to_string())?;
    let delimiter = tuple[1]
        .as_string()
        .map_err(|_| "split requires string arguments".to_string())?;

    let parts: Vec<Value> = s
        .split(delimiter.as_str())
        .map(|p| Value::String(p.to_string()))
        .collect();
    Ok(Value::Tuple(parts))
}

fn to_json_fn(args: &Value) -> Result<Value, String> {
    let json = eval_value_to_json(args);
    serde_json::to_string(&json)
        .map(Value::String)
        .map_err(|e| format!("toJSON error: {}", e))
}

// ---------------------------------------------------------------------------
// Context building
// ---------------------------------------------------------------------------

fn create_context(
    context: &HashMap<String, serde_json::Value>,
) -> Result<HashMapContext, EvalError> {
    let mut ctx = HashMapContext::new();

    // Register randomInt function
    let random_int_func = evalexpr::Function::new(|args: &Value| {
        random_int_fn(args).map_err(evalexpr::EvalexprError::CustomMessage)
    });
    ctx.set_function("randomInt".to_string(), random_int_func)
        .map_err(|e: evalexpr::EvalexprError| {
            EvalError::ExpressionError("randomInt".into(), e.to_string())
        })?;

    // Register sequence function
    let sequence_func = evalexpr::Function::new(|args| {
        sequence_fn(args).map_err(evalexpr::EvalexprError::CustomMessage)
    });
    ctx.set_function("sequence".to_string(), sequence_func)
        .map_err(|e: evalexpr::EvalexprError| {
            EvalError::ExpressionError("sequence".into(), e.to_string())
        })?;

    // Register fromJSON function
    let from_json_func = evalexpr::Function::new(|args| {
        from_json_fn(args).map_err(evalexpr::EvalexprError::CustomMessage)
    });
    ctx.set_function("fromJSON".to_string(), from_json_func)
        .map_err(|e: evalexpr::EvalexprError| {
            EvalError::ExpressionError("fromJSON".into(), e.to_string())
        })?;

    // Register split function
    let split_func = evalexpr::Function::new(|args| {
        split_fn(args).map_err(evalexpr::EvalexprError::CustomMessage)
    });
    ctx.set_function("split".to_string(), split_func)
        .map_err(|e: evalexpr::EvalexprError| {
            EvalError::ExpressionError("split".into(), e.to_string())
        })?;

    // Register toJSON function
    let to_json_func = evalexpr::Function::new(|args| {
        to_json_fn(args).map_err(evalexpr::EvalexprError::CustomMessage)
    });
    ctx.set_function("toJSON".to_string(), to_json_func)
        .map_err(|e: evalexpr::EvalexprError| {
            EvalError::ExpressionError("toJSON".into(), e.to_string())
        })?;

    // Add context variables
    for (key, value) in context {
        let eval_value = json_to_eval_value(value)?;
        ctx.set_value(key.clone(), eval_value)
            .map_err(|e: evalexpr::EvalexprError| {
                EvalError::ExpressionError(key.clone(), e.to_string())
            })?;
    }

    Ok(ctx)
}

fn json_to_eval_value(json: &serde_json::Value) -> Result<Value, EvalError> {
    match json {
        serde_json::Value::Null => Ok(Value::Empty),
        serde_json::Value::Bool(b) => Ok(Value::Boolean(*b)),
        serde_json::Value::Number(n) => {
            if let Some(i) = n.as_i64() {
                Ok(Value::Int(i))
            } else if let Some(f) = n.as_f64() {
                Ok(Value::Float(f))
            } else {
                Err(EvalError::InvalidExpression(
                    "unsupported number type".into(),
                ))
            }
        }
        serde_json::Value::String(s) => Ok(Value::String(s.clone())),
        serde_json::Value::Array(arr) => {
            let values: Result<Vec<Value>, EvalError> =
                arr.iter().map(json_to_eval_value).collect();
            Ok(Value::Tuple(values?))
        }
        serde_json::Value::Object(obj) => {
            let pairs: Result<Vec<Value>, EvalError> = obj
                .iter()
                .map(|(k, v)| {
                    let val = json_to_eval_value(v)?;
                    Ok(Value::Tuple(vec![Value::String(k.clone()), val]))
                })
                .collect();
            Ok(Value::Tuple(pairs?))
        }
    }
}

fn eval_value_to_json(value: &Value) -> serde_json::Value {
    match value {
        Value::Empty => serde_json::Value::Null,
        Value::Boolean(b) => serde_json::Value::Bool(*b),
        Value::Int(i) => serde_json::json!(*i),
        Value::Float(f) => serde_json::json!(*f),
        Value::String(s) => serde_json::Value::String(s.clone()),
        Value::Tuple(items) => {
            serde_json::Value::Array(items.iter().map(eval_value_to_json).collect())
        }
    }
}

// ---------------------------------------------------------------------------
// Sanitization & operator transforms
// ---------------------------------------------------------------------------

fn sanitize_expr(expr: &str) -> String {
    let trimmed = expr.trim();
    let re = match get_template_regex() {
        Ok(r) => r,
        Err(_) => return trimmed.to_string(),
    };
    let without_braces = re
        .captures(trimmed)
        .map_or_else(|| trimmed.to_string(), |caps| caps[1].trim().to_string());
    transform_operators(&without_braces)
}

fn transform_operators(expr: &str) -> String {
    expr.replace(" and ", " && ").replace(" or ", " || ")
}

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

#[allow(clippy::too_many_lines, clippy::implicit_hasher)]
pub fn evaluate_task(
    task: &crate::task::Task,
    context: &HashMap<String, serde_json::Value>,
) -> Result<crate::task::Task, EvalError> {
    // Evaluate an optional string field through the template engine.
    let eval_field = |field: &Option<String>| -> Result<Option<String>, EvalError> {
        field
            .as_ref()
            .map_or(Ok(None), |s| evaluate_template(s, context).map(Some))
    };

    // Evaluate a map of string → string through the template engine.
    let eval_map =
        |map: &Option<HashMap<String, String>>| -> Result<HashMap<String, String>, EvalError> {
            match map.as_ref() {
                Some(m) => m
                    .iter()
                    .map(|(k, v)| {
                        let result = evaluate_template(v, context)?;
                        Ok((k.clone(), result))
                    })
                    .collect(),
                None => Ok(HashMap::new()),
            }
        };

    // Evaluate a list of tasks recursively.
    let eval_tasks =
        |tasks: &Option<Vec<crate::task::Task>>| -> Result<Option<Vec<crate::task::Task>>, EvalError> {
            tasks
                .as_ref()
                .map(|ts| {
                    ts.iter()
                        .map(|t| evaluate_task(t, context))
                        .collect::<Result<Vec<_>, _>>()
                })
                .transpose()
        };

    // -- String fields --
    // NOTE: Go does NOT evaluate the `run` field - it's treated as raw shell command
    let name = eval_field(&task.name)?;
    let var = eval_field(&task.var)?;
    let image = eval_field(&task.image)?;
    let run = task.run.clone();
    let queue = eval_field(&task.queue)?;
    let r#if = eval_field(&task.r#if)?;

    // -- Maps --
    let env = eval_map(&task.env)?;
    let files = eval_map(&task.files)?;

    // -- Task lists (recursive) --
    let pre = eval_tasks(&task.pre)?;
    let post = eval_tasks(&task.post)?;
    let sidecars = eval_tasks(&task.sidecars)?;

    // -- Parallel --
    let parallel = task
        .parallel
        .as_ref()
        .map(|par| {
            let tasks = eval_tasks(&par.tasks)?;
            Ok(crate::task::ParallelTask {
                tasks,
                completions: par.completions,
            })
        })
        .transpose()?;

    // -- Each --
    let each = task
        .each
        .as_ref()
        .map(|each| {
            let var = eval_field(&each.var)?;
            let list = eval_field(&each.list)?;
            let inner_task = each
                .task
                .as_ref()
                .map(|t| evaluate_task(t, context))
                .transpose()?;
            Ok(crate::task::EachTask {
                var,
                list,
                task: inner_task.map(Box::new),
                size: each.size,
                completions: each.completions,
                concurrency: each.concurrency,
                index: each.index,
            })
        })
        .transpose()?;

    // -- CMD --
    let cmd = task
        .cmd
        .as_ref()
        .map(|cmd| {
            cmd.iter()
                .map(|s| evaluate_template(s, context))
                .collect::<Result<Vec<_>, _>>()
        })
        .transpose()?;

    // -- Entrypoint --
    let entrypoint = task
        .entrypoint
        .as_ref()
        .map(|ep| {
            ep.iter()
                .map(|s| evaluate_template(s, context))
                .collect::<Result<Vec<_>, _>>()
        })
        .transpose()?;

    // -- SubJob --
    let subjob = task
        .subjob
        .as_ref()
        .map(|sj| {
            let subjob_name = eval_field(&sj.name)?;

            let inputs = eval_map(&sj.inputs)?;
            let secrets = eval_map(&sj.secrets)?;

            // Evaluate subjob tasks recursively (Go parity)
            let subjob_tasks = eval_tasks(&sj.tasks)?;

            let webhooks = sj
                .webhooks
                .as_ref()
                .map(|whs| {
                    whs.iter()
                        .map(|wh| {
                            let url = eval_field(&wh.url)?;
                            let headers = eval_map(&wh.headers)?;
                            let wh_if = eval_field(&wh.r#if)?;
                            Ok(crate::webhook::Webhook {
                                url,
                                headers: if headers.is_empty() {
                                    None
                                } else {
                                    Some(headers)
                                },
                                event: wh.event.clone(),
                                r#if: wh_if,
                            })
                        })
                        .collect::<Result<Vec<_>, _>>()
                })
                .transpose()?;

            Ok(crate::task::SubJobTask {
                id: sj.id.clone(),
                name: subjob_name,
                description: sj.description.clone(),
                tasks: subjob_tasks,
                inputs: if inputs.is_empty() {
                    None
                } else {
                    Some(inputs)
                },
                secrets: if secrets.is_empty() {
                    None
                } else {
                    Some(secrets)
                },
                auto_delete: sj.auto_delete.clone(),
                output: sj.output.clone(),
                detached: sj.detached,
                webhooks: webhooks.filter(|w| !w.is_empty()),
            })
        })
        .transpose()?;

    // -- Assemble new task (structural copy) --
    Ok(crate::task::Task {
        id: task.id.clone(),
        job_id: task.job_id.clone(),
        parent_id: task.parent_id.clone(),
        position: task.position,
        name,
        description: task.description.clone(),
        state: task.state.clone(),
        created_at: task.created_at,
        scheduled_at: task.scheduled_at,
        started_at: task.started_at,
        completed_at: task.completed_at,
        failed_at: task.failed_at,
        cmd,
        entrypoint,
        run,
        image,
        registry: task.registry.clone(),
        env: if env.is_empty() { None } else { Some(env) },
        files: if files.is_empty() { None } else { Some(files) },
        queue,
        redelivered: task.redelivered,
        error: task.error.clone(),
        pre,
        post,
        sidecars,
        mounts: task.mounts.clone(),
        networks: task.networks.clone(),
        node_id: task.node_id.clone(),
        retry: task.retry.clone(),
        limits: task.limits.clone(),
        timeout: task.timeout.clone(),
        result: task.result.clone(),
        var,
        r#if,
        parallel,
        each: each.map(Box::new),
        subjob,
        gpus: task.gpus.clone(),
        tags: task.tags.clone(),
        workdir: task.workdir.clone(),
        priority: task.priority,
        progress: task.progress,
        probe: task.probe.clone(),
    })
}

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

#[must_use]
pub fn valid_expr(expr: &str) -> bool {
    let sanitized = sanitize_expr(expr);
    if sanitized.is_empty() {
        return false;
    }
    let context = HashMap::new();
    evaluate_expr(&sanitized, &context).is_ok()
}

pub fn evaluate_condition(expr: &str, summary: &crate::job::JobSummary) -> Result<bool, String> {
    let mut context = HashMap::new();
    context.insert(
        "job_state".to_string(),
        serde_json::Value::String(summary.state.to_string()),
    );
    context.insert(
        "job_id".to_string(),
        serde_json::json!(summary.id.as_deref().map_or("", |id| id)),
    );
    if let Some(name) = &summary.name {
        context.insert(
            "job_name".to_string(),
            serde_json::Value::String(name.to_string()),
        );
    }
    if let Some(error) = &summary.error {
        context.insert(
            "job_error".to_string(),
            serde_json::Value::String(error.to_string()),
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
                    "expression did not evaluate to a boolean, got: {}",
                    json_val
                )),
            }
        }
        Err(e) => Err(format!("expression evaluation failed: {}", e)),
    }
}

pub fn evaluate_task_condition(
    expr: &str,
    task_summary: &crate::task::TaskSummary,
    job_summary: &crate::job::JobSummary,
) -> Result<bool, String> {
    let mut context = HashMap::new();

    // Flattened job context
    context.insert(
        "job_state".to_string(),
        serde_json::Value::String(job_summary.state.to_string()),
    );
    context.insert(
        "job_id".to_string(),
        serde_json::json!(job_summary.id.as_deref().map_or("", |id| id)),
    );

    // Flattened task context
    context.insert(
        "task_state".to_string(),
        serde_json::Value::String(task_summary.state.to_string()),
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
                    "expression did not evaluate to a boolean, got: {}",
                    json_val
                )),
            }
        }
        Err(e) => Err(format!("expression evaluation failed: {}", e)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::job::JobSummary;
    use crate::task::TaskSummary;

    fn make_job_summary(state: &str) -> JobSummary {
        JobSummary {
            id: Some(crate::id::JobId::new("job-123")),
            name: Some("test-job".to_string()),
            state: state.to_string(),
            error: None,
            ..Default::default()
        }
    }

    fn make_task_summary(state: &str) -> TaskSummary {
        TaskSummary {
            id: Some(crate::id::TaskId::new("task-456")),
            job_id: Some(crate::id::JobId::new("job-123")),
            state: state.to_string(),
            ..Default::default()
        }
    }

    mod evaluate_condition_tests {
        use super::*;

        #[test]
        fn test_empty_expression_returns_true() {
            let summary = make_job_summary("running");
            assert_eq!(evaluate_condition("", &summary), Ok(true));
        }

        #[test]
        fn test_simple_state_comparison() {
            let summary = make_job_summary("running");
            assert_eq!(
                evaluate_condition("job_state == \"running\"", &summary),
                Ok(true)
            );
            assert_eq!(
                evaluate_condition("job_state == \"completed\"", &summary),
                Ok(false)
            );
        }

        #[test]
        fn test_job_state_inequality() {
            let summary = make_job_summary("failed");
            assert_eq!(
                evaluate_condition("job_state != \"running\"", &summary),
                Ok(true)
            );
            assert_eq!(
                evaluate_condition("job_state != \"failed\"", &summary),
                Ok(false)
            );
        }

        #[test]
        fn test_job_id_comparison() {
            let summary = make_job_summary("running");
            assert_eq!(
                evaluate_condition("job_id == \"job-123\"", &summary),
                Ok(true)
            );
            assert_eq!(
                evaluate_condition("job_id == \"other\"", &summary),
                Ok(false)
            );
        }

        #[test]
        fn test_job_name_comparison() {
            let summary = make_job_summary("running");
            assert_eq!(
                evaluate_condition("job_name == \"test-job\"", &summary),
                Ok(true)
            );
            assert_eq!(
                evaluate_condition("job_name == \"other\"", &summary),
                Ok(false)
            );
        }

        #[test]
        fn test_logical_and_operator() {
            let summary = make_job_summary("running");
            let result = evaluate_condition(
                "job_state == \"running\" and job_id == \"job-123\"",
                &summary,
            );
            assert_eq!(result, Ok(true));

            let result =
                evaluate_condition("job_state == \"running\" and job_id == \"other\"", &summary);
            assert_eq!(result, Ok(false));
        }

        #[test]
        fn test_logical_or_operator() {
            let summary = make_job_summary("running");
            let result = evaluate_condition(
                "job_state == \"completed\" or job_id == \"job-123\"",
                &summary,
            );
            assert_eq!(result, Ok(true));

            let result = evaluate_condition(
                "job_state == \"completed\" or job_id == \"other\"",
                &summary,
            );
            assert_eq!(result, Ok(false));
        }

        #[test]
        fn test_boolean_literals() {
            let summary = make_job_summary("running");
            assert_eq!(evaluate_condition("true", &summary), Ok(true));
            assert_eq!(evaluate_condition("false", &summary), Ok(false));
        }

        #[test]
        fn test_parenthesized_expression() {
            let summary = make_job_summary("running");
            let result = evaluate_condition("(job_state == \"running\")", &summary);
            assert_eq!(result, Ok(true));
        }

        #[test]
        fn test_complex_logical_expression() {
            let summary = make_job_summary("running");
            let result = evaluate_condition(
                "(job_state == \"running\" or job_state == \"pending\") and job_id == \"job-123\"",
                &summary,
            );
            assert_eq!(result, Ok(true));
        }

        #[test]
        fn test_job_error_with_error_present() {
            let mut summary = make_job_summary("failed");
            summary.error = Some("something went wrong".to_string());
            assert_eq!(evaluate_condition("job_error != \"\"", &summary), Ok(true));
            assert_eq!(evaluate_condition("job_error == \"\"", &summary), Ok(false));
        }

        #[test]
        fn test_job_error_without_error() {
            let summary = make_job_summary("running");
            let result = evaluate_condition("job_error == \"\"", &summary);
            assert!(result.is_err());
        }

        #[test]
        fn test_evaluate_condition_non_boolean_result() {
            let summary = make_job_summary("running");
            let result = evaluate_condition("\"hello\"", &summary);
            assert!(result.is_err());
            assert!(result
                .unwrap_err()
                .contains("did not evaluate to a boolean"));
        }

        #[test]
        fn test_evaluate_condition_invalid_expression() {
            let summary = make_job_summary("running");
            let result = evaluate_condition("job_state ===", &summary);
            assert!(result.is_err());
        }
    }

    mod evaluate_task_condition_tests {
        use super::*;

        #[test]
        fn test_empty_expression_returns_true() {
            let task_summary = make_task_summary("running");
            let job_summary = make_job_summary("running");
            assert_eq!(
                evaluate_task_condition("", &task_summary, &job_summary),
                Ok(true)
            );
        }

        #[test]
        fn test_task_state_comparison() {
            let task_summary = make_task_summary("running");
            let job_summary = make_job_summary("running");
            assert_eq!(
                evaluate_task_condition("task_state == \"running\"", &task_summary, &job_summary),
                Ok(true)
            );
            assert_eq!(
                evaluate_task_condition("task_state == \"completed\"", &task_summary, &job_summary),
                Ok(false)
            );
        }

        #[test]
        fn test_job_state_in_task_condition() {
            let task_summary = make_task_summary("running");
            let job_summary = make_job_summary("running");
            assert_eq!(
                evaluate_task_condition("job_state == \"running\"", &task_summary, &job_summary),
                Ok(true)
            );
            assert_eq!(
                evaluate_task_condition("job_state == \"failed\"", &task_summary, &job_summary),
                Ok(false)
            );
        }

        #[test]
        fn test_task_id_comparison() {
            let task_summary = make_task_summary("running");
            let job_summary = make_job_summary("running");
            assert_eq!(
                evaluate_task_condition("task_id == \"task-456\"", &task_summary, &job_summary),
                Ok(true)
            );
            assert_eq!(
                evaluate_task_condition("task_id == \"other\"", &task_summary, &job_summary),
                Ok(false)
            );
        }

        #[test]
        fn test_combined_task_and_job_context() {
            let task_summary = make_task_summary("running");
            let job_summary = make_job_summary("running");
            let result = evaluate_task_condition(
                "task_state == \"running\" and job_state == \"running\"",
                &task_summary,
                &job_summary,
            );
            assert_eq!(result, Ok(true));
        }

        #[test]
        fn test_task_and_job_different_states() {
            let task_summary = make_task_summary("completed");
            let job_summary = make_job_summary("running");
            let result = evaluate_task_condition(
                "task_state == \"completed\" and job_state == \"running\"",
                &task_summary,
                &job_summary,
            );
            assert_eq!(result, Ok(true));
        }

        #[test]
        fn test_task_condition_with_failure() {
            let task_summary = make_task_summary("failed");
            let job_summary = make_job_summary("running");
            let result =
                evaluate_task_condition("task_state == \"failed\"", &task_summary, &job_summary);
            assert_eq!(result, Ok(true));
        }

        #[test]
        fn test_task_condition_logical_or() {
            let task_summary = make_task_summary("failed");
            let job_summary = make_job_summary("running");
            let result = evaluate_task_condition(
                "task_state == \"failed\" or job_state == \"failed\"",
                &task_summary,
                &job_summary,
            );
            assert_eq!(result, Ok(true));
        }

        #[test]
        fn test_task_condition_invalid_expression() {
            let task_summary = make_task_summary("running");
            let job_summary = make_job_summary("running");
            let result = evaluate_task_condition("task_state ===", &task_summary, &job_summary);
            assert!(result.is_err());
        }

        #[test]
        fn test_task_condition_non_boolean() {
            let task_summary = make_task_summary("running");
            let job_summary = make_job_summary("running");
            let result = evaluate_task_condition("task_id", &task_summary, &job_summary);
            assert!(result.is_err());
        }
    }

    mod evaluate_expr_tests {
        use super::*;

        fn empty_context() -> HashMap<String, serde_json::Value> {
            HashMap::new()
        }

        #[test]
        fn test_from_json_string() {
            let context = empty_context();
            let result = evaluate_expr(r#"fromJSON("\"hello\"")"#, &context);
            assert_eq!(result.unwrap(), serde_json::json!("hello"));
        }

        #[test]
        fn test_from_json_number() {
            let context = empty_context();
            let result = evaluate_expr(r#"fromJSON("42")"#, &context);
            assert_eq!(result.unwrap(), serde_json::json!(42));
        }

        #[test]
        fn test_from_json_boolean() {
            let context = empty_context();
            let result = evaluate_expr(r#"fromJSON("true")"#, &context);
            assert_eq!(result.unwrap(), serde_json::json!(true));
        }

        #[test]
        fn test_from_json_null() {
            let context = empty_context();
            let result = evaluate_expr(r#"fromJSON("null")"#, &context);
            assert_eq!(result.unwrap(), serde_json::json!(null));
        }

        #[test]
        fn test_from_json_array() {
            let context = empty_context();
            let result = evaluate_expr(r#"fromJSON("[1, 2, 3]")"#, &context);
            assert_eq!(result.unwrap(), serde_json::json!([1, 2, 3]));
        }

        #[test]
        fn test_from_json_object() {
            let context = empty_context();
            let result = evaluate_expr(r#"fromJSON("{\"key\": \"value\"}")"#, &context);
            let expected: serde_json::Value = serde_json::json!([["key", "value"]]);
            assert_eq!(result.unwrap(), expected);
        }

        #[test]
        fn test_split_basic() {
            let context = empty_context();
            let result = evaluate_expr(r#"split("a,b,c", ",")"#, &context);
            let expected: serde_json::Value = serde_json::json!(["a", "b", "c"]);
            assert_eq!(result.unwrap(), expected);
        }

        #[test]
        fn test_split_with_empty_result() {
            let context = empty_context();
            let result = evaluate_expr(r#"split("no-delimiter", ",")"#, &context);
            let expected: serde_json::Value = serde_json::json!(["no-delimiter"]);
            assert_eq!(result.unwrap(), expected);
        }

        #[test]
        fn test_split_multiple_delimiters() {
            let context = empty_context();
            let result = evaluate_expr(r#"split("a-b-c-d", "-")"#, &context);
            let expected: serde_json::Value = serde_json::json!(["a", "b", "c", "d"]);
            assert_eq!(result.unwrap(), expected);
        }

        #[test]
        fn test_to_json_string() {
            let context = empty_context();
            let result = evaluate_expr(r#"toJSON("hello")"#, &context);
            assert_eq!(result.unwrap(), serde_json::json!("\"hello\""));
        }

        #[test]
        fn test_to_json_number() {
            let context = empty_context();
            let result = evaluate_expr("toJSON(42)", &context);
            assert_eq!(result.unwrap(), serde_json::json!("42"));
        }

        #[test]
        fn test_to_json_boolean() {
            let context = empty_context();
            let result = evaluate_expr("toJSON(true)", &context);
            assert_eq!(result.unwrap(), serde_json::json!("true"));
        }

        #[test]
        fn test_to_json_array() {
            let context = empty_context();
            let result = evaluate_expr(r#"toJSON(fromJSON("[1, 2, 3]"))"#, &context);
            assert_eq!(result.unwrap(), serde_json::json!("[1,2,3]"));
        }

        #[test]
        fn test_from_json_and_to_json_chained() {
            let context = empty_context();
            let result = evaluate_expr(r#"toJSON(fromJSON("[1, 2, 3]"))"#, &context);
            assert_eq!(result.unwrap(), serde_json::json!("[1,2,3]"));
        }

        #[test]
        fn test_evaluate_expr_simple_arithmetic() {
            let context = empty_context();
            let result = evaluate_expr("2 + 2", &context);
            assert_eq!(result.unwrap(), serde_json::json!(4));
        }

        #[test]
        fn test_evaluate_expr_with_variables() {
            let mut context = empty_context();
            context.insert("x".to_string(), serde_json::json!(10));
            let result = evaluate_expr("x + 5", &context);
            assert_eq!(result.unwrap(), serde_json::json!(15));
        }

        #[test]
        fn test_evaluate_expr_logical_operators() {
            let context = empty_context();
            let result = evaluate_expr("true and false", &context);
            assert_eq!(result.unwrap(), serde_json::json!(false));
        }

        #[test]
        fn test_evaluate_expr_string_comparison() {
            let context = empty_context();
            let result = evaluate_expr(r#""hello" == "hello""#, &context);
            assert_eq!(result.unwrap(), serde_json::json!(true));
        }
    }

    mod evaluate_template_tests {
        use super::*;

        fn empty_context() -> HashMap<String, serde_json::Value> {
            HashMap::new()
        }

        #[test]
        fn test_template_with_json_functions() {
            let context = empty_context();
            let result = evaluate_template(r#"Value: {{ toJSON(42) }}"#, &context);
            assert_eq!(result.unwrap(), "Value: 42");
        }

        #[test]
        fn test_template_multiple_expressions() {
            let context = empty_context();
            let result =
                evaluate_template(r#"First: {{ "hello" }}, Second: {{ "world" }}"#, &context);
            assert_eq!(result.unwrap(), "First: hello, Second: world");
        }

        #[test]
        fn test_template_no_expression() {
            let context = empty_context();
            let result = evaluate_template("Plain text", &context);
            assert_eq!(result.unwrap(), "Plain text");
        }

        #[test]
        fn test_template_empty_expression() {
            let context = empty_context();
            let result = evaluate_template("{{ }}", &context);
            assert_eq!(result.unwrap(), "null");
        }

        #[test]
        fn test_template_with_variable() {
            let mut context = empty_context();
            context.insert("name".to_string(), serde_json::json!("Alice"));
            let result = evaluate_template(r#"Hello, {{ name }}!"#, &context);
            assert_eq!(result.unwrap(), "Hello, Alice!");
        }

        #[test]
        fn test_template_with_number_variable() {
            let mut context = empty_context();
            context.insert("count".to_string(), serde_json::json!(42));
            let result = evaluate_template(r#"Count: {{ count }}"#, &context);
            assert_eq!(result.unwrap(), "Count: 42");
        }

        #[test]
        fn test_template_trailing_text() {
            let context = empty_context();
            let result = evaluate_template(r#"Start {{ 1 + 1 }} end"#, &context);
            assert_eq!(result.unwrap(), "Start 2 end");
        }

        #[test]
        fn test_template_leading_text() {
            let context = empty_context();
            let result = evaluate_template(r#"Start {{ "test" }}"#, &context);
            assert_eq!(result.unwrap(), "Start test");
        }
    }

    mod valid_expr_tests {
        use super::*;

        #[test]
        fn test_valid_expr_simple() {
            assert!(valid_expr("true"));
            assert!(valid_expr("false"));
        }

        #[test]
        fn test_valid_expr_with_comparison() {
            assert!(valid_expr("5 == 5"));
            assert!(valid_expr("\"hello\" == \"hello\""));
        }

        #[test]
        fn test_valid_expr_empty() {
            assert!(!valid_expr(""));
        }

        #[test]
        fn test_valid_expr_with_whitespace() {
            assert!(valid_expr("  true  "));
        }

        #[test]
        fn test_valid_expr_with_operators() {
            assert!(valid_expr("true and false"));
            assert!(valid_expr("true or false"));
        }
    }

    mod sanitize_expr_tests {
        use super::*;

        #[test]
        fn test_sanitize_removes_braces() {
            let result = sanitize_expr("{{ 1 + 1 }}");
            assert_eq!(result, "1 + 1");
        }

        #[test]
        fn test_sanitize_trims_whitespace() {
            let result = sanitize_expr("{{  1 + 1  }}");
            assert_eq!(result, "1 + 1");
        }

        #[test]
        fn test_sanitize_preserves_plain_expr() {
            let result = sanitize_expr("1 + 1");
            assert_eq!(result, "1 + 1");
        }
    }

    mod transform_operators_tests {
        use super::*;

        #[test]
        fn test_transform_and() {
            let result = transform_operators("a and b");
            assert_eq!(result, "a && b");
        }

        #[test]
        fn test_transform_or() {
            let result = transform_operators("a or b");
            assert_eq!(result, "a || b");
        }

        #[test]
        fn test_transform_preserves_standard_operators() {
            let result = transform_operators("a && b");
            assert_eq!(result, "a && b");
        }
    }
}
