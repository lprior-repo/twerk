//! Domain errors for locker operations

use thiserror::Error;

/// Errors that can occur during lock operations.
#[derive(Debug, Error)]
pub enum LockError {
    /// The lock is already held by another process.
    #[error("lock already held for key '{key}'")]
    AlreadyLocked { key: String },

    /// The lock is not held (for release operations).
    #[error("lock not held for key '{key}'")]
    NotLocked { key: String },

    /// The lock has been invalidated or is no longer valid.
    #[error("lock invalidated for key '{key}'")]
    LockInvalidated { key: String },

    /// A database error occurred.
    #[error("database error: {0}")]
    Database(#[from] sqlx::Error),

    /// A database connection error occurred during initialization.
    #[error("failed to connect to database: {0}")]
    Connection(String),

    /// Transaction error during lock acquisition.
    #[error("transaction error for key '{key}': {source}")]
    Transaction {
        key: String,
        #[source]
        source: Box<dyn std::error::Error + Send + Sync>,
    },
}

/// Errors that can occur during locker initialization.
#[derive(Debug, Error)]
pub enum InitError {
    /// Database connection failed.
    #[error("failed to connect to database: {0}")]
    Connection(String),

    /// Database ping failed.
    #[error("failed to ping database: {0}")]
    Ping(String),

    /// Connection pool configuration error.
    #[error("invalid connection pool configuration: {0}")]
    PoolConfig(String),
}
