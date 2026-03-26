use serde::{Deserialize, Serialize};
use time::OffsetDateTime;

pub const ROLE_PUBLIC: &str = "public";

#[derive(Clone, Debug, Serialize, Deserialize, Default, PartialEq, Eq)]
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
}
