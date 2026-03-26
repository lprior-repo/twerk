//! User, Role, and permission record types and conversions to domain types.

use sqlx::FromRow;

use twerk_core::{
    id::{RoleId, UserId},
    role::Role,
    user::User,
};

/// Job permission record from the database
#[derive(Debug, Clone, FromRow)]
pub struct JobPermRecord {
    pub id: String,
    pub job_id: String,
    pub user_id: Option<String>,
    pub role_id: Option<String>,
    pub created_at: Option<time::OffsetDateTime>,
}

/// Scheduled job permission record from the database
#[derive(Debug, Clone, FromRow)]
pub struct ScheduledPermRecord {
    pub id: String,
    pub scheduled_job_id: String,
    pub user_id: Option<String>,
    pub role_id: Option<String>,
    pub created_at: Option<time::OffsetDateTime>,
}

/// User record from the database
#[derive(Debug, Clone, FromRow)]
pub struct UserRecord {
    pub id: String,
    pub name: String,
    pub username_: String,
    pub password_: String,
    pub created_at: time::OffsetDateTime,
    pub is_disabled: bool,
}

/// Role record from the database
#[derive(Debug, Clone, FromRow)]
pub struct RoleRecord {
    pub id: String,
    pub slug: String,
    pub name: String,
    pub created_at: time::OffsetDateTime,
}

/// Extension trait for UserRecord conversions
pub trait UserRecordExt {
    /// Converts the database record to a User domain object.
    #[must_use]
    fn to_user(&self) -> User;
}

impl UserRecordExt for UserRecord {
    fn to_user(&self) -> User {
        User {
            id: Some(UserId::new(self.id.clone())),
            name: Some(self.name.clone()),
            username: Some(self.username_.clone()),
            password_hash: Some(self.password_.clone()),
            password: None,
            created_at: Some(self.created_at),
            disabled: self.is_disabled,
        }
    }
}

/// Extension trait for RoleRecord conversions
pub trait RoleRecordExt {
    /// Converts the database record to a Role domain object.
    #[must_use]
    fn to_role(&self) -> Role;
}

impl RoleRecordExt for RoleRecord {
    fn to_role(&self) -> Role {
        Role {
            id: Some(RoleId::new(self.id.clone())),
            slug: Some(self.slug.clone()),
            name: Some(self.name.clone()),
            created_at: Some(self.created_at),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── Helpers ──────────────────────────────────────────────────────────

    /// Creates a fixed-point timestamp for deterministic tests.
    fn fixed_now() -> time::OffsetDateTime {
        time::OffsetDateTime::new_utc(
            time::Date::from_calendar_date(2026, time::Month::March, 22).unwrap_or_else(|_| {
                time::Date::from_calendar_date(2026, time::Month::January, 1).unwrap()
            }),
            time::Time::from_hms(12, 0, 0).unwrap_or_else(|_| time::Time::MIDNIGHT),
        )
    }

    // ── UserRecord → User conversion tests ──────────────────────────────

    #[test]
    fn user_record_to_user_basic_fields() {
        let now = fixed_now();
        let record = UserRecord {
            id: "user-001".to_string(),
            name: "Test User".to_string(),
            username_: "testuser".to_string(),
            password_: "$2b$12$hashed".to_string(),
            created_at: now,
            is_disabled: false,
        };
        let user = record.to_user();

        assert_eq!(user.id.as_deref(), Some("user-001"));
        assert_eq!(user.name.as_deref(), Some("Test User"));
        assert_eq!(user.username.as_deref(), Some("testuser"));
        assert_eq!(user.password_hash.as_deref(), Some("$2b$12$hashed"));
        assert!(user.password.is_none()); // password should never be set from record
        assert!(!user.disabled);
    }

    #[test]
    fn user_record_to_user_disabled() {
        let now = fixed_now();
        let record = UserRecord {
            id: "user-002".to_string(),
            name: "Banned".to_string(),
            username_: "banned".to_string(),
            password_: "".to_string(),
            created_at: now,
            is_disabled: true,
        };
        let user = record.to_user();

        assert!(user.disabled);
    }

    // ── RoleRecord → Role conversion tests ──────────────────────────────

    #[test]
    fn role_record_to_role_basic_fields() {
        let now = fixed_now();
        let record = RoleRecord {
            id: "role-001".to_string(),
            slug: "admin".to_string(),
            name: "Administrator".to_string(),
            created_at: now,
        };
        let role = record.to_role();

        assert_eq!(role.id.as_deref(), Some("role-001"));
        assert_eq!(role.slug.as_deref(), Some("admin"));
        assert_eq!(role.name.as_deref(), Some("Administrator"));
    }
}
