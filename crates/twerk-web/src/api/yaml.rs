#![forbid(unsafe_code)]
#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use super::error::ApiError;
use serde::de::DeserializeOwned;
use std::str;

pub mod tests;

pub(crate) const MAX_YAML_DEPTH: usize = 64;
pub(crate) const MAX_YAML_BODY_SIZE: usize = 512 * 1024;
pub(crate) const MAX_YAML_NODES: usize = 10_000;

/// Converts yaml-rust2 Yaml to serde_json::Value for deserialization.
fn yaml_to_json(yaml: &yaml_rust2::Yaml) -> serde_json::Value {
    match yaml {
        yaml_rust2::Yaml::Null => serde_json::Value::Null,
        yaml_rust2::Yaml::Boolean(b) => serde_json::Value::Bool(*b),
        yaml_rust2::Yaml::Integer(i) => serde_json::Value::Number((*i).into()),
        yaml_rust2::Yaml::Real(s) => {
            // Real is a string representation, parse as f64
            s.parse::<f64>()
                .ok()
                .and_then(|f| serde_json::Number::from_f64(f))
                .map_or(serde_json::Value::Null, serde_json::Value::Number)
        }
        yaml_rust2::Yaml::String(s) => serde_json::Value::String(s.clone()),
        yaml_rust2::Yaml::Array(arr) => {
            serde_json::Value::Array(arr.iter().map(yaml_to_json).collect())
        }
        yaml_rust2::Yaml::Hash(map) => {
            let obj = map
                .iter()
                .map(|(k, v)| {
                    let key = match k {
                        yaml_rust2::Yaml::String(s) => s.clone(),
                        yaml_rust2::Yaml::Integer(i) => i.to_string(),
                        yaml_rust2::Yaml::Boolean(b) => b.to_string(),
                        yaml_rust2::Yaml::Real(r) => r.clone(),
                        yaml_rust2::Yaml::Null => "null".to_string(),
                        _ => format!("{k:?}"),
                    };
                    (key, yaml_to_json(v))
                })
                .collect();
            serde_json::Value::Object(obj)
        }
        yaml_rust2::Yaml::Alias(_) | yaml_rust2::Yaml::BadValue => serde_json::Value::Null,
    }
}

/// Parses a YAML document from bytes with size and complexity limits.
///
/// # Errors
/// Returns an `ApiError` if the YAML document cannot be parsed or violates size/complexity limits.
pub fn from_slice<T: DeserializeOwned>(bytes: &[u8]) -> Result<T, ApiError> {
    if bytes.is_empty() {
        return Err(ApiError::bad_request("YAML body is empty"));
    }
    if bytes.len() > MAX_YAML_BODY_SIZE {
        return Err(ApiError::bad_request("YAML body exceeds size limit"));
    }
    // Reject null bytes - they can cause issues in C interop and are not valid in YAML strings
    if bytes.contains(&b'\0') {
        return Err(ApiError::bad_request("YAML parse error"));
    }
    let s =
        str::from_utf8(bytes).map_err(|_| ApiError::bad_request("invalid UTF-8 in YAML body"))?;
    let docs = yaml_rust2::YamlLoader::load_from_str(s)
        .map_err(|_| ApiError::bad_request("YAML parse error"))?;
    let doc = docs
        .first()
        .ok_or_else(|| ApiError::bad_request("YAML parse error"))?;
    let (depth, nodes) = measure_ast_depth_and_nodes(doc);
    if depth > MAX_YAML_DEPTH {
        return Err(ApiError::bad_request("YAML nesting depth exceeds limit"));
    }
    if nodes > MAX_YAML_NODES {
        return Err(ApiError::bad_request(
            "YAML document exceeds complexity limit",
        ));
    }
    // Convert yaml-rust2 Yaml to serde_json::Value, then deserialize
    let json_value = yaml_to_json(doc);
    serde_json::from_value(json_value).map_err(|_| ApiError::bad_request("YAML parse error"))
}

pub(crate) fn measure_ast_depth_and_nodes(yaml: &yaml_rust2::Yaml) -> (usize, usize) {
    fn walk(yaml: &yaml_rust2::Yaml, depth: usize, max_depth: &mut usize, count: &mut usize) {
        *count += 1;
        if *count > MAX_YAML_NODES {
            return;
        }
        match yaml {
            yaml_rust2::Yaml::Array(items) => {
                *max_depth = (*max_depth).max(depth + 1);
                for item in items {
                    walk(item, depth + 1, max_depth, count);
                }
            }
            yaml_rust2::Yaml::Hash(map) => {
                *max_depth = (*max_depth).max(depth + 1);
                for (k, v) in map {
                    walk(k, depth + 1, max_depth, count);
                    walk(v, depth + 1, max_depth, count);
                }
            }
            yaml_rust2::Yaml::Alias(_) => {
                *count += 1;
            }
            _ => {}
        }
    }
    let mut max_depth = 0usize;
    let mut count = 0usize;
    walk(yaml, 0, &mut max_depth, &mut count);
    (max_depth, count)
}
