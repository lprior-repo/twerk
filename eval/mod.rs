//! Expression evaluation module — 100% parity with Go `internal/eval`.
//!
//! Provides template evaluation with `{{ expression }}` syntax and
//! support for built-in functions like `randomInt()` and `sequence()`.
//! Uses the `evalexpr` crate for expression evaluation support.
//!
//! # Architecture
//!
//! - **Data**: `EvalError`, `Context` types
//! - **Calc**: `evaluate_task`, `evaluate_template`, `evaluate_expr`, built-in functions
//! - **Actions**: None (pure calculation module)

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

/// Regular expression to match `{{ ... }}` template expressions.
/// Matches the Go regex: `\{\{(.+?)\}\}` with added whitespace tolerance.
static TEMPLATE_REGEX: std::sync::LazyLock<Regex> = std::sync::LazyLock::new(|| {
    Regex::new(r"\{\{\s*(.+?)\s*\}\}").expect("invalid regex constant")
});

// ---------------------------------------------------------------------------
// Built-in functions
// ---------------------------------------------------------------------------

/// Built-in function: `randomInt()` or `randomInt(max)`.
///
/// Parity with Go `randomInt(args ...any) (int, error)`:
/// - 0 args → random non-negative integer (like Go `rand.Int()`)
/// - 1 arg  → random integer in `[0, max)` (like Go `rand.Intn(max)`)
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
                i64::try_from(val.unsigned_abs()).unwrap_or(i64::MAX),
            ))
        }
        Some(max) if max > 0 => {
            let val = rand::random::<i64>();
            let val_abs = val.unsigned_abs();
            let result = val_abs % u64::try_from(max).unwrap_or(u64::MAX);
            Ok(Value::Int(i64::try_from(result).unwrap_or(i64::MAX)))
        }
        Some(_) => Ok(Value::Int(0)),
    }
}

/// Built-in function: `sequence(start, stop)` → `[start, start+1, ..., stop-1]`.
///
/// Parity with Go `sequence(start, stop int) []int`:
/// - Returns empty if `start >= stop` (Go checks `start > stop` but result has
///   length `stop-start` which is 0 when equal).
/// - Range is half-open `[start, stop)`.
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

/// Creates an evalexpr context with built-in functions and caller-provided variables.
///
/// Note: `HashMapContext` requires `&mut self` for registration. This is a
/// boundary concern (setting up the evaluation environment), not core logic.
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

/// Converts a [`serde_json::Value`] to an [`evalexpr::Value`].
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

/// Converts an [`evalexpr::Value`] to a [`serde_json::Value`].
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

/// Strips surrounding `{{ }}` from an expression, if present.
/// Parity with Go `sanitizeExpr`.
fn sanitize_expr(expr: &str) -> String {
    let trimmed = expr.trim();
    let without_braces = TEMPLATE_REGEX
        .captures(trimmed)
        .map_or_else(|| trimmed.to_string(), |caps| caps[1].trim().to_string());
    transform_operators(&without_braces)
}

/// Transforms Go/expr-lang boolean keywords to evalexpr operators.
/// `and` → `&&`, `or` → `||`.
fn transform_operators(expr: &str) -> String {
    expr.replace(" and ", " && ").replace(" or ", " || ")
}

// ---------------------------------------------------------------------------
// Public API — full parity with Go `internal/eval`
// ---------------------------------------------------------------------------

/// Evaluates all template expressions in a task's string fields.
///
/// Recursively processes: name, var, image, run, queue, env, if, cmd,
/// pre, post, parallel tasks, sidecars, each task, and subjob
/// (name, inputs, secrets, webhooks, and nested tasks).
///
/// Returns a **new** `Task`; the original is not modified.
/// Parity with Go `EvaluateTask(t *Task, c map[string]any) error`.
///
/// # Errors
///
/// Returns [`EvalError::ExpressionError`] if any template expression
/// fails to compile or evaluate.
#[allow(clippy::too_many_lines, clippy::implicit_hasher)]
pub fn evaluate_task(
    task: &tork::task::Task,
    context: &HashMap<String, serde_json::Value>,
) -> Result<tork::task::Task, EvalError> {
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
        |tasks: &Option<Vec<tork::task::Task>>| -> Result<Option<Vec<tork::task::Task>>, EvalError> {
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
            Ok(tork::task::ParallelTask {
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
            Ok(tork::task::EachTask {
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
                            Ok(tork::task::Webhook {
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

            Ok(tork::task::SubJobTask {
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
    Ok(tork::task::Task {
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
        each,
        subjob,
        gpus: task.gpus.clone(),
        tags: task.tags.clone(),
        workdir: task.workdir.clone(),
        priority: task.priority,
        progress: task.progress,
        probe: task.probe.clone(),
    })
}

/// Evaluates a template string by replacing all `{{ expression }}` patterns.
///
/// Non-template text is passed through unchanged. Each expression is evaluated
/// via [`evaluate_expr`] and its result is interpolated as a string.
///
/// Returns an error immediately if any expression fails (Go parity).
/// Parity with Go `EvaluateTemplate(ex string, c map[string]any) (string, error)`.
///
/// # Errors
///
/// Returns [`EvalError::ExpressionError`] if any expression fails to compile
/// or evaluate, or [`EvalError::InvalidExpression`] if a regex match lacks
/// a capture group.
#[allow(clippy::implicit_hasher)]
pub fn evaluate_template(
    template: &str,
    context: &HashMap<String, serde_json::Value>,
) -> Result<String, EvalError> {
    if template.is_empty() {
        return Ok(String::new());
    }

    let matches = TEMPLATE_REGEX.find_iter(template).collect::<Vec<_>>();

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
            let caps = TEMPLATE_REGEX.captures(m.as_str()).ok_or_else(|| {
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

/// Evaluates a single expression string and returns the result.
///
/// The expression is first sanitized (strips `{{ }}` wrappers if present),
/// then compiled and evaluated with the given context plus built-in functions.
///
/// Parity with Go `EvaluateExpr(ex string, c map[string]any) (any, error)`.
///
/// # Errors
///
/// Returns [`EvalError::ExpressionError`] if the expression fails to compile
/// or evaluate.
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

/// Checks if an expression is valid (can be compiled and evaluated without error).
///
/// Parity with Go `ValidExpr(ex string) bool`.
#[must_use]
pub fn valid_expr(expr: &str) -> bool {
    let sanitized = sanitize_expr(expr);
    if sanitized.is_empty() {
        return false;
    }
    let context = HashMap::new();
    evaluate_expr(&sanitized, &context).is_ok()
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_evaluate_template_no_expr() {
        let ctx = HashMap::new();
        let result = evaluate_template("hello world", &ctx);
        assert_eq!(result, Ok("hello world".to_string()));
    }

    #[test]
    fn test_evaluate_template_empty() {
        let ctx = HashMap::new();
        let result = evaluate_template("", &ctx);
        assert_eq!(result, Ok(String::new()));
    }

    #[test]
    fn test_evaluate_template_single_var() {
        let mut ctx = HashMap::new();
        ctx.insert("name".into(), serde_json::Value::String("world".into()));
        let result = evaluate_template("hello {{name}}", &ctx);
        assert_eq!(result, Ok("hello world".to_string()));
    }

    #[test]
    fn test_evaluate_template_multiple_vars() {
        let mut ctx = HashMap::new();
        ctx.insert("greeting".into(), serde_json::Value::String("hello".into()));
        ctx.insert("target".into(), serde_json::Value::String("world".into()));
        let result = evaluate_template("{{greeting}} {{target}}!", &ctx);
        assert_eq!(result, Ok("hello world!".to_string()));
    }

    #[test]
    fn test_evaluate_template_expr_between_text() {
        let ctx = HashMap::new();
        let result = evaluate_template("result: {{1 + 2}} done", &ctx);
        assert_eq!(result, Ok("result: 3 done".to_string()));
    }

    #[test]
    fn test_valid_expr_true() {
        assert!(valid_expr("1 == 1"));
        assert!(valid_expr("{{1+1}}"));
        assert!(valid_expr("randomInt()"));
    }

    #[test]
    fn test_valid_expr_false() {
        assert!(!valid_expr(""));
        assert!(!valid_expr("   "));
        assert!(!valid_expr("{{}}"));
    }

    #[test]
    fn test_sanitize_expr() {
        assert_eq!(sanitize_expr("{{ 1 + 1 }}"), "1 + 1");
        assert_eq!(sanitize_expr("{{inputs.var}}"), "inputs.var");
        assert_eq!(sanitize_expr("randomInt()"), "randomInt()");
    }

    #[test]
    fn test_math_expressions() {
        let ctx = HashMap::new();
        assert_eq!(evaluate_template("{{1 + 2}}", &ctx).unwrap(), "3");
        assert_eq!(evaluate_template("{{5 - 3}}", &ctx).unwrap(), "2");
        assert_eq!(evaluate_template("{{4 * 2}}", &ctx).unwrap(), "8");
        assert_eq!(evaluate_template("{{10 / 2}}", &ctx).unwrap(), "5");
        assert_eq!(evaluate_template("{{10 % 3}}", &ctx).unwrap(), "1");
    }

    #[test]
    fn test_comparison_expressions() {
        let ctx = HashMap::new();
        assert_eq!(evaluate_template("{{1 == 1}}", &ctx).unwrap(), "true");
        assert_eq!(evaluate_template("{{1 != 2}}", &ctx).unwrap(), "true");
        assert_eq!(evaluate_template("{{2 > 1}}", &ctx).unwrap(), "true");
        assert_eq!(evaluate_template("{{1 < 2}}", &ctx).unwrap(), "true");
        assert_eq!(evaluate_template("{{2 >= 2}}", &ctx).unwrap(), "true");
        assert_eq!(evaluate_template("{{2 <= 2}}", &ctx).unwrap(), "true");
    }

    #[test]
    fn test_boolean_expressions() {
        let ctx = HashMap::new();
        assert_eq!(evaluate_template("{{true && true}}", &ctx).unwrap(), "true");
        assert_eq!(
            evaluate_template("{{true || false}}", &ctx).unwrap(),
            "true"
        );
        assert_eq!(evaluate_template("{{!false}}", &ctx).unwrap(), "true");
    }

    #[test]
    fn test_boolean_keyword_transform() {
        let ctx = HashMap::new();
        // Go-style keywords get transformed to evalexpr operators
        assert_eq!(
            evaluate_template("{{true and true}}", &ctx).unwrap(),
            "true"
        );
        assert_eq!(
            evaluate_template("{{true or false}}", &ctx).unwrap(),
            "true"
        );
    }

    #[test]
    fn test_variable_access() {
        let mut ctx = HashMap::new();
        ctx.insert("name".into(), serde_json::Value::String("test".into()));
        assert_eq!(evaluate_template("{{name}}", &ctx).unwrap(), "test");
    }

    #[test]
    fn test_sequence_function() {
        let ctx = HashMap::new();
        let result = evaluate_template("{{sequence(1, 5)}}", &ctx).unwrap();
        // sequence(1,5) → [1, 2, 3, 4] (half-open range)
        assert!(result.contains("1"));
        assert!(result.contains("2"));
        assert!(result.contains("3"));
        assert!(result.contains("4"));
        // Stop value is exclusive
        assert!(!result.contains("5"));
    }

    #[test]
    fn test_sequence_empty_range() {
        let ctx = HashMap::new();
        let result = evaluate_template("{{sequence(5, 1)}}", &ctx).unwrap();
        assert_eq!(result, "[]");
    }

    #[test]
    fn test_random_int_function_no_arg() {
        let ctx = HashMap::new();
        let result = evaluate_template("{{randomInt()}}", &ctx).unwrap();
        let num: i64 = result
            .parse()
            .expect("randomInt() should return a valid integer");
        assert!(num >= 0);
    }

    #[test]
    fn test_random_int_function_with_arg() {
        let ctx = HashMap::new();
        let result = evaluate_template("{{randomInt(10)}}", &ctx).unwrap();
        let num: i64 = result
            .parse()
            .expect("randomInt(10) should return a valid integer");
        assert!(num >= 0);
        assert!(num < 10);
    }

    #[test]
    fn test_evaluate_task_basic() {
        let mut ctx = HashMap::new();
        ctx.insert(
            "img".into(),
            serde_json::Value::String("ubuntu:22.04".into()),
        );
        ctx.insert("cmd".into(), serde_json::Value::String("echo hello".into()));

        let task = tork::task::Task {
            name: Some("build-{{img}}".into()),
            run: Some("{{cmd}}".into()),
            image: Some("{{img}}".into()),
            ..Default::default()
        };

        let result = evaluate_task(&task, &ctx).unwrap();
        assert_eq!(result.name.as_deref(), Some("build-ubuntu:22.04"));
        assert_eq!(result.run.as_deref(), Some("echo hello"));
        assert_eq!(result.image.as_deref(), Some("ubuntu:22.04"));
    }

    #[test]
    fn test_evaluate_task_recursive_pre() {
        let mut ctx = HashMap::new();
        ctx.insert("pre_cmd".into(), serde_json::Value::String("setup".into()));

        let task = tork::task::Task {
            name: Some("main".into()),
            pre: Some(vec![tork::task::Task {
                name: Some("pre-{{pre_cmd}}".into()),
                run: Some("echo {{pre_cmd}}".into()),
                ..Default::default()
            }]),
            ..Default::default()
        };

        let result = evaluate_task(&task, &ctx).unwrap();
        let pre = result.pre.as_ref().expect("pre should be present");
        assert_eq!(pre.len(), 1);
        assert_eq!(pre[0].name.as_deref(), Some("pre-setup"));
        assert_eq!(pre[0].run.as_deref(), Some("echo setup"));
    }

    #[test]
    fn test_evaluate_task_env_map() {
        let mut ctx = HashMap::new();
        ctx.insert("val".into(), serde_json::Value::String("42".into()));

        let mut env = HashMap::new();
        env.insert("PORT".into(), "{{val}}".into());

        let task = tork::task::Task {
            name: Some("server".into()),
            env: Some(env),
            ..Default::default()
        };

        let result = evaluate_task(&task, &ctx).unwrap();
        let env = result.env.as_ref().expect("env should be present");
        assert_eq!(env.get("PORT").map(String::as_str), Some("42"));
    }

    #[test]
    fn test_evaluate_task_parallel() {
        let mut ctx = HashMap::new();
        ctx.insert("img".into(), serde_json::Value::String("alpine".into()));

        let task = tork::task::Task {
            name: Some("fan-out".into()),
            parallel: Some(tork::task::ParallelTask {
                tasks: Some(vec![
                    tork::task::Task {
                        name: Some("worker-{{img}}".into()),
                        ..Default::default()
                    },
                    tork::task::Task {
                        name: Some("worker-2".into()),
                        ..Default::default()
                    },
                ]),
                completions: 2,
            }),
            ..Default::default()
        };

        let result = evaluate_task(&task, &ctx).unwrap();
        let parallel = result
            .parallel
            .as_ref()
            .expect("parallel should be present");
        let tasks = parallel
            .tasks
            .as_ref()
            .expect("parallel tasks should be present");
        assert_eq!(tasks.len(), 2);
        assert_eq!(tasks[0].name.as_deref(), Some("worker-alpine"));
    }

    #[test]
    fn test_evaluate_task_subjob_tasks() {
        let mut ctx = HashMap::new();
        ctx.insert(
            "var_name".into(),
            serde_json::Value::String("MY_VAR".into()),
        );

        let task = tork::task::Task {
            name: Some("orchestrator".into()),
            subjob: Some(tork::task::SubJobTask {
                name: Some("child-job".into()),
                inputs: Some({
                    let mut m = HashMap::new();
                    m.insert("key".into(), "{{var_name}}".into());
                    m
                }),
                tasks: Some(vec![tork::task::Task {
                    name: Some("inner-{{var_name}}".into()),
                    ..Default::default()
                }]),
                id: None,
                description: None,
                secrets: None,
                auto_delete: None,
                output: None,
                detached: false,
                webhooks: None,
            }),
            ..Default::default()
        };

        let result = evaluate_task(&task, &ctx).unwrap();
        let sj = result.subjob.as_ref().expect("subjob should be present");
        assert_eq!(sj.name.as_deref(), Some("child-job"));
        let inputs = sj.inputs.as_ref().expect("inputs should be present");
        assert_eq!(inputs.get("key").map(String::as_str), Some("MY_VAR"));
        let tasks = sj.tasks.as_ref().expect("subjob tasks should be present");
        assert_eq!(tasks.len(), 1);
        assert_eq!(tasks[0].name.as_deref(), Some("inner-MY_VAR"));
    }

    #[test]
    fn test_evaluate_task_does_not_modify_original() {
        let ctx = HashMap::new();
        let task = tork::task::Task {
            name: Some("before-{{none}}".into()),
            ..Default::default()
        };

        let _result = evaluate_task(&task, &ctx);
        // Original should be unchanged
        assert_eq!(task.name.as_deref(), Some("before-{{none}}"));
    }

    #[test]
    fn test_evaluate_expr_null_for_empty() {
        let ctx = HashMap::new();
        let result = evaluate_expr("", &ctx).unwrap();
        assert!(result.is_null());
    }
}
