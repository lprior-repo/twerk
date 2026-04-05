//! Domain newtypes for twerk-web API layer.
//!
//! These types enforce validation at the API boundary, ensuring that invalid
//! representations are rejected before reaching core business logic.
//!
//! # Design Principles
//!
//! - **Parse, don't validate**: Raw input is parsed into domain types at
//!   boundary entry points. Core logic receives only validated types.
//! - **Make illegal states unrepresentable**: Newtypes encode business rules
//!   that cannot be violated after construction.
//! - **Zero-cost abstractions**: Newtypes are compile-time enforced with no
//!   runtime overhead beyond their validation.

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
    pub fn new(addr: impl Into<String>) -> Result<Self, ServerAddressError> {
        let s = addr.into();
        if s.is_empty() {
            return Err(ServerAddressError::EmptyHost);
        }

        let parts: Vec<&str> = s.rsplitn(2, ':').collect();
        if parts.len() != 2 || parts[1].is_empty() {
            return Err(ServerAddressError::MissingSeparator);
        }

        let port_str = parts[0];
        let host = parts[1];

        port_str
            .parse::<u16>()
            .map_err(|_| ServerAddressError::InvalidPort)?;

        if host.is_empty() {
            return Err(ServerAddressError::EmptyHost);
        }

        Ok(Self(s))
    }

    /// Returns the address as a string slice.
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
// Pagination
// ---------------------------------------------------------------------------

/// A validated page number (1-indexed).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(transparent)]
pub struct Page(u64);

#[derive(Debug, Clone, PartialEq, Error)]
pub enum PageError {
    #[error("page number must be at least 1")]
    TooSmall,
}

impl Page {
    /// Create a new `Page` from a 64-bit unsigned integer.
    pub fn new(page: u64) -> Result<Self, PageError> {
        if page < 1 {
            return Err(PageError::TooSmall);
        }
        Ok(Self(page))
    }

    /// Returns the page number as a u64.
    pub fn get(self) -> u64 {
        self.0
    }

    /// Returns the page number as an i64 (for API compatibility).
    pub fn as_i64(self) -> i64 {
        self.0 as i64
    }
}

impl Default for Page {
    fn default() -> Self {
        Self(1)
    }
}

impl From<Page> for i64 {
    fn from(p: Page) -> Self {
        p.as_i64()
    }
}

impl TryFrom<i64> for Page {
    type Error = PageError;

    fn try_from(v: i64) -> Result<Self, Self::Error> {
        if v < 1 {
            return Err(PageError::TooSmall);
        }
        Ok(Self(v as u64))
    }
}

/// A validated page size.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(transparent)]
pub struct PageSize(u64);

#[derive(Debug, Clone, PartialEq, Error)]
pub enum PageSizeError {
    #[error("page size must be at least 1")]
    TooSmall,
    #[error("page size {0} exceeds maximum allowed ({0})")]
    TooLarge(u64),
}

impl PageSize {
    /// Maximum allowed page size.
    pub const MAX_VALUE: u64 = 100;

    /// Default page size.
    pub const DEFAULT: u64 = 10;

    /// Create a new `PageSize` from a 64-bit unsigned integer.
    pub fn new(size: u64) -> Result<Self, PageSizeError> {
        if size < 1 {
            return Err(PageSizeError::TooSmall);
        }
        if size > Self::MAX_VALUE {
            return Err(PageSizeError::TooLarge {
                max: Self::MAX_VALUE,
            });
        }
        Ok(Self(size))
    }

    /// Create a `PageSize` with a default value if None.
    pub fn or_default(size: Option<u64>) -> Self {
        size.and_then(|s| Self::new(s).ok()).unwrap_or_default()
    }

    /// Returns the page size as a u64.
    pub fn get(self) -> u64 {
        self.0
    }

    /// Returns the page size as an i64 (for API compatibility).
    pub fn as_i64(self) -> i64 {
        self.0 as i64
    }
}

impl Default for PageSize {
    fn default() -> Self {
        Self(Self::DEFAULT)
    }
}

impl From<PageSize> for i64 {
    fn from(p: PageSize) -> Self {
        p.as_i64()
    }
}

impl TryFrom<i64> for PageSize {
    type Error = PageSizeError;

    fn try_from(v: i64) -> Result<Self, Self::Error> {
        if v < 1 {
            return Err(PageSizeError::TooSmall);
        }
        if v as u64 > Self::MAX_VALUE {
            return Err(PageSizeError::TooLarge {
                max: Self::MAX_VALUE,
            });
        }
        Ok(Self(v as u64))
    }
}

// ---------------------------------------------------------------------------
// Username
// ---------------------------------------------------------------------------

/// A validated username.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct Username(String);

#[derive(Debug, Clone, PartialEq, Error)]
pub enum UsernameError {
    #[error("username cannot be empty")]
    Empty,
    #[error("username must be 3-64 characters")]
    LengthOutOfRange,
    #[error("username must start with a letter and contain only alphanumeric characters, underscores, or hyphens")]
    InvalidCharacter,
}

impl Username {
    /// Create a new `Username` from a string.
    pub fn new(username: impl Into<String>) -> Result<Self, UsernameError> {
        let s = username.into();

        if s.is_empty() {
            return Err(UsernameError::Empty);
        }

        let len = s.len();
        if len < 3 || len > 64 {
            return Err(UsernameError::LengthOutOfRange);
        }

        let mut chars = s.chars();
        let first = chars.next().unwrap();
        if !first.is_alphabetic() {
            return Err(UsernameError::InvalidCharacter);
        }

        for c in chars {
            if !c.is_alphanumeric() && c != '_' && c != '-' {
                return Err(UsernameError::InvalidCharacter);
            }
        }

        Ok(Self(s))
    }

    /// View the username as a string slice.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for Username {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl FromStr for Username {
    type Err = UsernameError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::new(s)
    }
}

impl AsRef<str> for Username {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

impl Deref for Username {
    type Target = str;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

// ---------------------------------------------------------------------------
// Password
// ---------------------------------------------------------------------------

/// A validated password (minimum requirements enforced).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct Password(String);

#[derive(Debug, Clone, PartialEq, Error)]
pub enum PasswordError {
    #[error("password cannot be empty")]
    Empty,
    #[error("password must be at least 8 characters")]
    TooShort,
}

impl Password {
    /// Create a new `Password` from a string.
    pub fn new(password: impl Into<String>) -> Result<Self, PasswordError> {
        let s = password.into();

        if s.is_empty() {
            return Err(PasswordError::Empty);
        }

        if s.len() < 8 {
            return Err(PasswordError::TooShort);
        }

        Ok(Self(s))
    }

    /// View the password as a string slice.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for Password {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("[REDACTED]")
    }
}

impl AsRef<str> for Password {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

// ---------------------------------------------------------------------------
// SearchQuery
// ---------------------------------------------------------------------------

/// A search query string.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(transparent)]
pub struct SearchQuery(String);

impl SearchQuery {
    /// Create a new `SearchQuery` from a string.
    pub fn new(q: impl Into<String>) -> Self {
        Self(q.into())
    }

    /// View the query as a string slice.
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Returns true if the query is empty.
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

impl fmt::Display for SearchQuery {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl FromStr for SearchQuery {
    type Err = std::convert::Infallible;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(Self::new(s))
    }
}

impl AsRef<str> for SearchQuery {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

impl Deref for SearchQuery {
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
    pub fn from_header(value: &str) -> Self {
        match value.to_lowercase().as_str() {
            "application/json" => Self::Json,
            "text/yaml" | "application/x-yaml" => Self::Yaml,
            other => Self::Unknown(other.to_string()),
        }
    }

    /// Returns true if this content type is supported for request bodies.
    pub fn is_supported(&self) -> bool {
        matches!(self, Self::Json | Self::Yaml)
    }
}

impl fmt::Display for ContentType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Json => write!(f, "application/json"),
            Self::Yaml => write!(f, "application/x-yaml"),
            Self::Unknown(s) => write!(f, "{}", s),
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
    pub fn from_str(s: &str) -> Option<Self> {
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
    pub fn is_enabled(&self, feature: ApiFeature) -> bool {
        self.0
            .iter()
            .find(|(f, _)| *f == feature)
            .map(|(_, enabled)| *enabled)
            .unwrap_or_else(|| feature.default_enabled())
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
    fn page_valid() {
        let page = Page::new(1).unwrap();
        assert_eq!(page.get(), 1);
    }

    #[test]
    fn page_rejects_zero() {
        assert!(matches!(Page::new(0), Err(PageError::TooSmall)));
    }

    #[test]
    fn page_size_valid() {
        let size = PageSize::new(20).unwrap();
        assert_eq!(size.get(), 20);
    }

    #[test]
    fn page_size_clamped_to_max() {
        let size = PageSize::new(200);
        assert!(size.is_err());
    }

    #[test]
    fn username_valid() {
        let u = Username::new("john_doe").unwrap();
        assert_eq!(u.as_str(), "john_doe");
    }

    #[test]
    fn username_rejects_short() {
        assert!(matches!(
            Username::new("ab"),
            Err(UsernameError::LengthOutOfRange)
        ));
    }

    #[test]
    fn username_rejects_invalid_start() {
        assert!(matches!(
            Username::new("123_user"),
            Err(UsernameError::InvalidCharacter)
        ));
    }

    #[test]
    fn password_valid() {
        let p = Password::new("secretpassword123").unwrap();
        assert_eq!(p.as_str(), "secretpassword123");
    }

    #[test]
    fn password_rejects_short() {
        assert!(matches!(
            Password::new("short"),
            Err(PasswordError::TooShort)
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
