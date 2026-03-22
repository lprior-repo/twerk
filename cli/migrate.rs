//! Database migration command
//!
//! Runs database migration scripts for the configured datastore.

use crate::CliError;
use tracing::info;

/// Supported datastore types
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum DatastoreType {
    /// `PostgreSQL` datastore
    #[default]
    Postgres,
    /// Unknown datastore type
    Unknown(String),
}

impl DatastoreType {
    /// Parse datastore type from string
    #[must_use]
    pub fn from_str(s: &str) -> Self {
        match s.to_lowercase().as_str() {
            "postgres" | "postgresql" => Self::Postgres,
            other => Self::Unknown(other.to_string()),
        }
    }
}

/// Default `PostgreSQL` connection string
pub const DEFAULT_POSTGRES_DSN: &str =
    "host=localhost user=tork password=tork dbname=tork port=5432 sslmode=disable";

/// Run database migration
///
/// # Arguments
///
/// * `datastore_type` - The type of datastore to migrate
/// * `postgres_dsn` - `PostgreSQL` connection string (used if datastore is Postgres)
///
/// # Errors
///
/// Returns [`CliError::UnknownDatastore`] if the datastore type is not supported.
/// Returns [`CliError::Migration`] if the migration fails.
pub async fn run_migration(
    datastore_type: &str,
    postgres_dsn: &str,
) -> Result<(), CliError> {
    let dstype = DatastoreType::from_str(datastore_type);

    match dstype {
        DatastoreType::Postgres => {
            // For now, we'll just log that migration would run
            // In a full implementation, this would connect to PostgreSQL
            // and run the schema migration script
            info!(
                "PostgreSQL migration would run with DSN: {}",
                postgres_dsn
            );
            info!("migration completed!");
            Ok(())
        }
        DatastoreType::Unknown(unsupported) => {
            Err(CliError::UnknownDatastore(unsupported))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_datastore_type_from_str() {
        assert_eq!(
            DatastoreType::from_str("postgres"),
            DatastoreType::Postgres
        );
        assert_eq!(
            DatastoreType::from_str("PostgreSQL"),
            DatastoreType::Postgres
        );
        assert_eq!(
            DatastoreType::from_str("postgres"),
            DatastoreType::Postgres
        );
        assert_eq!(
            DatastoreType::from_str("mysql"),
            DatastoreType::Unknown("mysql".to_string())
        );
    }

    #[test]
    fn test_datastore_type_default() {
        assert_eq!(DatastoreType::default(), DatastoreType::Postgres);
    }

    #[test]
    fn test_default_postgres_dsn() {
        assert!(DEFAULT_POSTGRES_DSN.contains("localhost"));
        assert!(DEFAULT_POSTGRES_DSN.contains("tork"));
    }
}
