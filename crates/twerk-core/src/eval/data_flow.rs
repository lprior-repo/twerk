//! ASL data flow processing — `input_path`, `result_path`, `output_path`.
//!
//! Implements the subset of JSONPath needed for AWS Step Functions data flow:
//! `$`, `$.field`, `$.field.nested`, `$.field[0]`.

use serde_json::Value;
use thiserror::Error;

use crate::asl::types::JsonPath;

// ---------------------------------------------------------------------------
// Error
// ---------------------------------------------------------------------------

#[derive(Debug, Error, PartialEq, Eq)]
pub enum DataFlowError {
    #[error("path not found: {path} (available: {available:?})")]
    PathNotFound {
        path: String,
        available: Vec<String>,
    },

    #[error("invalid path '{path}': {reason}")]
    InvalidPath { path: String, reason: String },

    #[error("expected object at '{path}', got non-object")]
    NotAnObject { path: String },
}

// ---------------------------------------------------------------------------
// Path segment parsing
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
enum Segment {
    Field(String),
    Index(usize),
}

fn parse_segments(path: &JsonPath) -> Result<Vec<Segment>, DataFlowError> {
    let raw = path.as_str();
    // JsonPath::new() guarantees '$' prefix — strip it
    let rest = &raw[1..];
    if rest.is_empty() {
        return Ok(vec![]);
    }
    // Strip leading "."
    let rest = match rest.strip_prefix('.') {
        Some(s) => s,
        None => rest,
    };
    if rest.is_empty() {
        return Ok(vec![]);
    }

    let mut segments = Vec::new();
    for token in rest.split('.') {
        if let Some(bracket_pos) = token.find('[') {
            let field = &token[..bracket_pos];
            if !field.is_empty() {
                segments.push(Segment::Field(field.to_string()));
            }
            let idx_str = token[bracket_pos + 1..].strip_suffix(']').ok_or_else(|| {
                DataFlowError::InvalidPath {
                    path: raw.to_string(),
                    reason: "unclosed bracket".to_string(),
                }
            })?;
            let idx: usize = idx_str.parse().map_err(|_| DataFlowError::InvalidPath {
                path: raw.to_string(),
                reason: format!("invalid array index: {idx_str}"),
            })?;
            segments.push(Segment::Index(idx));
        } else {
            segments.push(Segment::Field(token.to_string()));
        }
    }
    Ok(segments)
}

// ---------------------------------------------------------------------------
// Path resolution (read)
// ---------------------------------------------------------------------------

fn resolve_path(
    value: &Value,
    segments: &[Segment],
    full_path: &str,
) -> Result<Value, DataFlowError> {
    let mut current = value;
    for seg in segments {
        match seg {
            Segment::Field(name) => {
                let obj = current
                    .as_object()
                    .ok_or_else(|| DataFlowError::NotAnObject {
                        path: full_path.to_string(),
                    })?;
                current = obj
                    .get(name.as_str())
                    .ok_or_else(|| DataFlowError::PathNotFound {
                        path: full_path.to_string(),
                        available: obj.keys().cloned().collect(),
                    })?;
            }
            Segment::Index(idx) => {
                let arr = current
                    .as_array()
                    .ok_or_else(|| DataFlowError::NotAnObject {
                        path: full_path.to_string(),
                    })?;
                current = arr.get(*idx).ok_or_else(|| DataFlowError::PathNotFound {
                    path: full_path.to_string(),
                    available: vec![format!("0..{}", arr.len())],
                })?;
            }
        }
    }
    Ok(current.clone())
}

// ---------------------------------------------------------------------------
// Path assignment (write) — only field segments for result_path
// ---------------------------------------------------------------------------

fn set_at_path(
    root: &Value,
    segments: &[Segment],
    val: &Value,
    full_path: &str,
) -> Result<Value, DataFlowError> {
    if segments.is_empty() {
        return Ok(val.clone());
    }

    let mut result = root.clone();
    let mut cursor = &mut result;

    for (i, seg) in segments.iter().enumerate() {
        let is_last = i + 1 == segments.len();
        match seg {
            Segment::Field(name) => {
                if is_last {
                    cursor
                        .as_object_mut()
                        .ok_or_else(|| DataFlowError::NotAnObject {
                            path: full_path.to_string(),
                        })?
                        .insert(name.clone(), val.clone());
                } else {
                    let obj = cursor
                        .as_object_mut()
                        .ok_or_else(|| DataFlowError::NotAnObject {
                            path: full_path.to_string(),
                        })?;
                    if !obj.contains_key(name) {
                        obj.insert(name.clone(), Value::Object(serde_json::Map::new()));
                    }
                    cursor = obj
                        .get_mut(name)
                        .ok_or_else(|| DataFlowError::NotAnObject {
                            path: full_path.to_string(),
                        })?;
                }
            }
            Segment::Index(_) => {
                return Err(DataFlowError::InvalidPath {
                    path: full_path.to_string(),
                    reason: "array index not supported in result_path".to_string(),
                });
            }
        }
    }
    Ok(result)
}

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

/// Apply `InputPath` to filter input data before state processing.
pub fn apply_input_path(input: &Value, path: Option<&JsonPath>) -> Result<Value, DataFlowError> {
    let Some(p) = path else {
        return Ok(input.clone());
    };
    let segments = parse_segments(p)?;
    resolve_path(input, &segments, p.as_str())
}

/// Apply `ResultPath` to merge a state's result into the original input.
pub fn apply_result_path(
    input: &Value,
    result: &Value,
    path: Option<&JsonPath>,
) -> Result<Value, DataFlowError> {
    let Some(p) = path else {
        return Ok(result.clone());
    };
    let segments = parse_segments(p)?;
    if segments.is_empty() {
        return Ok(result.clone());
    }
    set_at_path(input, &segments, result, p.as_str())
}

/// Apply `OutputPath` to filter output data after state processing.
pub fn apply_output_path(output: &Value, path: Option<&JsonPath>) -> Result<Value, DataFlowError> {
    let Some(p) = path else {
        return Ok(output.clone());
    };
    let segments = parse_segments(p)?;
    resolve_path(output, &segments, p.as_str())
}

/// Full data flow pipeline: `input_path` → process → `result_path` → `output_path`.
pub fn apply_data_flow(
    input: &Value,
    result: &Value,
    input_path: Option<&JsonPath>,
    result_path: Option<&JsonPath>,
    output_path: Option<&JsonPath>,
) -> Result<Value, DataFlowError> {
    let filtered = apply_input_path(input, input_path)?;
    let merged = apply_result_path(&filtered, result, result_path)?;
    apply_output_path(&merged, output_path)
}
