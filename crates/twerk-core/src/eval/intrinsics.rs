//! ASL intrinsic functions.
//!
//! Provides evaluation functions for:
//! - String: `format`, `stringToJson`, `jsonToString`, `array`
//! - Math: `mathRandom`, `mathAdd`, `mathSub`
//! - Utility: `uuid`, `hash`, `base64Encode`, `base64Decode`
//! - Array: `arrayPartition`, `arrayContains`, `arrayRange`, `arrayLength`, `arrayUnique`

use base64::Engine;
use evalexpr::Value;
use sha2::{Digest, Sha256};

use super::context::{eval_value_to_json, json_to_eval_value};

// ───────────────── helpers ─────────────────

fn extract_tuple(args: &Value, name: &str, expected: usize) -> Result<Vec<Value>, String> {
    let tuple = args
        .as_tuple()
        .map_err(|_| format!("{name} expects tuple arguments"))?;
    if tuple.len() != expected {
        return Err(format!(
            "{name} expects {expected} arguments, got {}",
            tuple.len()
        ));
    }
    Ok(tuple)
}

fn as_numeric(v: &Value, name: &str) -> Result<f64, String> {
    match v {
        Value::Int(i) => Ok(*i as f64),
        Value::Float(f) => Ok(*f),
        _ => Err(format!("{name} requires numeric arguments")),
    }
}

// ───────────────── string functions ─────────────────

pub fn format_fn(args: &Value) -> Result<Value, String> {
    let parts = match args.as_tuple() {
        Ok(t) => t,
        Err(_) => match args {
            Value::String(s) => return Ok(Value::String(s.clone())),
            _ => return Err("format requires at least a template argument".into()),
        },
    };
    if parts.is_empty() {
        return Err("format requires at least a template argument".into());
    }
    let template = parts[0]
        .as_string()
        .map_err(|_| "format: first argument must be a string")?;

    let mut result = String::new();
    let mut arg_idx = 1;
    let mut chars = template.chars().peekable();
    while let Some(ch) = chars.next() {
        if ch == '{' && chars.peek() == Some(&'}') {
            chars.next();
            if arg_idx < parts.len() {
                let json = eval_value_to_json(&parts[arg_idx]);
                match &json {
                    serde_json::Value::String(s) => result.push_str(s),
                    other => result.push_str(&other.to_string()),
                }
                arg_idx += 1;
            } else {
                result.push_str("{}");
            }
        } else {
            result.push(ch);
        }
    }
    Ok(Value::String(result))
}

pub fn string_to_json_fn(args: &Value) -> Result<Value, String> {
    let s = match args.as_tuple() {
        Ok(t) if !t.is_empty() => t[0].as_string(),
        _ => args.as_string(),
    }
    .map_err(|_| "stringToJson requires a string argument")?;

    let parsed: serde_json::Value =
        serde_json::from_str(&s).map_err(|e| format!("stringToJson parse error: {e}"))?;
    json_to_eval_value(&parsed).map_err(|e| format!("stringToJson conversion error: {e}"))
}

pub fn json_to_string_fn(args: &Value) -> Result<Value, String> {
    let json = eval_value_to_json(args);
    serde_json::to_string(&json)
        .map(Value::String)
        .map_err(|e| format!("jsonToString error: {e}"))
}

pub fn array_fn(args: &Value) -> Result<Value, String> {
    match args {
        Value::Empty => Ok(Value::Tuple(Vec::new())),
        Value::Tuple(items) => Ok(Value::Tuple(items.clone())),
        single => Ok(Value::Tuple(vec![single.clone()])),
    }
}

// ───────────────── math functions ─────────────────

pub fn math_random_fn(args: &Value) -> Result<Value, String> {
    let tuple = extract_tuple(args, "mathRandom", 2)?;
    let start = tuple[0]
        .as_int()
        .map_err(|_| "mathRandom requires integer arguments")?;
    let end = tuple[1]
        .as_int()
        .map_err(|_| "mathRandom requires integer arguments")?;
    if start >= end {
        return Err(format!(
            "mathRandom: start ({start}) must be less than end ({end})"
        ));
    }
    let range_size = (end - start) as u64;
    let val: u64 = rand::random::<u64>() % range_size;
    let result = start + i64::try_from(val).map_or(0, |v| v);
    Ok(Value::Int(result))
}

pub fn math_add_fn(args: &Value) -> Result<Value, String> {
    let tuple = extract_tuple(args, "mathAdd", 2)?;
    match (&tuple[0], &tuple[1]) {
        (Value::Int(a), Value::Int(b)) => Ok(Value::Int(a.saturating_add(*b))),
        _ => {
            let a = as_numeric(&tuple[0], "mathAdd")?;
            let b = as_numeric(&tuple[1], "mathAdd")?;
            Ok(Value::Float(a + b))
        }
    }
}

pub fn math_sub_fn(args: &Value) -> Result<Value, String> {
    let tuple = extract_tuple(args, "mathSub", 2)?;
    match (&tuple[0], &tuple[1]) {
        (Value::Int(a), Value::Int(b)) => Ok(Value::Int(a.saturating_sub(*b))),
        _ => {
            let a = as_numeric(&tuple[0], "mathSub")?;
            let b = as_numeric(&tuple[1], "mathSub")?;
            Ok(Value::Float(a - b))
        }
    }
}

// ───────────────── utility functions ─────────────────

pub fn uuid_fn(args: &Value) -> Result<Value, String> {
    match args {
        Value::Empty => {}
        Value::Tuple(t) if t.is_empty() => {}
        _ => return Err("uuid takes no arguments".into()),
    }
    Ok(Value::String(uuid::Uuid::new_v4().to_string()))
}

pub fn hash_fn(args: &Value) -> Result<Value, String> {
    let tuple = extract_tuple(args, "hash", 2)?;
    let input = tuple[0]
        .as_string()
        .map_err(|_| "hash: first argument must be a string")?;
    let algo = tuple[1]
        .as_string()
        .map_err(|_| "hash: second argument must be algorithm name")?;

    match algo.as_str() {
        "sha256" => {
            let digest = Sha256::digest(input.as_bytes());
            Ok(Value::String(hex_encode(&digest)))
        }
        "md5" => {
            let digest = md5::compute(input.as_bytes());
            Ok(Value::String(format!("{digest:x}")))
        }
        other => Err(format!(
            "hash: unsupported algorithm '{other}', use sha256 or md5"
        )),
    }
}

fn hex_encode(bytes: &[u8]) -> String {
    bytes.iter().fold(String::new(), |mut acc, b| {
        use std::fmt::Write;
        let _ = write!(acc, "{b:02x}");
        acc
    })
}

pub fn base64_encode_fn(args: &Value) -> Result<Value, String> {
    let s = match args.as_tuple() {
        Ok(t) if !t.is_empty() => t[0].as_string(),
        _ => args.as_string(),
    }
    .map_err(|_| "base64Encode requires a string argument")?;

    let encoded = base64::engine::general_purpose::STANDARD.encode(s.as_bytes());
    Ok(Value::String(encoded))
}

pub fn base64_decode_fn(args: &Value) -> Result<Value, String> {
    let s = match args.as_tuple() {
        Ok(t) if !t.is_empty() => t[0].as_string(),
        _ => args.as_string(),
    }
    .map_err(|_| "base64Decode requires a string argument")?;

    let bytes = base64::engine::general_purpose::STANDARD
        .decode(s.as_bytes())
        .map_err(|e| format!("base64Decode error: {e}"))?;
    String::from_utf8(bytes)
        .map(Value::String)
        .map_err(|e| format!("base64Decode: invalid UTF-8: {e}"))
}

// ───────────────── array functions ─────────────────

pub fn array_partition_fn(args: &Value) -> Result<Value, String> {
    let tuple = extract_tuple(args, "arrayPartition", 2)?;
    let items = tuple[0]
        .as_tuple()
        .map_err(|_| "arrayPartition: first argument must be an array")?;
    let chunk_size = tuple[1]
        .as_int()
        .map_err(|_| "arrayPartition: second argument must be an integer")?;
    if chunk_size <= 0 {
        return Err("arrayPartition: chunk_size must be positive".into());
    }
    let size = chunk_size as usize;
    let chunks: Vec<Value> = items
        .chunks(size)
        .map(|c| Value::Tuple(c.to_vec()))
        .collect();
    Ok(Value::Tuple(chunks))
}

pub fn array_contains_fn(args: &Value) -> Result<Value, String> {
    let tuple = extract_tuple(args, "arrayContains", 2)?;
    let items = tuple[0]
        .as_tuple()
        .map_err(|_| "arrayContains: first argument must be an array")?;
    let needle = &tuple[1];
    Ok(Value::Boolean(items.contains(needle)))
}

pub fn array_range_fn(args: &Value) -> Result<Value, String> {
    let tuple = extract_tuple(args, "arrayRange", 3)?;
    let start = tuple[0]
        .as_int()
        .map_err(|_| "arrayRange requires integer arguments")?;
    let end = tuple[1]
        .as_int()
        .map_err(|_| "arrayRange requires integer arguments")?;
    let step = tuple[2]
        .as_int()
        .map_err(|_| "arrayRange requires integer arguments")?;
    if step == 0 {
        return Err("arrayRange: step must not be zero".into());
    }
    let mut result = Vec::new();
    let mut current = start;
    while (step > 0 && current < end) || (step < 0 && current > end) {
        result.push(Value::Int(current));
        current = current.saturating_add(step);
    }
    Ok(Value::Tuple(result))
}

pub fn array_length_fn(args: &Value) -> Result<Value, String> {
    let items = match args.as_tuple() {
        Ok(t) => t,
        Err(_) => match args {
            Value::Empty => return Ok(Value::Int(0)),
            _ => return Err("arrayLength: argument must be an array".into()),
        },
    };
    Ok(Value::Int(items.len() as i64))
}

pub fn array_unique_fn(args: &Value) -> Result<Value, String> {
    let items = match args.as_tuple() {
        Ok(t) => t,
        Err(_) => match args {
            Value::Empty => return Ok(Value::Tuple(Vec::new())),
            _ => return Err("arrayUnique: argument must be an array".into()),
        },
    };
    let mut seen = Vec::new();
    for item in &items {
        if !seen.contains(item) {
            seen.push(item.clone());
        }
    }
    Ok(Value::Tuple(seen))
}
