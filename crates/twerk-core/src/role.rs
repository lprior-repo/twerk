use crate::id::RoleId;
use serde::{Deserialize, Serialize};
use time::OffsetDateTime;
use utoipa::ToSchema;

pub const ROLE_PUBLIC: &str = "public";

<<<<<<< HEAD
<<<<<<< HEAD
#[derive(Clone, Debug, Serialize, Deserialize, Default, PartialEq, Eq, utoipa::ToSchema)]
=======
#[derive(Clone, Debug, Serialize, Deserialize, Default, PartialEq, Eq, ToSchema)]
>>>>>>> origin/tw-polecat/tau
=======
#[derive(Clone, Debug, Serialize, Deserialize, Default, PartialEq, Eq, ToSchema)]
>>>>>>> origin/tw-polecat/upsilon
pub struct Role {
    #[serde(skip_serializing_if = "Option::is_none")]
    #[schema(value_type = String)]
    pub id: Option<RoleId>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub slug: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub created_at: Option<OffsetDateTime>,
}

impl Role {
    #[must_use]
    pub fn is_public(&self) -> bool {
        self.slug.as_deref() == Some(ROLE_PUBLIC)
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use rstest::rstest;

    // ── is_public ───────────────────────────────────────────────

    #[rstest]
    #[case(Some(ROLE_PUBLIC), true)]
    #[case(Some("admin"), false)]
    #[case(None, false)]
    fn is_public_returns_expected_value(#[case] slug: Option<&str>, #[case] expected: bool) {
        let role = Role {
            slug: slug.map(String::from),
            ..Role::default()
        };
        assert_eq!(role.is_public(), expected);
    }

    #[test]
    fn is_public_returns_true_when_slug_is_exact_public() {
        let role = Role {
            slug: Some("public".to_string()),
            ..Role::default()
        };
        assert!(role.is_public());
    }

    #[test]
    fn is_public_returns_false_when_slug_is_public_with_spaces() {
        let role = Role {
            slug: Some(" public ".to_string()),
            ..Role::default()
        };
        assert!(!role.is_public());
    }

    #[test]
    fn is_public_returns_false_when_slug_is_public_uppercase() {
        let role = Role {
            slug: Some("PUBLIC".to_string()),
            ..Role::default()
        };
        assert!(!role.is_public());
    }

    #[test]
    fn is_public_returns_false_when_slug_is_empty_string() {
        let role = Role {
            slug: Some(String::new()),
            ..Role::default()
        };
        assert!(!role.is_public());
    }

    // ── clone / equality ────────────────────────────────────────

    #[test]
    fn clone_preserves_all_fields() {
        let original = Role {
            id: Some(RoleId::new("role-123").unwrap()),
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
    fn partial_eq_matches_for_identical_roles() {
        let role1 = Role {
            id: Some(RoleId::new("r1").unwrap()),
            slug: Some("s1".to_string()),
            ..Role::default()
        };
        let role2 = role1.clone();
        assert_eq!(role1, role2);
    }

    #[test]
    fn partial_eq_returns_false_for_different_ids() {
        let r1 = Role {
            id: Some(RoleId::new("r1").unwrap()),
            ..Role::default()
        };
        let r2 = Role {
            id: Some(RoleId::new("r2").unwrap()),
            ..Role::default()
        };
        assert_ne!(r1, r2);
    }

    #[test]
    fn partial_eq_returns_false_for_different_slugs() {
        let r1 = Role {
            slug: Some("admin".to_string()),
            ..Role::default()
        };
        let r2 = Role {
            slug: Some("viewer".to_string()),
            ..Role::default()
        };
        assert_ne!(r1, r2);
    }

    // ── debug ───────────────────────────────────────────────────

    #[test]
    fn debug_format_contains_fields() {
        let role = Role {
            id: Some(RoleId::new("r1").unwrap()),
            slug: Some("s1".to_string()),
            ..Role::default()
        };
        let debug = format!("{role:?}");
        assert!(debug.contains("r1"));
        assert!(debug.contains("s1"));
    }

    #[test]
    fn debug_format_on_default_shows_none_fields() {
        let role = Role::default();
        let debug = format!("{role:?}");
        assert!(debug.contains("None"));
    }

    // ── serialization ───────────────────────────────────────────

    #[test]
    fn serialization_roundtrip_preserves_all_fields() {
        let role = Role {
            id: Some(RoleId::new("role-1").unwrap()),
            slug: Some("admin".to_string()),
            name: Some("Admin".to_string()),
            created_at: Some(OffsetDateTime::now_utc()),
        };
        let serialized = serde_json::to_string(&role).unwrap();
        let deserialized: Role = serde_json::from_str(&serialized).unwrap();
        assert_eq!(role, deserialized);
    }

    #[test]
    fn serialization_omits_none_fields() {
        let role = Role::default();
        let serialized = serde_json::to_string(&role).unwrap();
        assert!(!serialized.contains("id"));
        assert!(!serialized.contains("slug"));
        assert!(!serialized.contains("name"));
    }

    #[test]
    fn serialization_includes_slug_when_some() {
        let role = Role {
            slug: Some("editor".to_string()),
            ..Role::default()
        };
        let json = serde_json::to_string(&role).unwrap();
        assert!(json.contains("editor"));
    }

    // ── RoleId newtype ──────────────────────────────────────────

    #[test]
    fn id_newtype_preserves_value() {
        let id = RoleId::new("abc").unwrap();
        assert_eq!(id.as_str(), "abc");
    }

    #[test]
    fn id_newtype_from_string() {
        let id: RoleId = "from-str".to_string().into();
        assert_eq!(id.as_str(), "from-str");
    }

    #[test]
    fn id_display_matches_inner() {
        let id = RoleId::new("display-test").unwrap();
        assert_eq!(format!("{id}"), "display-test");
    }

    // ── constants ───────────────────────────────────────────────

    #[test]
    fn role_public_constant_is_public() {
        assert_eq!(ROLE_PUBLIC, "public");
    }

    // ── edge cases ──────────────────────────────────────────────

    #[test]
    fn role_with_unicode_name_roundtrips() {
        let role = Role {
            name: Some("管理者 🛡️".to_string()),
            ..Role::default()
        };
        let json = serde_json::to_string(&role).unwrap();
        let restored: Role = serde_json::from_str(&json).unwrap();
        assert_eq!(restored.name, role.name);
    }

    #[test]
    fn role_with_all_none_fields_is_default() {
        let role = Role {
            id: None,
            slug: None,
            name: None,
            created_at: None,
        };
        assert_eq!(role, Role::default());
    }

    #[test]
    fn role_clone_preserves_created_at() {
        let now = OffsetDateTime::now_utc();
        let role = Role {
            created_at: Some(now),
            ..Role::default()
        };
        let cloned = role.clone();
        assert_eq!(cloned.created_at, Some(now));
    }

    #[test]
    fn is_public_only_checks_slug_not_name() {
        let role = Role {
            name: Some("public".to_string()),
            slug: Some("admin".to_string()),
            ..Role::default()
        };
        assert!(!role.is_public());
    }

    #[test]
    fn default_role_is_not_public() {
        assert!(!Role::default().is_public());
    }
}
