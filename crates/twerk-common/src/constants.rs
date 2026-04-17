//! General-purpose string constants used across the codebase.

/// Default value when a task name is unknown/absent.
pub const DEFAULT_TASK_NAME: &str = "unknown";

/// Default value when a mount type is unspecified.
pub const DEFAULT_MOUNT_TYPE: &str = "volume";

/// Default buffer size for mpsc channels used in runtime pull operations.
pub const CHANNEL_BUFFER_SIZE: usize = 100;

// ── Broker constants ─────────────────────────────────────────────

/// Default `RabbitMQ` URL.
///
/// SECURITY: This placeholder MUST be overridden via `TWERK_BROKER_RABBITMQ_URL`
/// or `broker.rabbitmq.url` config. Using this default in production exposes
/// RabbitMQ with default guest credentials.
pub const DEFAULT_RABBITMQ_URL: &str = "amqp://GUEST_GUEST_MUST_OVERRIDE@localhost:5672/";

/// Default consumer timeout in milliseconds for `RabbitMQ` consumers.
pub const DEFAULT_CONSUMER_TIMEOUT_MS: u64 = 30_000;

/// Queue type constant — classic (`RabbitMQ`).
pub const QUEUE_TYPE_CLASSIC: &str = "classic";

// ── Datastore/locker constants ──────────────────────────────────

/// Default `PostgreSQL` connection string.
///
/// SECURITY: This placeholder MUST be overridden via `TWERK_DATASTORE_POSTGRES_DSN`
/// or `datastore.postgres.dsn` config. Using this default in production exposes
/// PostgreSQL with weak credentials.
pub const DEFAULT_POSTGRES_DSN: &str =
    "host=localhost user=PLACEHOLDER_MUST_OVERRIDE password=PLACEHOLDER_MUST_OVERRIDE dbname=twerk port=5432 sslmode=disable";
