//! Role-related domain types

use serde::{Deserialize, Serialize};
use time::OffsetDateTime;

/// Public role constant
pub const ROLE_PUBLIC: &str = "public";

impl Role {
    /// Creates a deep clone of this role
    #[must_use]
    pub fn deep_clone(&self) -> Self {
        Self {
            id: self.id.clone(),
            slug: self.slug.clone(),
            name: self.name.clone(),
            created_at: self.created_at,
        }
    }
}

/// Role represents a role in the system
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Role {
    /// Unique identifier
    pub id: Option<String>,
    /// Role slug (e.g., "admin", "user")
    pub slug: Option<String>,
    /// Display name
    pub name: Option<String>,
    /// When the role was created
    #[serde(skip_serializing_if = "Option::is_none")]
    pub created_at: Option<OffsetDateTime>,
}

impl UserRole {
    /// Creates a deep clone of this user role
    #[must_use]
    pub fn deep_clone(&self) -> Self {
        Self {
            id: self.id.clone(),
            user_id: self.user_id.clone(),
            role_id: self.role_id.clone(),
            created_at: self.created_at,
        }
    }
}

/// UserRole represents the assignment of a role to a user
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UserRole {
    /// Unique identifier
    pub id: Option<String>,
    /// User ID
    pub user_id: Option<String>,
    /// Role ID
    pub role_id: Option<String>,
    /// When the assignment was created
    #[serde(skip_serializing_if = "Option::is_none")]
    pub created_at: Option<OffsetDateTime>,
}
