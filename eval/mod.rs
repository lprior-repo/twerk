//! Expression evaluation module.
//!
//! Provides template evaluation with `{{ expression }}` syntax and
//! support for built-in functions like `randomInt()` and `sequence()`.
//! Uses the `evalexpr` crate for full expression evaluation support.

use evalexpr::{eval_with_context, Context, ContextWithMutableFunctions, HashMapContext, Value};
use regex::Regex;
use std::collections::HashMap;
use thiserror::Error;

/// Errors that can occur during evaluation.
#[derive(Debug, Error)]
pub enum EvalError {
    #[error("error evaluating expression '{0}': {1}")]
    ExpressionError(String, String),

    #[error("invalid expression: {0}")]
    InvalidExpression(String),

    #[error("unsupported function: {0}")]
    UnsupportedFunction(String),
}

/// Regular expression to match {{ ... }} template expressions.
/// Compiled once at startup to avoid repeated compilation.
static TEMPLATE_REGEX: std::sync::LazyLock<Regex> =
    std::sync::LazyLock::new(|| Regex::new(r"\{\{\s*(.+?)\s*\}\}").expect("invalid regex"));

/// Built-in function: randomInt([max]) - returns a random integer.
fn random_int_fn(args: &Value) -> Result<Value, String> {
    // Use rand::random() for simplicity - seed from OS entropy
    match args.as_tuple() {
        Ok(tuple) => match tuple.len() {
            0 => Ok(Value::Int(rand::random::<i64>())),
            1 => {
                let max = tuple[0]
                    .as_int()
                    .ok_or_else(|| "randomInt requires a number argument".to_string())?;
                if max > 0 {
                    Ok(Value::Int(rand::random::<u64>() % (max as u64) as i64))
                } else {
                    Ok(Value::Int(0))
                }
            }
            _ => Err(format!(
                "randomInt expects 0 or 1 arguments, got {}",
                tuple.len()
            )),
        },
        Err(_) => {
            // If called without tuple (e.g., just "randomInt" without parentheses)
            Ok(Value::Int(rand::random::<i64>()))
        }
    }
}

/// Built-in function: sequence(start, stop) - returns a sequence of integers.
fn sequence_fn(args: &Value) -> Result<Value, String> {
    let tuple = args
        .as_tuple()
        .map_err(|_| "sequence expects a tuple argument".to_string())?;

    if tuple.len() != 2 {
        return Err(format!("sequence expects 2 arguments, got {}", tuple.len()));
    }

    let start = tuple[0]
        .as_int()
        .ok_or_else(|| "sequence requires number arguments".to_string())?;
    let stop = tuple[1]
        .as_int()
        .ok_or_else(|| "sequence requires number arguments".to_string())?;

    if start > stop {
        Ok(Value::Tuple(vec![]))
    } else {
        Ok(Value::Tuple((start..stop).map(Value::Int).collect()))
    }
}

/// Creates a context with built-in functions registered.
fn create_context(
    context: &HashMap<String, serde_json::Value>,
) -> Result<HashMapContext, EvalError> {
    let mut ctx = HashMapContext::new();

    // Register randomInt function
    let random_int_func = evalexpr::Function::new(|args| {
        random_int_fn(args).map_err(|e| evalexpr::EvalexprError::Unknown(e))
    });
    ctx.set_function("randomInt".to_string(), random_int_func)
        .map_err(|e| EvalError::ExpressionError("randomInt".into(), e.to_string()))?;

    // Register sequence function
    let sequence_func = evalexpr::Function::new(|args| {
        sequence_fn(args).map_err(|e| evalexpr::EvalexprError::Unknown(e))
    });
    ctx.set_function("sequence".to_string(), sequence_func)
        .map_err(|e| EvalError::ExpressionError("sequence".into(), e.to_string()))?;

    // Add context variables
    for (key, value) in context {
        let eval_value = json_to_eval_value(value)?;
        ctx.set_value(key.clone(), eval_value)
            .map_err(|e| EvalError::ExpressionError(key.clone(), e.to_string()))?;
    }

    Ok(ctx)
}

/// Converts a serde_json::Value to an evalexpr::Value.
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
            let values: Result<Vec<Value>, _> = arr.iter().map(json_to_eval_value).collect();
            Ok(Value::Tuple(values?))
        }
        serde_json::Value::Object(obj) => {
            // For objects, we store them as a tuple of key-value pairs
            // or we could use a different approach
            let mut pairs: Vec<Value> = Vec::new();
            for (k, v) in obj {
                let key_value =
                    Value::Tuple(vec![Value::String(k.clone()), json_to_eval_value(v)?]);
                pairs.push(key_value);
            }
            Ok(Value::Tuple(pairs))
        }
    }
}

/// Converts an evalexpr::Value to a serde_json::Value.
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

/// Evaluates a task by replacing all template expressions in its fields.
/// This recursively evaluates pre-tasks, post-tasks, parallel tasks, and subjob.
/// Returns a new Task with evaluated fields, leaving the original unchanged.
#[allow(clippy::too_many_lines)]
pub fn evaluate_task(
    task: &tork::task::Task,
    context: &HashMap<String, serde_json::Value>,
) -> Result<tork::task::Task, EvalError> {
    // Helper to evaluate an optional string field.
    let eval_opt_field = |field: &Option<String>| -> Result<Option<String>, EvalError> {
        field
            .as_ref()
            .map_or_else(|| Ok(None), |s| evaluate_template(s, context).map(Some))
    };

    // Evaluate optional string fields
    let name = eval_opt_field(&task.name)?;
    let var = eval_opt_field(&task.var)?;
    let image = eval_opt_field(&task.image)?;
    let queue = eval_opt_field(&task.queue)?;
    let r#if = eval_opt_field(&task.r#if)?;

    // Evaluate env vars
    let env: HashMap<String, String> = match task.env.as_ref() {
        Some(env) => env
            .iter()
            .map(|(k, v)| {
                let result = evaluate_template(v, context)?;
                Ok((k.clone(), result))
            })
            .collect::<Result<HashMap<_, _>, _>>()?,
        None => HashMap::new(),
    };

    // Evaluate pre-tasks
    let pre = task
        .pre
        .as_ref()
        .map(|pre| {
            pre.iter()
                .map(|p| evaluate_task(p, context))
                .collect::<Result<Vec<_>, _>>()
        })
        .transpose()?;

    // Evaluate post-tasks
    let post = task
        .post
        .as_ref()
        .map(|post| {
            post.iter()
                .map(|p| evaluate_task(p, context))
                .collect::<Result<Vec<_>, _>>()
        })
        .transpose()?;

    // Evaluate parallel tasks
    let parallel = task
        .parallel
        .as_ref()
        .map(|par| {
            let tasks = par
                .tasks
                .as_ref()
                .map(|tasks| {
                    tasks
                        .iter()
                        .map(|t| evaluate_task(t, context))
                        .collect::<Result<Vec<_>, _>>()
                })
                .transpose()?;
            Ok(tork::task::ParallelTask {
                tasks: Some(tasks),
                completions: par.completions,
            })
        })
        .transpose()?;

    // Evaluate cmd
    let cmd = task
        .cmd
        .as_ref()
        .map(|cmd| {
            cmd.iter()
                .map(|s| evaluate_template(s, context))
                .collect::<Result<Vec<_>, _>>()
        })
        .transpose()?;

    // Evaluate subjob
    let subjob = task
        .subjob
        .as_ref()
        .map(|sj| {
            let subjob_name = sj
                .name
                .as_ref()
                .map(|n| evaluate_template(n, context))
                .transpose()?;

            let inputs: HashMap<String, String> = match sj.inputs.as_ref() {
                Some(inputs) => inputs
                    .iter()
                    .map(|(k, v)| {
                        let result = evaluate_template(v, context)?;
                        Ok((k.clone(), result))
                    })
                    .collect::<Result<HashMap<_, _>, _>>()?,
                None => HashMap::new(),
            };

            let secrets: HashMap<String, String> = match sj.secrets.as_ref() {
                Some(secrets) => secrets
                    .iter()
                    .map(|(k, v)| {
                        let result = evaluate_template(v, context)?;
                        Ok((k.clone(), result))
                    })
                    .collect::<Result<HashMap<_, _>, _>>()?,
                None => HashMap::new(),
            };

            let webhooks: Vec<tork::task::Webhook> = match sj.webhooks.as_ref() {
                Some(webhooks) => webhooks
                    .iter()
                    .map(|wh| {
                        let url = wh
                            .url
                            .as_ref()
                            .map(|u| evaluate_template(u, context))
                            .transpose()?;
                        let headers: HashMap<String, String> = match wh.headers.as_ref() {
                            Some(headers) => headers
                                .iter()
                                .map(|(k, v)| {
                                    let result = evaluate_template(v, context)?;
                                    Ok((k.clone(), result))
                                })
                                .collect::<Result<HashMap<_, _>, _>>()?,
                            None => HashMap::new(),
                        };
                        Ok(tork::task::Webhook {
                            url,
                            headers: Some(headers),
                            ..wh.clone()
                        })
                    })
                    .collect::<Result<Vec<_>, _>>()?,
                None => Vec::new(),
            };

            Ok(tork::task::SubJobTask {
                name: subjob_name,
                inputs: Some(inputs),
                secrets: Some(secrets),
                webhooks: Some(webhooks),
                ..sj.clone()
            })
        })
        .transpose()?;

    // Build new task using builder-style or with()
    Ok(tork::task::Task {
        name,
        var,
        image,
        queue,
        env: Some(env),
        pre: Some(pre),
        post: Some(post),
        parallel,
        cmd: Some(cmd),
        subjob,
        r#if,
        ..task.clone()
    })
}

/// Evaluates a template string containing {{ expression }} patterns.
/// Each expression is evaluated and replaced with its result.
pub fn evaluate_template(
    template: &str,
    context: &HashMap<String, serde_json::Value>,
) -> Result<String, EvalError> {
    if template.is_empty() {
        return Ok(String::new());
    }

    let result = TEMPLATE_REGEX.replace_all(template, |caps: &regex::Captures| {
        let expr_str = &caps[1];
        match evaluate_expr(expr_str, context) {
            Ok(val) => format!("{val}"),
            Err(_) => format!("{{{{{}}}}}", expr_str), // Keep original if error
        }
    });

    Ok(result.to_string())
}

/// Evaluates a single expression and returns the result.
pub fn evaluate_expr(
    expr_str: &str,
    context: &HashMap<String, serde_json::Value>,
) -> Result<serde_json::Value, EvalError> {
    let sanitized = sanitize_expr(expr_str);
    if sanitized.is_empty() {
        return Ok(serde_json::Value::Null);
    }

    // Build context with variables and functions
    let ctx = create_context(context)?;

    // Evaluate the expression
    let result = eval_with_context(&sanitized, &ctx)
        .map_err(|e| EvalError::ExpressionError(sanitized.clone(), e.to_string()))?;

    Ok(eval_value_to_json(&result))
}

/// Sanitizes an expression by removing surrounding {{ }} if present.
fn sanitize_expr(expr: &str) -> String {
    let trimmed = expr.trim();
    TEMPLATE_REGEX
        .captures(trimmed)
        .map_or_else(|| trimmed.to_string(), |caps| caps[1].trim().to_string())
}

/// Checks if an expression is valid (can be evaluated without error).
#[allow(clippy::needless_return)]
pub fn valid_expr(expr: &str) -> bool {
    let sanitized = sanitize_expr(expr);
    if sanitized.is_empty() {
        return false;
    }

    let context = HashMap::new();
    evaluate_expr(&sanitized, &context).is_ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_evaluate_template_no_expr() {
        let context = HashMap::new();
        let result = evaluate_template("hello world", &context);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "hello world");
    }

    #[test]
    fn test_evaluate_template_empty() {
        let context = HashMap::new();
        let result = evaluate_template("", &context);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "");
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
        let context = HashMap::new();
        assert_eq!(evaluate_template("{{1 + 2}}", &context).unwrap(), "3");
        assert_eq!(evaluate_template("{{5 - 3}}", &context).unwrap(), "2");
        assert_eq!(evaluate_template("{{4 * 2}}", &context).unwrap(), "8");
        assert_eq!(evaluate_template("{{10 / 2}}", &context).unwrap(), "5");
        assert_eq!(evaluate_template("{{10 % 3}}", &context).unwrap(), "1");
    }

    #[test]
    fn test_comparison_expressions() {
        let context = HashMap::new();
        assert_eq!(evaluate_template("{{1 == 1}}", &context).unwrap(), "true");
        assert_eq!(evaluate_template("{{1 != 2}}", &context).unwrap(), "true");
        assert_eq!(evaluate_template("{{2 > 1}}", &context).unwrap(), "true");
        assert_eq!(evaluate_template("{{1 < 2}}", &context).unwrap(), "true");
        assert_eq!(evaluate_template("{{2 >= 2}}", &context).unwrap(), "true");
        assert_eq!(evaluate_template("{{2 <= 2}}", &context).unwrap(), "true");
    }

    #[test]
    fn test_boolean_expressions() {
        let context = HashMap::new();
        assert_eq!(
            evaluate_template("{{true and true}}", &context).unwrap(),
            "true"
        );
        assert_eq!(
            evaluate_template("{{true or false}}", &context).unwrap(),
            "true"
        );
        assert_eq!(evaluate_template("{{!false}}", &context).unwrap(), "true");
    }

    #[test]
    fn test_variable_access() {
        let mut context = HashMap::new();
        context.insert(
            "name".to_string(),
            serde_json::Value::String("test".to_string()),
        );
        assert_eq!(evaluate_template("{{name}}", &context).unwrap(), "test");
    }

    #[test]
    fn test_sequence_function() {
        let context = HashMap::new();
        let result = evaluate_template("{{sequence(1, 5)}}", &context).unwrap();
        // The result should be a tuple [1, 2, 3, 4]
        assert!(result.contains("1"));
        assert!(result.contains("4"));
    }

    #[test]
    fn test_random_int_function() {
        let context = HashMap::new();
        // Just verify it doesn't panic and returns something
        let result = evaluate_template("{{randomInt(10)}}", &context).unwrap();
        let num: i64 = result.parse().unwrap();
        assert!(num >= 0 && num < 10);
    }
}
