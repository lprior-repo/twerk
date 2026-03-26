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
    let name = eval_field(&task.name)?;
    let var = eval_field(&task.var)?;
    let image = eval_field(&task.image)?;
    let run = eval_field(&task.run)?;
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
