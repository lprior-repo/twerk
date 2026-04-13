//! Context building utilities.
//!
//! Provides pure functions for converting between JSON and evalexpr values,
//! and for building evaluation contexts with registered functions.

use evalexpr::{ContextWithMutableFunctions, ContextWithMutableVariables, HashMapContext, Value};
use std::collections::HashMap;

use super::functions;
use super::intrinsics;
use crate::eval::EvalError;

/// Converts a JSON value to an evalexpr Value.
///
/// # Type mappings
/// - JSON null → evalexpr Empty
/// - JSON bool → evalexpr Boolean
/// - JSON number → evalexpr Int or Float
/// - JSON string → evalexpr String
/// - JSON array → evalexpr Tuple
/// - JSON object → evalexpr Tuple of (key, value) pairs
///
/// # Errors
/// Returns `EvalError::InvalidExpression` if the number type is unsupported.
pub fn json_to_eval_value(json: &serde_json::Value) -> Result<Value, EvalError> {
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

/// Converts an evalexpr Value to a JSON value.
///
/// # Type mappings
/// - evalexpr Empty → JSON null
/// - evalexpr Boolean → JSON bool
/// - evalexpr Int/Float → JSON number
/// - evalexpr String → JSON string
/// - evalexpr Tuple → JSON array
pub fn eval_value_to_json(value: &Value) -> serde_json::Value {
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

/// Parses a JSON string into evalexpr values.
///
/// # Arguments
/// * `args` - A string containing JSON data
///
/// # Returns
/// The parsed evalexpr Value.
///
/// # Errors
/// Returns an error if the arguments are invalid or JSON parsing fails.
pub fn from_json_fn(args: &Value) -> Result<Value, String> {
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
        serde_json::from_str(&s).map_err(|e| format!("fromJSON parse error: {e}"))?;
    json_to_eval_value(&parsed).map_err(|e| format!("fromJSON conversion error: {e}"))
}

/// Converts an evalexpr value to a JSON string.
///
/// # Arguments
/// * `args` - Any evalexpr value
///
/// # Returns
/// A JSON string representation of the value.
///
/// # Errors
/// Returns an error if JSON serialization fails.
pub fn to_json_fn(args: &Value) -> Result<Value, String> {
    let json = eval_value_to_json(args);
    serde_json::to_string(&json)
        .map(Value::String)
        .map_err(|e| format!("toJSON error: {e}"))
}

/// Creates an evalexpr context with registered built-in functions and variables.
///
/// # Registered functions
/// - `randomInt([max])` - random integer generation
/// - `sequence(start, stop)` - integer range
/// - `fromJSON(string)` - parse JSON string
/// - `split(string, delimiter)` - split string
/// - `toJSON(value)` - serialize to JSON
///
/// # Arguments
/// * `context` - JSON key-value pairs to add as context variables
///
/// # Errors
/// Returns `EvalError` if function registration or variable setting fails.
#[allow(clippy::implicit_hasher)]
pub fn create_context(
    context: &HashMap<String, serde_json::Value>,
) -> Result<HashMapContext, EvalError> {
    let mut ctx = HashMapContext::new();

    // Register randomInt function
    let random_int_func = evalexpr::Function::new(|args: &Value| {
        functions::random_int_fn(args).map_err(evalexpr::EvalexprError::CustomMessage)
    });
    ctx.set_function("randomInt".to_string(), random_int_func)
        .map_err(|e: evalexpr::EvalexprError| {
            EvalError::ExpressionError("randomInt".into(), e.to_string())
        })?;

    // Register sequence function
    let sequence_func = evalexpr::Function::new(|args| {
        functions::sequence_fn(args).map_err(evalexpr::EvalexprError::CustomMessage)
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
        functions::split_fn(args).map_err(evalexpr::EvalexprError::CustomMessage)
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

    // Register ASL intrinsic functions
    register_intrinsics(&mut ctx)?;

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

/// Registers a single intrinsic function on the context.
fn register_one(
    ctx: &mut HashMapContext,
    name: &str,
    f: fn(&Value) -> Result<Value, String>,
) -> Result<(), EvalError> {
    let func = evalexpr::Function::new(move |args| {
        f(args).map_err(evalexpr::EvalexprError::CustomMessage)
    });
    ctx.set_function(name.to_string(), func)
        .map_err(|e: evalexpr::EvalexprError| {
            EvalError::ExpressionError(name.into(), e.to_string())
        })
}

/// Type alias for intrinsic function pointers.
type IntrinsicFn = fn(&Value) -> Result<Value, String>;

/// Registers all ASL intrinsic functions on the context.
fn register_intrinsics(ctx: &mut HashMapContext) -> Result<(), EvalError> {
    let entries: &[(&str, IntrinsicFn)] = &[
        ("format", intrinsics::format_fn),
        ("stringToJson", intrinsics::string_to_json_fn),
        ("jsonToString", intrinsics::json_to_string_fn),
        ("array", intrinsics::array_fn),
        ("mathRandom", intrinsics::math_random_fn),
        ("mathAdd", intrinsics::math_add_fn),
        ("mathSub", intrinsics::math_sub_fn),
        ("uuid", intrinsics::uuid_fn),
        ("hash", intrinsics::hash_fn),
        ("base64Encode", intrinsics::base64_encode_fn),
        ("base64Decode", intrinsics::base64_decode_fn),
        ("arrayPartition", intrinsics::array_partition_fn),
        ("arrayContains", intrinsics::array_contains_fn),
        ("arrayRange", intrinsics::array_range_fn),
        ("arrayLength", intrinsics::array_length_fn),
        ("arrayUnique", intrinsics::array_unique_fn),
    ];
    for &(name, f) in entries {
        register_one(ctx, name, f)?;
    }
    Ok(())
}
