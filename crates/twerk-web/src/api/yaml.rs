#![forbid(unsafe_code)]
#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use super::error::ApiError;
use serde::de::DeserializeOwned;
use serde_saphyr::DuplicateKeyPolicy;

pub mod tests;

pub(crate) const MAX_YAML_BODY_SIZE: usize = 512 * 1024;

const DEFAULT_MAX_DEPTH: usize = 64;
const DEFAULT_MAX_NODES: usize = 10_000;

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

fn build_options() -> serde_saphyr::Options {
    serde_saphyr::options! {
        budget: serde_saphyr::budget! {
            max_depth: DEFAULT_MAX_DEPTH,
            max_nodes: DEFAULT_MAX_NODES,
        },
        duplicate_keys: DuplicateKeyPolicy::Error,
    }
}

/// Parse a YAML byte slice into a deserialized value.
///
/// # Errors
///
/// Returns [`ApiError`] if the input is empty, exceeds size limits,
/// contains invalid UTF-8, or fails to parse as valid YAML.
pub fn from_slice<T: DeserializeOwned>(bytes: &[u8]) -> Result<T, ApiError> {
    validate_yaml_input(bytes)?;

    let s = std::str::from_utf8(bytes)
        .map_err(|e| ApiError::bad_request(format!("invalid UTF-8 in YAML body: {e}")))?;

    let options = build_options();

    serde_saphyr::from_str_with_options::<T>(s, options)
        .map_err(|e| ApiError::bad_request(format!("YAML parse error: {e}")))
}

/// Serialize a value to a YAML string.
///
/// # Errors
///
/// Returns [`ApiError`] if serialization fails.
pub fn to_string<T: serde::Serialize>(value: &T) -> Result<String, ApiError> {
    serde_saphyr::to_string(value)
        .map_err(|e| ApiError::internal(format!("YAML serialization error: {e}")))
}
