//! User and role operations for `PostgresDatastore`.

use sqlx::Postgres;
use twerk_core::role::Role;
use twerk_core::user::User;

use crate::datastore::postgres::records::{RoleRecord, RoleRecordExt, UserRecord, UserRecordExt};
use crate::datastore::postgres::{DatastoreError, DatastoreResult, Executor, PostgresDatastore};

impl PostgresDatastore {
    pub(super) async fn create_user_impl(&self, user: &User) -> DatastoreResult<()> {
        let id = user.id.as_ref().ok_or(DatastoreError::InvalidInput(
            "user ID is required".to_string(),
        ))?;
        let username = user.username.as_ref().ok_or(DatastoreError::InvalidInput(
            "username is required".to_string(),
        ))?;
        let name = user.name.as_deref().unwrap_or("");
        let password_hash = user
            .password_hash
            .as_ref()
            .ok_or(DatastoreError::InvalidInput(
                "password_hash is required".to_string(),
            ))?;

        let q = r"INSERT INTO users (id, name, username_, password_, created_at, is_disabled) VALUES ($1, $2, $3, $4, $5, $6)";
        let query = sqlx::query(q)
            .bind(&**id)
            .bind(name)
            .bind(username)
            .bind(password_hash)
            .bind(
                user.created_at
                    .unwrap_or_else(time::OffsetDateTime::now_utc),
            )
            .bind(user.disabled);

        match &self.executor {
            Executor::Pool(p) => {
                query
                    .execute(p)
                    .await
                    .map_err(|e| DatastoreError::Database(format!("create user failed: {e}")))?;
            }
            Executor::Tx(tx) => {
                let mut tx = tx.lock().await;
                query
                    .execute(&mut **tx)
                    .await
                    .map_err(|e| DatastoreError::Database(format!("create user failed: {e}")))?;
            }
        }
        Ok(())
    }

    pub(super) async fn get_user_impl(&self, identifier: &str) -> DatastoreResult<User> {
        let record: UserRecord = match &self.executor {
            Executor::Pool(p) => {
                sqlx::query_as::<Postgres, UserRecord>(
                    "SELECT * FROM users WHERE username_ = $1 OR id = $1",
                )
                .bind(identifier)
                .fetch_optional(p)
                .await
            }
            Executor::Tx(tx) => {
                let mut tx = tx.lock().await;
                sqlx::query_as::<Postgres, UserRecord>(
                    "SELECT * FROM users WHERE username_ = $1 OR id = $1",
                )
                .bind(identifier)
                .fetch_optional(&mut **tx)
                .await
            }
        }
        .map_err(|e| DatastoreError::Database(format!("get user failed: {e}")))?
        .ok_or(DatastoreError::UserNotFound)?;
        Ok(record.to_user())
    }

    pub(super) async fn create_role_impl(&self, role: &Role) -> DatastoreResult<()> {
        let id = role.id.as_ref().ok_or(DatastoreError::InvalidInput(
            "role ID is required".to_string(),
        ))?;
        let slug = role.slug.as_ref().ok_or(DatastoreError::InvalidInput(
            "role slug is required".to_string(),
        ))?;
        let name = role.name.as_deref().unwrap_or("");

        let q = r"INSERT INTO roles (id, slug, name, created_at) VALUES ($1, $2, $3, $4)";
        let query = sqlx::query(q).bind(&**id).bind(slug).bind(name).bind(
            role.created_at
                .unwrap_or_else(time::OffsetDateTime::now_utc),
        );

        match &self.executor {
            Executor::Pool(p) => {
                query
                    .execute(p)
                    .await
                    .map_err(|e| DatastoreError::Database(format!("create role failed: {e}")))?;
            }
            Executor::Tx(tx) => {
                let mut tx = tx.lock().await;
                query
                    .execute(&mut **tx)
                    .await
                    .map_err(|e| DatastoreError::Database(format!("create role failed: {e}")))?;
            }
        }
        Ok(())
    }

    pub(super) async fn get_role_impl(&self, id: &str) -> DatastoreResult<Role> {
        let record: RoleRecord = match &self.executor {
            Executor::Pool(p) => {
                sqlx::query_as::<Postgres, RoleRecord>("SELECT * FROM roles WHERE id = $1")
                    .bind(id)
                    .fetch_optional(p)
                    .await
            }
            Executor::Tx(tx) => {
                let mut tx = tx.lock().await;
                sqlx::query_as::<Postgres, RoleRecord>("SELECT * FROM roles WHERE id = $1")
                    .bind(id)
                    .fetch_optional(&mut **tx)
                    .await
            }
        }
        .map_err(|e| DatastoreError::Database(format!("get role failed: {e}")))?
        .ok_or(DatastoreError::RoleNotFound)?;
        Ok(record.to_role())
    }

    pub(super) async fn get_roles_impl(&self) -> DatastoreResult<Vec<Role>> {
        let records: Vec<RoleRecord> = match &self.executor {
            Executor::Pool(p) => {
                sqlx::query_as::<Postgres, RoleRecord>("SELECT * FROM roles ORDER BY name ASC")
                    .fetch_all(p)
                    .await
            }
            Executor::Tx(tx) => {
                let mut tx = tx.lock().await;
                sqlx::query_as::<Postgres, RoleRecord>("SELECT * FROM roles ORDER BY name ASC")
                    .fetch_all(&mut **tx)
                    .await
            }
        }
        .map_err(|e| DatastoreError::Database(format!("get roles failed: {e}")))?;
        Ok(records.into_iter().map(|r| r.to_role()).collect())
    }

    pub(super) async fn get_user_roles_impl(&self, user_id: &str) -> DatastoreResult<Vec<Role>> {
        let records: Vec<RoleRecord> = match &self.executor {
            Executor::Pool(p) => {
                sqlx::query_as::<Postgres, RoleRecord>(
                    r#"
                    SELECT r.* FROM roles r
                    INNER JOIN users_roles ur ON r.id = ur.role_id
                    WHERE ur.user_id = $1
                    ORDER BY r.name ASC
                    "#,
                )
                .bind(user_id)
                .fetch_all(p)
                .await
            }
            Executor::Tx(tx) => {
                let mut tx = tx.lock().await;
                sqlx::query_as::<Postgres, RoleRecord>(
                    r#"
                    SELECT r.* FROM roles r
                    INNER JOIN users_roles ur ON r.id = ur.role_id
                    WHERE ur.user_id = $1
                    ORDER BY r.name ASC
                    "#,
                )
                .bind(user_id)
                .fetch_all(&mut **tx)
                .await
            }
        }
        .map_err(|e| DatastoreError::Database(format!("get user roles failed: {e}")))?;
        Ok(records.into_iter().map(|r| r.to_role()).collect())
    }

    pub(super) async fn assign_role_impl(
        &self,
        user_id: &str,
        role_id: &str,
    ) -> DatastoreResult<()> {
        let q = r"INSERT INTO users_roles (user_id, role_id) VALUES ($1, $2)";
        let query = sqlx::query(q).bind(user_id).bind(role_id);

        match &self.executor {
            Executor::Pool(p) => {
                query
                    .execute(p)
                    .await
                    .map_err(|e| DatastoreError::Database(format!("assign role failed: {e}")))?;
            }
            Executor::Tx(tx) => {
                let mut tx = tx.lock().await;
                query
                    .execute(&mut **tx)
                    .await
                    .map_err(|e| DatastoreError::Database(format!("assign role failed: {e}")))?;
            }
        }
        Ok(())
    }

    pub(super) async fn unassign_role_impl(
        &self,
        user_id: &str,
        role_id: &str,
    ) -> DatastoreResult<()> {
        let q = r"DELETE FROM users_roles WHERE user_id = $1 AND role_id = $2";
        let query = sqlx::query(q).bind(user_id).bind(role_id);

        match &self.executor {
            Executor::Pool(p) => {
                query
                    .execute(p)
                    .await
                    .map_err(|e| DatastoreError::Database(format!("unassign role failed: {e}")))?;
            }
            Executor::Tx(tx) => {
                let mut tx = tx.lock().await;
                query
                    .execute(&mut **tx)
                    .await
                    .map_err(|e| DatastoreError::Database(format!("unassign role failed: {e}")))?;
            }
        }
        Ok(())
    }
}
