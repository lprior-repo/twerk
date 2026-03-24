//! User-related domain types and operations

use serde::{Deserialize, Serialize};
use time::OffsetDateTime;

/// Username key type for context
pub type UsernameKey = &'static str;

/// Guest username constant
pub const USER_GUEST: UsernameKey = "guest";

/// Username context key
pub const USERNAME: UsernameKey = "username";

/// Authenticated username stored in request extensions by auth middleware.
///
/// Handlers extract this from `axum::http::Extensions` to determine the
/// current user for permission-scoped queries (e.g., `get_jobs`).
#[derive(Clone, Debug)]
pub struct UsernameValue(pub String);

impl User {
    /// Creates a deep clone of this user.
    ///
    /// Matches Go's `User.Clone()` which explicitly excludes the `Password`
    /// field to prevent credential leakage during cloning.
    #[must_use]
    pub fn deep_clone(&self) -> Self {
        Self {
            id: self.id.clone(),
            name: self.name.clone(),
            username: self.username.clone(),
            password_hash: self.password_hash.clone(),
            password: None,
            created_at: self.created_at,
            disabled: self.disabled,
        }
    }
}

/// User represents a user in the system
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct User {
    /// Unique identifier
    pub id: Option<String>,
    /// Display name
    pub name: Option<String>,
    /// Username for authentication
    pub username: Option<String>,
    /// Hashed password (never serialized)
    #[serde(skip)]
    pub password_hash: Option<String>,
    /// Plain password (never serialized)
    #[serde(skip)]
    pub password: Option<String>,
    /// When the user was created
    #[serde(skip_serializing_if = "Option::is_none")]
    pub created_at: Option<OffsetDateTime>,
    /// Whether the user is disabled
    #[serde(default)]
    pub disabled: bool,
}
