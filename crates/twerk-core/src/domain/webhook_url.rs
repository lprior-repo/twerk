//! `WebhookUrl` newtype wrapper.

use std::fmt;
use std::ops::Deref;
use std::str::FromStr;

use serde::{Deserialize, Serialize};
use thiserror::Error;

/// A validated webhook URL with RFC 3986 compliance.
///
/// Validation rules:
/// - Must be a valid URI per RFC 3986
/// - Scheme must be `http` or `https` (case-insensitive)
/// - Host component must be non-empty
/// - Port is optional
/// - Path must be non-empty or defaults to `/`
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
#[must_use = "WebhookUrl should be used; it validates at construction"]
pub struct WebhookUrl(String);

/// Errors that can arise when constructing a [`WebhookUrl`].
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum WebhookUrlError {
    #[error("URL parse error: {0}")]
    UrlParseError(String),
    #[error("invalid scheme: {0} (must be http or https)")]
    InvalidScheme(String),
    #[error("URL exceeds maximum length of 2048 characters")]
    UrlTooLong,
    #[error("URL path contains unencoded spaces")]
    SpaceInPath,
    #[error("URL path contains control character (0x{0:02X})")]
    ControlCharacterInPath(u8),
}

// ---------------------------------------------------------------------------
// Private validation helpers
// ---------------------------------------------------------------------------

fn validate_length(s: &str) -> Result<(), WebhookUrlError> {
    if s.len() > 2048 {
        Err(WebhookUrlError::UrlTooLong)
    } else {
        Ok(())
    }
}

fn validate_and_parse_url(s: &str) -> Result<url::Url, WebhookUrlError> {
    url::Url::parse(s).map_err(|e| WebhookUrlError::UrlParseError(e.to_string()))
}

fn validate_scheme(parsed: &url::Url) -> Result<(), WebhookUrlError> {
    let scheme = parsed.scheme();
    if scheme.eq_ignore_ascii_case("http") || scheme.eq_ignore_ascii_case("https") {
        Ok(())
    } else {
        Err(WebhookUrlError::InvalidScheme(scheme.to_string()))
    }
}

fn validate_path(parsed: &url::Url, original: &str) -> Result<(), WebhookUrlError> {
    if parsed.path().contains(' ') {
        return Err(WebhookUrlError::SpaceInPath);
    }
    for b in original.bytes() {
        if b == 0 || (b < 0x20 && b != 0x09) {
            return Err(WebhookUrlError::ControlCharacterInPath(b));
        }
    }
    Ok(())
}

impl WebhookUrl {
    /// Create a new `WebhookUrl`, returning an error if validation fails.
    ///
    /// # Errors
    /// Returns [`WebhookUrlError::UrlParseError`] if the string fails to parse as a URL.
    /// Returns [`WebhookUrlError::InvalidScheme`] if the scheme is not http or https.
    /// Returns [`WebhookUrlError::UrlTooLong`] if the URL exceeds 2048 characters.
    pub fn new(url: impl Into<String>) -> Result<Self, WebhookUrlError> {
        let s = url.into();
        validate_length(&s)?;
        let parsed = validate_and_parse_url(&s)?;
        validate_scheme(&parsed)?;
        validate_path(&parsed, &s)?;
        Ok(Self(s))
    }

    /// View the webhook URL as a string slice.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Parse and return the URL components.
    ///
    /// This parses on demand rather than storing the parsed URL, keeping the type simple.
    ///
    /// # Errors
    /// Returns [`WebhookUrlError::UrlParseError`] if the internal string fails to parse.
    /// This should never occur if the instance was constructed via [`WebhookUrl::new`].
    #[must_use]
    pub fn as_url(&self) -> Result<url::Url, WebhookUrlError> {
        url::Url::parse(&self.0).map_err(|e| WebhookUrlError::UrlParseError(e.to_string()))
    }
}

impl fmt::Display for WebhookUrl {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl FromStr for WebhookUrl {
    type Err = WebhookUrlError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::new(s)
    }
}

impl AsRef<str> for WebhookUrl {
    fn as_ref(&self) -> &str {
        self.as_str()
    }
}

impl Deref for WebhookUrl {
    type Target = str;

    fn deref(&self) -> &Self::Target {
        self.as_str()
    }
}

impl Serialize for WebhookUrl {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str(&self.0)
    }
}

impl<'de> Deserialize<'de> for WebhookUrl {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        Self::new(s).map_err(serde::de::Error::custom)
    }
}

mod tests;
