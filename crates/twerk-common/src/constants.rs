//! General-purpose string constants used across the codebase.

/// Default value when a task name is unknown/absent.
pub const DEFAULT_TASK_NAME: &str = "unknown";

/// Default value when a mount type is unspecified.
pub const DEFAULT_MOUNT_TYPE: &str = "volume";

/// Default buffer size for mpsc channels used in runtime pull operations.
pub const CHANNEL_BUFFER_SIZE: usize = 100;

// ── Broker constants ─────────────────────────────────────────────

/// Default `RabbitMQ` URL (matches Go default).
pub const DEFAULT_RABBITMQ_URL: &str = "amqp://guest:guest@localhost:5672/";

/// Default consumer timeout in milliseconds for `RabbitMQ` consumers.
pub const DEFAULT_CONSUMER_TIMEOUT_MS: u64 = 30_000;

/// Queue type constant — classic (`RabbitMQ`).
pub const QUEUE_TYPE_CLASSIC: &str = "classic";

// ── Datastore/locker constants ──────────────────────────────────

/// Default `PostgreSQL` connection string.
///
/// Matches Go: `host=localhost user=twerk password=twerk dbname=twerk port=5432 sslmode=disable`
pub const DEFAULT_POSTGRES_DSN: &str =
    "host=localhost user=twerk password=twerk dbname=twerk port=5432 sslmode=disable";
