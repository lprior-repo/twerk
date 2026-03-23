use serde::{Deserialize, Serialize};
use time::OffsetDateTime;

pub type UsernameKey = String;

pub const USER_GUEST: &str = "guest";
pub const USERNAME: UsernameKey = UsernameKey::from("username");

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct User {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub username: Option<String>,
    #[serde(skip)]
    pub password_hash: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub password: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub created_at: Option<OffsetDateTime>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub disabled: Option<bool>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_user_clone() {
        let user = User {
            id: Some("123".to_string()),
            name: Some("Alice".to_string()),
            username: Some("alice".to_string()),
            password_hash: "secret_hash".to_string(),
            password: Some("secret".to_string()),
            created_at: Some(OffsetDateTime::now_utc()),
            disabled: Some(false),
        };

        let cloned = user.clone();

        assert_eq!(cloned.id, user.id);
        assert_eq!(cloned.name, user.name);
        assert_eq!(cloned.username, user.username);
        assert_eq!(cloned.password_hash, user.password_hash);
        assert_eq!(cloned.password, user.password);
        assert_eq!(cloned.created_at, user.created_at);
        assert_eq!(cloned.disabled, user.disabled);

        // Ensure it's a true deep clone (different references)
        assert_ne!(std::ptr::addr_of!(user), std::ptr::addr_of!(cloned));
    }

    #[test]
    fn test_user_clone_empty() {
        let user = User {
            id: None,
            name: None,
            username: None,
            password_hash: String::new(),
            password: None,
            created_at: None,
            disabled: None,
        };

        let cloned = user.clone();

        assert_eq!(cloned.id, None);
        assert_eq!(cloned.name, None);
        assert_eq!(cloned.username, None);
        assert_eq!(cloned.password_hash, String::new());
        assert_eq!(cloned.password, None);
        assert_eq!(cloned.created_at, None);
        assert_eq!(cloned.disabled, None);
    }
}
