#![forbid(unsafe_code)]
#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use super::error::ApiError;
use serde::de::DeserializeOwned;
use std::str;

pub mod tests;

pub(crate) const MAX_YAML_DEPTH: usize = 64;
pub(crate) const MAX_YAML_BODY_SIZE: usize = 512 * 1024;
pub(crate) const MAX_YAML_NODES: usize = 10_000;

/// Converts a YAML scalar to a JSON string key.
fn yaml_key_to_string(yaml: &yaml_rust2::Yaml) -> String {
    match yaml {
        yaml_rust2::Yaml::String(s) => s.clone(),
        yaml_rust2::Yaml::Integer(i) => i.to_string(),
        yaml_rust2::Yaml::Boolean(b) => b.to_string(),
        yaml_rust2::Yaml::Real(r) => r.clone(),
        yaml_rust2::Yaml::Null => "null".to_string(),
        _ => format!("{yaml:?}"),
    }
}

/// Parses a YAML Real (float) to `serde_json::Number`, returning 0.0 for invalid literals.
fn parse_yaml_real(s: &str) -> serde_json::Number {
    // Parse the string as f64, then convert to serde_json::Number.
    // If parsing fails or conversion fails (e.g., NaN, infinity), use 0.0 as fallback.
    // The fallback to 0.0 is safe because 0.0 is always a valid f64 value
    // and serde_json::Number::from_f64(0.0) is guaranteed to return Some.
    // This is documented in serde_json - from_f64 only returns None for NaN and infinity.
    // Since we're explicitly using 0.0 (not NaN or infinity), the unwrap is safe.
    #[allow(clippy::unwrap_used, clippy::expect_used)]
    {
        s.parse::<f64>()
            .ok()
            .and_then(serde_json::Number::from_f64)
            .unwrap_or_else(|| {
                // SAFETY: 0.0 is a compile-time constant that is always valid.
                // from_f64(0.0) never returns None for finite values.
                serde_json::Number::from_f64(0.0).expect("0.0 is always a valid f64")
            })
    }
}

/// Converts yaml-rust2 `Yaml` to `serde_json::Value` for deserialization.
fn yaml_to_json(yaml: &yaml_rust2::Yaml) -> serde_json::Value {
    match yaml {
        yaml_rust2::Yaml::Boolean(b) => serde_json::Value::Bool(*b),
        yaml_rust2::Yaml::Integer(i) => serde_json::Value::Number((*i).into()),
        yaml_rust2::Yaml::Real(s) => serde_json::Value::Number(parse_yaml_real(s)),
        yaml_rust2::Yaml::String(s) => serde_json::Value::String(s.clone()),
        yaml_rust2::Yaml::Array(arr) => {
            serde_json::Value::Array(arr.iter().map(yaml_to_json).collect())
        }
        yaml_rust2::Yaml::Hash(map) => serde_json::Value::Object(
            map.iter()
                .map(|(k, v)| (yaml_key_to_string(k), yaml_to_json(v)))
                .collect(),
        ),
        yaml_rust2::Yaml::Null | yaml_rust2::Yaml::Alias(_) | yaml_rust2::Yaml::BadValue => {
            serde_json::Value::Null
        }
    }
}

/// Validates YAML input preconditions: empty body, size limit, null bytes.
fn validate_yaml_input(bytes: &[u8]) -> Result<(), ApiError> {
    if bytes.is_empty() {
        return Err(ApiError::bad_request("YAML body is empty"));
    }
    if bytes.len() > MAX_YAML_BODY_SIZE {
        return Err(ApiError::bad_request("YAML body exceeds size limit"));
    }
    if bytes.contains(&b'\0') {
        return Err(ApiError::bad_request("YAML parse error"));
    }
    Ok(())
}

/// Enforces AST depth and node count limits.
fn enforce_ast_limits(yaml: &yaml_rust2::Yaml) -> Result<(), ApiError> {
    let (depth, nodes) = measure_ast_depth_and_nodes(yaml);
    if depth > MAX_YAML_DEPTH {
        return Err(ApiError::bad_request("YAML nesting depth exceeds limit"));
    }
    if nodes > MAX_YAML_NODES {
        return Err(ApiError::bad_request(
            "YAML document exceeds complexity limit",
        ));
    }
    Ok(())
}

/// Parses a YAML document from bytes with size and complexity limits.
///
/// # Errors
/// Returns an `ApiError` if the YAML document cannot be parsed or violates size/complexity limits.
pub fn from_slice<T: DeserializeOwned>(bytes: &[u8]) -> Result<T, ApiError> {
    validate_yaml_input(bytes)?;
    let s =
        str::from_utf8(bytes).map_err(|_| ApiError::bad_request("invalid UTF-8 in YAML body"))?;
    let docs = yaml_rust2::YamlLoader::load_from_str(s)
        .map_err(|_| ApiError::bad_request("YAML parse error"))?;
    let doc = docs
        .first()
        .ok_or_else(|| ApiError::bad_request("YAML parse error"))?;
    enforce_ast_limits(doc)?;
    let json_value = yaml_to_json(doc);
    serde_json::from_value(json_value).map_err(|_| ApiError::bad_request("YAML parse error"))
}

/// AST visitor state for depth and node counting.
struct AstVisitor {
    max_depth: usize,
    count: usize,
}

impl AstVisitor {
    fn new() -> Self {
        Self {
            max_depth: 0,
            count: 0,
        }
    }

    fn visit(&mut self, yaml: &yaml_rust2::Yaml, depth: usize) {
        self.count += 1;
        if self.count > MAX_YAML_NODES {
            return;
        }
        match yaml {
            yaml_rust2::Yaml::Array(items) => {
                self.max_depth = self.max_depth.max(depth + 1);
                for item in items {
                    self.visit(item, depth + 1);
                }
            }
            yaml_rust2::Yaml::Hash(map) => {
                self.max_depth = self.max_depth.max(depth + 1);
                for (k, v) in map {
                    self.visit(k, depth + 1);
                    self.visit(v, depth + 1);
                }
            }
            yaml_rust2::Yaml::Alias(_) => self.count += 1,
            _ => {}
        }
    }
}

pub(crate) fn measure_ast_depth_and_nodes(yaml: &yaml_rust2::Yaml) -> (usize, usize) {
    let mut visitor = AstVisitor::new();
    visitor.visit(yaml, 0);
    (visitor.max_depth, visitor.count)
}
