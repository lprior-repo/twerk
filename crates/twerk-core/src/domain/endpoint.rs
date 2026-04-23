//! `Endpoint` newtype wrapper.

use std::fmt;
use std::ops::Deref;
use std::str::FromStr;

use serde::{Deserialize, Serialize};
use thiserror::Error;

/// A validated HTTP/HTTPS endpoint URL.
///
/// Validation rules:
/// - Must be a valid URI per RFC 3986
/// - Scheme must be `http` or `https` (case-insensitive)
/// - Host component must be non-empty
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
#[must_use = "Endpoint should be used; it validates at construction"]
pub struct Endpoint(String);

/// Errors that can arise when constructing an [`Endpoint`].
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum EndpointError {
    #[error("URL parse error: {0}")]
    UrlParseError(String),
    #[error("invalid scheme: {0} (must be http or https)")]
    InvalidScheme(String),
    #[error("URL has no host component")]
    MissingHost,
}

fn validate_and_parse_url(s: &str) -> Result<url::Url, EndpointError> {
    url::Url::parse(s).map_err(|e| EndpointError::UrlParseError(e.to_string()))
}

fn validate_scheme(parsed: &url::Url) -> Result<(), EndpointError> {
    let scheme = parsed.scheme();
    if scheme.eq_ignore_ascii_case("http") || scheme.eq_ignore_ascii_case("https") {
        Ok(())
    } else {
        Err(EndpointError::InvalidScheme(scheme.to_string()))
    }
}

fn validate_host(parsed: &url::Url) -> Result<(), EndpointError> {
    parsed
        .host()
        .map_or(Err(EndpointError::MissingHost), |_| Ok(()))
}

impl Endpoint {
    /// Create a new `Endpoint`, returning an error if validation fails.
    ///
    /// # Errors
    /// Returns [`EndpointError::UrlParseError`] if the string fails to parse as a URL.
    /// Returns [`EndpointError::InvalidScheme`] if the scheme is not http or https.
    /// Returns [`EndpointError::MissingHost`] if the URL has no host component.
    pub fn new(endpoint: impl Into<String>) -> Result<Self, EndpointError> {
        let s = endpoint.into();
        let parsed = validate_and_parse_url(&s)?;
        validate_scheme(&parsed)?;
        validate_host(&parsed)?;
        Ok(Self(s))
    }

    /// Create a new `Endpoint` without validation (use with caution).
    ///
    /// # Safety
    /// The caller must ensure the string is a valid http/https URL.
    pub fn new_unchecked(endpoint: impl Into<String>) -> Self {
        Self(endpoint.into())
    }

    /// View the endpoint as a string slice.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Parse and return the URL components.
    ///
    /// # Errors
    /// This will never return an error because the underlying string was
    /// validated at [`Endpoint::new`]. The [`Result`] return type exists
    /// solely to uphold the zero-panic invariant.
    #[must_use = "returns Result that must be handled for zero-panic invariant"]
    pub fn as_url(&self) -> Result<url::Url, EndpointError> {
        url::Url::parse(&self.0).map_err(|e| EndpointError::UrlParseError(e.to_string()))
    }
}

impl fmt::Display for Endpoint {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl FromStr for Endpoint {
    type Err = EndpointError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::new(s)
    }
}

impl AsRef<str> for Endpoint {
    fn as_ref(&self) -> &str {
        self.as_str()
    }
}

impl Deref for Endpoint {
    type Target = str;

    fn deref(&self) -> &Self::Target {
        self.as_str()
    }
}

impl Serialize for Endpoint {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str(&self.0)
    }
}

impl<'de> Deserialize<'de> for Endpoint {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        Self::new(s).map_err(serde::de::Error::custom)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn valid_http_endpoint() {
        let ep = Endpoint::new("http://localhost:8000").unwrap();
        assert_eq!(ep.as_str(), "http://localhost:8000");
    }

    #[test]
    fn valid_https_endpoint() {
        let ep = Endpoint::new("https://api.example.com/health").unwrap();
        assert_eq!(ep.as_str(), "https://api.example.com/health");
    }

    #[test]
    fn invalid_scheme_rejected() {
        let result = Endpoint::new("ftp://localhost:8000");
        assert!(matches!(result, Err(EndpointError::InvalidScheme(_))));
    }

    #[test]
    fn invalid_url_rejected() {
        let result = Endpoint::new("not-a-url");
        assert!(matches!(result, Err(EndpointError::UrlParseError(_))));
    }

    #[test]
    fn display_trait() {
        let ep = Endpoint::new("http://localhost:8000").unwrap();
        assert_eq!(format!("{}", ep), "http://localhost:8000");
    }

    #[test]
    fn from_str_trait() {
        let ep: Endpoint = "http://localhost:8000".parse().unwrap();
        assert_eq!(ep.as_str(), "http://localhost:8000");
    }
}
