use serde::{Deserialize, Serialize};
use time::OffsetDateTime;

pub type UsernameKey = &'static str;
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UsernameValue(pub String);

pub const USER_GUEST: &str = "guest";
pub const USERNAME: UsernameKey = "username";

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct User {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub username: Option<String>,
    #[serde(skip)]
    pub password_hash: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub password: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub created_at: Option<OffsetDateTime>,
    #[serde(default)]
    pub disabled: bool,
}

impl User {
    #[must_use]
    pub fn deep_clone(&self) -> User {
        let mut cloned = self.clone();
        cloned.password = None;
        cloned
    }
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
            password_hash: Some("secret_hash".to_string()),
            password: Some("secret".to_string()),
            created_at: Some(OffsetDateTime::now_utc()),
            disabled: false,
        };

        let cloned = user.deep_clone();

        assert_eq!(cloned.id, user.id);
        assert_eq!(cloned.name, user.name);
        assert_eq!(cloned.username, user.username);
        assert_eq!(cloned.password_hash, user.password_hash);
        assert!(cloned.password.is_none());
        assert_eq!(cloned.created_at, user.created_at);
        assert_eq!(cloned.disabled, user.disabled);
    }
}
