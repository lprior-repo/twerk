//! API infrastructure domain types.
//!
//! # Design Principles
//!
//! - **Parse, don't validate**: Raw input is parsed into domain types at
//!   boundary entry points. Core logic receives only validated types.
//! - **Make illegal states unrepresentable**: Newtypes encode business rules
//!   that cannot be violated after construction.

use std::fmt;
use std::ops::Deref;
use std::str::FromStr;

use serde::{Deserialize, Serialize};
use thiserror::Error;

// ---------------------------------------------------------------------------
// ServerAddress
// ---------------------------------------------------------------------------

/// A validated server address (host:port format).
///
/// # Validation Rules
/// - Must contain a colon (`:`) separating host and port
/// - Port must be a valid u16 number
/// - Host can be an IP address or hostname
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct ServerAddress(String);

#[derive(Debug, Clone, PartialEq, Error)]
pub enum ServerAddressError {
    #[error("address must contain a host:port separator")]
    MissingSeparator,
    #[error("port must be a valid number (0-65535)")]
    InvalidPort,
    #[error("host cannot be empty")]
    EmptyHost,
}

impl ServerAddress {
    /// Parse a `ServerAddress` from a string.
    ///
    /// # Errors
    ///
    /// Returns [`ServerAddressError`] if the address is invalid.
    pub fn new(addr: impl Into<String>) -> Result<Self, ServerAddressError> {
        let s = addr.into();
        if s.is_empty() {
            return Err(ServerAddressError::EmptyHost);
        }

        let parts: Vec<&str> = s.rsplitn(2, ':').collect();
        if parts.len() != 2 {
            return Err(ServerAddressError::MissingSeparator);
        }

        let port_str = parts[0];
        let host = parts[1];

        if host.is_empty() {
            return Err(ServerAddressError::EmptyHost);
        }

        port_str
            .parse::<u16>()
            .map_err(|_| ServerAddressError::InvalidPort)?;

        Ok(Self(s))
    }

    /// Returns the address as a string slice.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for ServerAddress {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl FromStr for ServerAddress {
    type Err = ServerAddressError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::new(s)
    }
}

impl AsRef<str> for ServerAddress {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

impl Deref for ServerAddress {
    type Target = str;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

// ---------------------------------------------------------------------------
// ContentType
// ---------------------------------------------------------------------------

/// A validated content type header value.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ContentType {
    Json,
    Yaml,
    Unknown(String),
}

impl ContentType {
    /// Parse content type from a header string value.
    #[must_use]
    pub fn from_header(value: &str) -> Self {
        match value.to_lowercase().as_str() {
            "application/json" => Self::Json,
            "text/yaml" | "application/x-yaml" => Self::Yaml,
            other => Self::Unknown(other.to_string()),
        }
    }

    /// Returns true if this content type is supported for request bodies.
    #[must_use]
    pub fn is_supported(&self) -> bool {
        matches!(self, Self::Json | Self::Yaml)
    }
}

impl fmt::Display for ContentType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Json => write!(f, "application/json"),
            Self::Yaml => write!(f, "application/x-yaml"),
            Self::Unknown(s) => write!(f, "{s}"),
        }
    }
}

// ---------------------------------------------------------------------------
// Feature Flags
// ---------------------------------------------------------------------------

/// Well-known API feature flags.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ApiFeature {
    Health,
    Tasks,
    Jobs,
    Queues,
    Nodes,
    Metrics,
    Users,
}

impl ApiFeature {
    /// Parse a feature flag from a string.
    #[must_use]
    pub fn from_str_opt(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "health" => Some(Self::Health),
            "tasks" => Some(Self::Tasks),
            "jobs" => Some(Self::Jobs),
            "queues" => Some(Self::Queues),
            "nodes" => Some(Self::Nodes),
            "metrics" => Some(Self::Metrics),
            "users" => Some(Self::Users),
            _ => None,
        }
    }

    /// Returns the default enabled state for this feature.
    #[must_use]
    pub fn default_enabled(&self) -> bool {
        matches!(self, Self::Health)
    }
}

/// A set of feature flags with their enabled/disabled state.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct FeatureFlags(Vec<(ApiFeature, bool)>);

impl FeatureFlags {
    /// Create a new `FeatureFlags` from a map.
    pub fn new(flags: impl IntoIterator<Item = (ApiFeature, bool)>) -> Self {
        Self(flags.into_iter().collect())
    }

    /// Check if a feature is enabled.
    #[must_use]
    pub fn is_enabled(&self, feature: ApiFeature) -> bool {
        self.0
            .iter()
            .find(|(f, _)| *f == feature)
            .map_or_else(|| feature.default_enabled(), |(_, enabled)| *enabled)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn server_address_valid() {
        let addr = ServerAddress::new("0.0.0.0:8080").unwrap();
        assert_eq!(addr.as_str(), "0.0.0.0:8080");
    }

    #[test]
    fn server_address_rejects_invalid() {
        assert!(matches!(
            ServerAddress::new("invalid"),
            Err(ServerAddressError::MissingSeparator)
        ));
        assert!(matches!(
            ServerAddress::new(":8080"),
            Err(ServerAddressError::EmptyHost)
        ));
        assert!(matches!(
            ServerAddress::new("localhost:invalid"),
            Err(ServerAddressError::InvalidPort)
        ));
    }

    #[test]
    fn content_type_parsing() {
        assert!(matches!(
            ContentType::from_header("application/json"),
            ContentType::Json
        ));
        assert!(matches!(
            ContentType::from_header("text/yaml"),
            ContentType::Yaml
        ));
        assert!(matches!(
            ContentType::from_header("application/x-yaml"),
            ContentType::Yaml
        ));
    }

    #[test]
    fn feature_flags_default() {
        let flags = FeatureFlags::default();
        assert!(flags.is_enabled(ApiFeature::Health)); // Health defaults to true
        assert!(!flags.is_enabled(ApiFeature::Users)); // Others default to false
    }
}
