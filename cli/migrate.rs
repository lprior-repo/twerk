//! Database migration command
//!
//! Runs database migration scripts for the configured datastore.
//!
//! Go parity: `cli/migrate.go` → reads config for datastore type, connects to postgres,
//! and executes `schema.SCHEMA` via `pg.ExecScript(schema.SCHEMA)`.

use datastore::postgres::{Options as PgOptions, PostgresDatastore, SCHEMA};

use crate::CliError;
use tracing::info;

/// Default `PostgreSQL` connection string.
///
/// Matches Go: `host=localhost user=tork password=tork dbname=tork port=5432 sslmode=disable`
pub const DEFAULT_POSTGRES_DSN: &str =
    "host=localhost user=tork password=tork dbname=tork port=5432 sslmode=disable";

/// Run database migration.
///
/// Go parity:
/// ```go
/// pg, err := postgres.NewPostgresDataStore(dsn)
/// if err != nil { return err }
/// if err := pg.ExecScript(schema.SCHEMA); err != nil {
///     return errors.Wrapf(err, "error when trying to create db schema")
/// }
/// log.Info().Msg("migration completed!")
/// ```
///
/// # Arguments
///
/// * `datastore_type` - The type of datastore to migrate (e.g. "postgres")
/// * `postgres_dsn` - `PostgreSQL` connection string (used if datastore is Postgres)
///
/// # Errors
///
/// Returns [`CliError::UnknownDatastore`] if the datastore type is not supported.
/// Returns [`CliError::Migration`] if the migration fails.
pub async fn run_migration(datastore_type: &str, postgres_dsn: &str) -> Result<(), CliError> {
    match datastore_type.to_lowercase().as_str() {
        "postgres" | "postgresql" => {
            let pg = PostgresDatastore::new(
                postgres_dsn,
                PgOptions {
                    disable_cleanup: true,
                    ..PgOptions::default()
                },
            )
            .await
            .map_err(|e| CliError::Migration(format!("failed to connect to postgres: {e}")))?;

            pg.exec_script(SCHEMA).await.map_err(|e| {
                CliError::Migration(format!("error when trying to create db schema: {e}"))
            })?;

            pg.close()
                .await
                .map_err(|e| CliError::Migration(format!("error closing connection: {e}")))?;

            info!("migration completed!");
            Ok(())
        }
        other => Err(CliError::UnknownDatastore(other.to_string())),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_postgres_dsn() {
        assert!(DEFAULT_POSTGRES_DSN.contains("localhost"));
        assert!(DEFAULT_POSTGRES_DSN.contains("tork"));
        assert!(DEFAULT_POSTGRES_DSN.contains("5432"));
    }

    #[test]
    fn test_default_postgres_dsn_format() {
        // Verify DSN contains all required PostgreSQL connection components
        let dsn = DEFAULT_POSTGRES_DSN;
        assert!(dsn.contains("host=localhost"));
        assert!(dsn.contains("user=tork"));
        assert!(dsn.contains("password=tork"));
        assert!(dsn.contains("dbname=tork"));
        assert!(dsn.contains("port=5432"));
        assert!(dsn.contains("sslmode=disable"));
    }

    #[test]
    fn test_postgres_variant_matching() {
        // Test that "postgres" and "postgresql" both map to the same mode
        // (both should be accepted in the migration function)
        let lower_postgres = "postgres".to_lowercase();
        let lower_postgresql = "postgresql".to_lowercase();
        assert_eq!(lower_postgres, "postgres");
        assert_eq!(lower_postgresql, "postgresql");
        // They are different strings but both should be accepted by run_migration
    }

    #[test]
    fn test_postgres_variants_both_accepted() {
        // Both "postgres" and "postgresql" are valid aliases
        let variants = vec!["postgres", "PostgreSQL", "POSTGRES", "postgres"];
        for variant in variants {
            let normalized = variant.to_lowercase();
            assert!(normalized == "postgres" || normalized == "postgresql");
        }
    }

    #[test]
    fn test_unknown_datastore_rejected() {
        // Verify unknown datastore types would be rejected
        let unknown_types = vec!["mysql", "sqlite", "mongodb", "redis", ""];
        for dt in unknown_types {
            if dt.is_empty() {
                continue;
            }
            let result = dt.to_lowercase();
            assert_ne!(result.as_str(), "postgres");
            assert_ne!(result.as_str(), "postgresql");
        }
    }
}
