#![deny(clippy::unwrap_used)]
#![deny(clippy::expect_used)]
#![deny(clippy::panic)]
#![forbid(unsafe_code)]

pub mod asl;
pub mod domain;
pub mod env;
pub mod eval;
pub mod fns;
pub mod host;
pub mod id;
pub mod job;
pub mod mount;
pub mod node;
pub mod redact;
pub mod repository;
pub mod repository_inmemory;
pub mod role;
pub mod stats;
pub mod task;
pub mod trigger;
pub mod types;
pub mod user;
pub use twerk_common::uuid;
pub mod validation;
pub mod webhook;

pub use domain::{
    CronExpression, CronExpressionError, DomainParseError, GoDuration, GoDurationError, Hostname,
    HostnameError, ParseRetryError, Priority, PriorityError, QueueName, QueueNameError, WebhookUrl,
    WebhookUrlError,
};
pub use id::TriggerId;
pub use repository::{Repository, Result as RepoResult};
pub use repository_inmemory::InMemoryRepository;
pub use trigger::{ParseTriggerStateError, TriggerState};
pub use types::{
    OptionalRetryLimitError, Port, PortError, Progress, ProgressError, RetryAttempt,
    RetryAttemptError, RetryLimit, TaskCount, TaskCountError, TaskPosition, TaskPositionError,
};
