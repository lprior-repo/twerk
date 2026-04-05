#![forbid(unsafe_code)]
#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use super::error::ApiError;
use serde::de::DeserializeOwned;
use std::str;

pub mod tests;

pub(crate) const MAX_YAML_DEPTH: usize = 64;
pub(crate) const MAX_YAML_BODY_SIZE: usize = 512 * 1024;
pub(crate) const MAX_YAML_NODES: usize = 10_000;

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
    let s =
        str::from_utf8(bytes).map_err(|_| ApiError::bad_request("invalid UTF-8 in YAML body"))?;
    let doc = serde_yaml::from_str::<serde_yaml::Value>(s)
        .map_err(|_| ApiError::bad_request("YAML parse error"))?;
    let (depth, nodes) = measure_ast_depth_and_nodes(&doc);
    if depth > MAX_YAML_DEPTH {
        return Err(ApiError::bad_request("YAML nesting depth exceeds limit"));
    }
    if nodes > MAX_YAML_NODES {
        return Err(ApiError::bad_request(
            "YAML document exceeds complexity limit",
        ));
    }
    serde_yaml::from_str(s).map_err(|_| ApiError::bad_request("YAML parse error"))
}

pub(crate) fn measure_ast_depth_and_nodes(yaml: &serde_yaml::Value) -> (usize, usize) {
    fn walk(yaml: &serde_yaml::Value, depth: usize, max_depth: &mut usize, count: &mut usize) {
        *count += 1;
        if *count > MAX_YAML_NODES {
            return;
        }
        match yaml {
            serde_yaml::Value::Sequence(items) => {
                *max_depth = (*max_depth).max(depth + 1);
                for item in items {
                    walk(item, depth + 1, max_depth, count);
                }
            }
            serde_yaml::Value::Mapping(map) => {
                *max_depth = (*max_depth).max(depth + 1);
                for (k, v) in map {
                    walk(k, depth + 1, max_depth, count);
                    walk(v, depth + 1, max_depth, count);
                }
            }
            _ => {}
        }
    }
    let mut max_depth = 0usize;
    let mut count = 0usize;
    walk(yaml, 0, &mut max_depth, &mut count);
    (max_depth, count)
}
