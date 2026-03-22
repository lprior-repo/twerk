//! Tests for the user module

#[cfg(test)]
mod tests {
    use crate::user::{User, USERNAME, USER_GUEST};

    #[test]
    fn test_deep_clone_excludes_password() {
        let user = User {
            id: Some("user-1".to_string()),
            name: Some("Test User".to_string()),
            username: Some("testuser".to_string()),
            password_hash: Some("hashed-pw".to_string()),
            password: Some("secret-password".to_string()),
            created_at: None,
            disabled: false,
        };

        let cloned = user.deep_clone();

        // Password must be excluded from deep_clone (matches Go's User.Clone())
        assert_eq!(cloned.id, user.id);
        assert_eq!(cloned.name, user.name);
        assert_eq!(cloned.username, user.username);
        assert_eq!(cloned.password_hash, user.password_hash);
        assert!(
            cloned.password.is_none(),
            "deep_clone must exclude password"
        );
    }

    #[test]
    fn test_deep_clone_preserves_other_fields() {
        let user = User {
            id: Some("u-2".to_string()),
            name: Some("Disabled User".to_string()),
            username: Some("disabled".to_string()),
            password_hash: None,
            password: Some("should-be-excluded".to_string()),
            created_at: None,
            disabled: true,
        };

        let cloned = user.deep_clone();

        assert_eq!(cloned.id, Some("u-2".to_string()));
        assert_eq!(cloned.name, Some("Disabled User".to_string()));
        assert_eq!(cloned.username, Some("disabled".to_string()));
        assert!(cloned.disabled);
    }

    #[test]
    fn test_guest_constant() {
        assert_eq!(USER_GUEST, "guest");
    }

    #[test]
    fn test_username_constant() {
        assert_eq!(USERNAME, "username");
    }
}
