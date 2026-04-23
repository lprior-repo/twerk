//! User, Role, and permission record types and conversions to domain types.

use sqlx::FromRow;

use crate::datastore::Error as DatastoreError;
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
    fn to_user(&self) -> Result<User, DatastoreError>;
}

impl UserRecordExt for UserRecord {
    fn to_user(&self) -> Result<User, DatastoreError> {
        Ok(User {
            id: Some(
                UserId::new(self.id.clone())
                    .map_err(|e| DatastoreError::InvalidId(e.to_string()))?,
            ),
            name: Some(self.name.clone()),
            username: Some(self.username_.clone()),
            password_hash: Some(self.password_.clone()),
            password: None,
            created_at: Some(self.created_at),
            disabled: self.is_disabled,
        })
    }
}

/// Extension trait for RoleRecord conversions
pub trait RoleRecordExt {
    /// Converts the database record to a Role domain object.
    fn to_role(&self) -> Result<Role, DatastoreError>;
}

impl RoleRecordExt for RoleRecord {
    fn to_role(&self) -> Result<Role, DatastoreError> {
        Ok(Role {
            id: Some(
                RoleId::new(self.id.clone())
                    .map_err(|e| DatastoreError::InvalidId(e.to_string()))?,
            ),
            slug: Some(self.slug.clone()),
            name: Some(self.name.clone()),
            created_at: Some(self.created_at),
        })
    }
}


