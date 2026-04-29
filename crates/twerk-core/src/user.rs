use crate::id::UserId;
use serde::{Deserialize, Serialize};
use time::OffsetDateTime;
use utoipa::ToSchema;

pub type UsernameKey = &'static str;
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UsernameValue(pub String);

pub const USER_GUEST: &str = "guest";
pub const USERNAME: UsernameKey = "username";

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq, Eq, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct User {
    #[serde(skip_serializing_if = "Option::is_none")]
    #[schema(value_type = String)]
    pub id: Option<UserId>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub username: Option<String>,
    #[serde(skip_serializing)]
    pub password_hash: Option<String>,
    #[serde(skip)]
    pub password: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[serde(with = "time::serde::rfc3339::option")]
    pub created_at: Option<OffsetDateTime>,
    #[serde(default)]
    pub disabled: bool,
}

impl User {
    #[must_use]
    pub fn deep_clone(&self) -> User {
        User {
            password: None,
            ..self.clone()
        }
    }

    #[must_use]
    pub fn is_guest(&self) -> bool {
        self.username.as_deref() == Some(USER_GUEST)
    }

    #[must_use]
    pub fn is_enabled(&self) -> bool {
        !self.disabled
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use rstest::rstest;

    // ── deep_clone ──────────────────────────────────────────────

    #[test]
    fn deep_clone_removes_password_when_password_is_some() {
        let user = User {
            id: Some(UserId::new("123").unwrap()),
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

    #[test]
    fn deep_clone_returns_none_password_when_password_is_none() {
        let user = User {
            id: Some(UserId::new("123").unwrap()),
            password: None,
            ..User::default()
        };

        let cloned = user.deep_clone();
        assert!(cloned.password.is_none());
        assert_eq!(cloned.id, user.id);
    }

    #[test]
    fn deep_clone_preserves_all_fields_except_password() {
        let now = OffsetDateTime::now_utc();
        let user = User {
            id: Some(UserId::new("id-1").unwrap()),
            name: Some("Bob".to_string()),
            username: Some("bob".to_string()),
            password_hash: Some("hash".to_string()),
            password: Some("pw".to_string()),
            created_at: Some(now),
            disabled: true,
        };

        let cloned = user.deep_clone();

        assert_eq!(cloned.id, Some(UserId::new("id-1").unwrap()));
        assert_eq!(cloned.name, Some("Bob".to_string()));
        assert_eq!(cloned.username, Some("bob".to_string()));
        assert_eq!(cloned.password_hash, Some("hash".to_string()));
        assert!(cloned.password.is_none());
        assert_eq!(cloned.created_at, Some(now));
        assert!(cloned.disabled);
    }

    #[test]
    fn deep_clone_preserves_disabled_true() {
        let user = User {
            disabled: true,
            ..User::default()
        };
        let cloned = user.deep_clone();
        assert!(cloned.disabled);
        assert!(!cloned.is_enabled());
    }

    #[test]
    fn deep_clone_preserves_disabled_false() {
        let user = User {
            disabled: false,
            ..User::default()
        };
        let cloned = user.deep_clone();
        assert!(!cloned.disabled);
        assert!(cloned.is_enabled());
    }

    #[test]
    fn deep_clone_preserves_empty_strings() {
        let user = User {
            name: Some(String::new()),
            username: Some(String::new()),
            ..User::default()
        };
        let cloned = user.deep_clone();
        assert_eq!(cloned.name, Some(String::new()));
        assert_eq!(cloned.username, Some(String::new()));
    }

    #[test]
    fn deep_clone_on_default_user_produces_default_except_password() {
        let user = User::default();
        let cloned = user.deep_clone();
        assert!(cloned.id.is_none());
        assert!(cloned.name.is_none());
        assert!(cloned.username.is_none());
        assert!(cloned.password_hash.is_none());
        assert!(cloned.password.is_none());
        assert!(cloned.created_at.is_none());
        assert!(!cloned.disabled);
    }

    // ── is_guest ────────────────────────────────────────────────

    #[rstest]
    #[case(Some(USER_GUEST), true)]
    #[case(Some("alice"), false)]
    #[case(None, false)]
    fn is_guest_returns_expected_value(#[case] username: Option<&str>, #[case] expected: bool) {
        let user = User {
            username: username.map(String::from),
            ..User::default()
        };
        assert_eq!(user.is_guest(), expected);
    }

    #[test]
    fn is_guest_returns_true_when_username_is_exact_guest() {
        let user = User {
            username: Some("guest".to_string()),
            ..User::default()
        };
        assert!(user.is_guest());
    }

    #[test]
    fn is_guest_returns_false_when_username_is_guest_with_spaces() {
        let user = User {
            username: Some(" guest ".to_string()),
            ..User::default()
        };
        assert!(!user.is_guest());
    }

    #[test]
    fn is_guest_returns_false_when_username_is_guest_uppercase() {
        let user = User {
            username: Some("GUEST".to_string()),
            ..User::default()
        };
        assert!(!user.is_guest());
    }

    #[test]
    fn is_guest_returns_false_when_username_is_empty_string() {
        let user = User {
            username: Some(String::new()),
            ..User::default()
        };
        assert!(!user.is_guest());
    }

    // ── is_enabled ──────────────────────────────────────────────

    #[rstest]
    #[case(false, true)]
    #[case(true, false)]
    fn is_enabled_returns_expected_value(#[case] disabled: bool, #[case] expected: bool) {
        let user = User {
            disabled,
            ..User::default()
        };
        assert_eq!(user.is_enabled(), expected);
    }

    #[test]
    fn default_user_is_enabled() {
        let user = User::default();
        assert!(user.is_enabled());
        assert!(!user.disabled);
    }

    // ── serialization ───────────────────────────────────────────

    #[test]
    fn serialization_roundtrip_preserves_all_public_fields() {
        let user = User {
            id: Some(UserId::new("123").unwrap()),
            name: Some("Alice".to_string()),
            username: Some("alice".to_string()),
            ..User::default()
        };
        let serialized = serde_json::to_string(&user).unwrap();
        let deserialized: User = serde_json::from_str(&serialized).unwrap();
        assert_eq!(user, deserialized);
    }

    #[test]
    fn password_hash_is_skipped_in_serialization() {
        let user = User {
            password_hash: Some("hidden".to_string()),
            ..User::default()
        };
        let serialized = serde_json::to_string(&user).unwrap();
        assert!(!serialized.contains("passwordHash"));
        assert!(!serialized.contains("hidden"));
    }

    #[test]
    fn password_is_skipped_in_serialization() {
        let user = User {
            password: Some("plaintext".to_string()),
            ..User::default()
        };
        let serialized = serde_json::to_string(&user).unwrap();
        assert!(!serialized.contains("plaintext"));
    }

    #[test]
    fn serialization_omits_none_fields() {
        let user = User::default();
        let serialized = serde_json::to_string(&user).unwrap();
        assert!(!serialized.contains("id"));
        assert!(!serialized.contains("name"));
        assert!(!serialized.contains("username"));
    }

    #[test]
    fn serialization_includes_disabled_when_true() {
        let user = User {
            disabled: true,
            ..User::default()
        };
        let serialized = serde_json::to_string(&user).unwrap();
        assert!(serialized.contains("disabled"));
    }

    #[test]
    fn serialization_roundtrip_with_all_fields_populated() {
        let now = OffsetDateTime::now_utc();
        let user = User {
            id: Some(UserId::new("uid-1").unwrap()),
            name: Some("Full User".to_string()),
            username: Some("fulluser".to_string()),
            password_hash: Some("hash".to_string()),
            password: Some("pw".to_string()),
            created_at: Some(now),
            disabled: true,
        };
        let json = serde_json::to_string(&user).unwrap();
        let restored: User = serde_json::from_str(&json).unwrap();
        assert_eq!(restored.id, user.id);
        assert_eq!(restored.name, user.name);
        assert_eq!(restored.username, user.username);
        assert_eq!(restored.created_at, user.created_at);
        assert_eq!(restored.disabled, user.disabled);
        // password_hash is skip_serializing — not emitted to JSON, so not restored after roundtrip
        assert!(restored.password_hash.is_none());
        // password is fully skipped (both serialization and deserialization)
        assert!(restored.password.is_none());
    }

    // ── equality / clone / debug ────────────────────────────────

    #[test]
    fn partial_eq_matches_for_identical_users() {
        let u1 = User {
            id: Some(UserId::new("u1").unwrap()),
            ..User::default()
        };
        let u2 = u1.clone();
        assert_eq!(u1, u2);
    }

    #[test]
    fn partial_eq_returns_false_for_different_ids() {
        let u1 = User {
            id: Some(UserId::new("u1").unwrap()),
            ..User::default()
        };
        let u2 = User {
            id: Some(UserId::new("u2").unwrap()),
            ..User::default()
        };
        assert_ne!(u1, u2);
    }

    #[test]
    fn clone_preserves_password() {
        let user = User {
            password: Some("secret".to_string()),
            ..User::default()
        };
        let cloned = user.clone();
        assert_eq!(cloned.password, Some("secret".to_string()));
    }

    #[test]
    fn debug_format_contains_fields() {
        let user = User {
            id: Some(UserId::new("u1").unwrap()),
            username: Some("alice".to_string()),
            ..User::default()
        };
        let debug = format!("{user:?}");
        assert!(debug.contains("u1"));
        assert!(debug.contains("alice"));
    }

    #[test]
    fn debug_format_on_default_shows_none_fields() {
        let user = User::default();
        let debug = format!("{user:?}");
        assert!(debug.contains("None"));
    }

    // ── UserId newtype ──────────────────────────────────────────

    #[test]
    fn id_newtype_preserves_value() {
        let id = UserId::new("abc").unwrap();
        assert_eq!(id.as_str(), "abc");
    }

    #[test]
    fn id_newtype_from_string() {
        let id: UserId = "from-str".to_string().into();
        assert_eq!(id.as_str(), "from-str");
    }

    #[test]
    fn id_newtype_from_str_ref() {
        let id: UserId = "from-str-ref".into();
        assert_eq!(id.as_str(), "from-str-ref");
    }

    #[test]
    fn id_display_matches_inner() {
        let id = UserId::new("display-test").unwrap();
        assert_eq!(format!("{id}"), "display-test");
    }

    // ── constants ───────────────────────────────────────────────

    #[test]
    fn user_guest_constant_is_guest() {
        assert_eq!(USER_GUEST, "guest");
    }

    #[test]
    fn username_constant_is_username() {
        assert_eq!(USERNAME, "username");
    }

    // ── edge cases ──────────────────────────────────────────────

    #[test]
    fn user_with_unicode_name_roundtrips() {
        let user = User {
            name: Some("🦀 Rustacean 日本語".to_string()),
            ..User::default()
        };
        let json = serde_json::to_string(&user).unwrap();
        let restored: User = serde_json::from_str(&json).unwrap();
        assert_eq!(restored.name, Some("🦀 Rustacean 日本語".to_string()));
    }

    #[test]
    fn user_with_very_long_username() {
        let long = "a".repeat(10_000);
        let user = User {
            username: Some(long.clone()),
            ..User::default()
        };
        assert_eq!(user.username, Some(long));
    }

    #[test]
    fn user_with_special_characters_in_fields() {
        let user = User {
            name: Some("<script>alert('xss')</script>".to_string()),
            username: Some("user@example.com".to_string()),
            ..User::default()
        };
        let json = serde_json::to_string(&user).unwrap();
        let restored: User = serde_json::from_str(&json).unwrap();
        assert_eq!(restored.name, user.name);
        assert_eq!(restored.username, user.username);
    }

    #[test]
    fn is_guest_and_is_enabled_are_independent() {
        let guest_disabled = User {
            username: Some(USER_GUEST.to_string()),
            disabled: true,
            ..User::default()
        };
        assert!(guest_disabled.is_guest());
        assert!(!guest_disabled.is_enabled());

        let non_guest_enabled = User {
            username: Some("admin".to_string()),
            disabled: false,
            ..User::default()
        };
        assert!(!non_guest_enabled.is_guest());
        assert!(non_guest_enabled.is_enabled());
    }
}
