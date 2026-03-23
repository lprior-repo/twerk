use serde::{Deserialize, Serialize};
use time::OffsetDateTime;

pub const ROLE_PUBLIC: &str = "public";

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Role {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub slug: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub created_at: Option<OffsetDateTime>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct UserRole {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub user_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub role_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub created_at: Option<OffsetDateTime>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_role_clone() {
        let original = Role {
            id: Some("role-123".to_string()),
            slug: Some("admin".to_string()),
            name: Some("Administrator".to_string()),
            created_at: None,
        };

        let cloned = original.clone();

        assert_eq!(cloned.id, original.id);
        assert_eq!(cloned.slug, original.slug);
        assert_eq!(cloned.name, original.name);
        assert_eq!(cloned.created_at, original.created_at);
    }

    #[test]
    fn test_user_role_clone() {
        let original = UserRole {
            id: Some("ur-456".to_string()),
            user_id: Some("user-789".to_string()),
            role_id: Some("role-123".to_string()),
            created_at: None,
        };

        let cloned = original.clone();

        assert_eq!(cloned.id, original.id);
        assert_eq!(cloned.user_id, original.user_id);
        assert_eq!(cloned.role_id, original.role_id);
        assert_eq!(cloned.created_at, original.created_at);
    }
}
