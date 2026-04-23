//! HTTP method enum for webhook and polling triggers.

use std::fmt;
use std::str::FromStr;

use serde::{Deserialize, Serialize};

use super::error::TriggerDataError;

// =============================================================================
// HttpMethod
// =============================================================================

/// HTTP methods supported by webhook and polling triggers.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "UPPERCASE")]
pub enum HttpMethod {
    Get,
    Post,
    Put,
    Delete,
    Patch,
}

impl HttpMethod {
    /// Parses an HTTP method string (case-insensitive).
    ///
    /// # Parameters
    /// - `s: impl Into<String>` - The string to parse (e.g., "GET", "post", "PuT")
    ///
    /// # Returns
    /// - `Ok(HttpMethod)` on valid HTTP method
    /// - `Err(TriggerDataError::InvalidHttpMethod)` on invalid method
    #[allow(clippy::should_implement_trait)]
    pub fn from_str(s: impl Into<String>) -> Result<HttpMethod, TriggerDataError> {
        let s = s.into();
        match s.to_uppercase().as_str() {
            "GET" => Ok(HttpMethod::Get),
            "POST" => Ok(HttpMethod::Post),
            "PUT" => Ok(HttpMethod::Put),
            "DELETE" => Ok(HttpMethod::Delete),
            "PATCH" => Ok(HttpMethod::Patch),
            _ => Err(TriggerDataError::InvalidHttpMethod(s)),
        }
    }
}

impl fmt::Display for HttpMethod {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            HttpMethod::Get => write!(f, "GET"),
            HttpMethod::Post => write!(f, "POST"),
            HttpMethod::Put => write!(f, "PUT"),
            HttpMethod::Delete => write!(f, "DELETE"),
            HttpMethod::Patch => write!(f, "PATCH"),
        }
    }
}

impl FromStr for HttpMethod {
    type Err = TriggerDataError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::from_str(s)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn httpmethod_from_str_returns_get_when_input_is_get_uppercase() {
        assert_eq!(HttpMethod::from_str("GET"), Ok(HttpMethod::Get));
    }

    #[test]
    fn httpmethod_from_str_returns_get_when_input_is_get_lowercase() {
        assert_eq!(HttpMethod::from_str("get"), Ok(HttpMethod::Get));
    }

    #[test]
    fn httpmethod_from_str_returns_post_when_input_is_post_uppercase() {
        assert_eq!(HttpMethod::from_str("POST"), Ok(HttpMethod::Post));
    }

    #[test]
    fn httpmethod_from_str_returns_put_when_input_is_put_uppercase() {
        assert_eq!(HttpMethod::from_str("PUT"), Ok(HttpMethod::Put));
    }

    #[test]
    fn httpmethod_from_str_returns_delete_when_input_is_delete_uppercase() {
        assert_eq!(HttpMethod::from_str("DELETE"), Ok(HttpMethod::Delete));
    }

    #[test]
    fn httpmethod_from_str_returns_patch_when_input_is_patch_uppercase() {
        assert_eq!(HttpMethod::from_str("PATCH"), Ok(HttpMethod::Patch));
    }

    #[test]
    fn httpmethod_from_str_returns_invalid_http_method_when_input_is_options() {
        assert_eq!(
            HttpMethod::from_str("OPTIONS"),
            Err(TriggerDataError::InvalidHttpMethod("OPTIONS".to_string()))
        );
    }

    #[test]
    fn httpmethod_from_str_returns_invalid_http_method_when_input_is_empty() {
        assert_eq!(
            HttpMethod::from_str(""),
            Err(TriggerDataError::InvalidHttpMethod("".to_string()))
        );
    }
}
