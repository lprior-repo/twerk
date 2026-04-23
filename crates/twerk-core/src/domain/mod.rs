//! Domain types module.
//!
//! This module contains newtype wrappers for domain primitives:
//! - [`WebhookUrl`]: Validated webhook URL with RFC 3986 compliance
//! - [`Hostname`]: Validated DNS hostname with RFC 1123 compliance
//! - [`CronExpression`]: Validated cron schedule expression (5-field or 6-field)
//! - [`Endpoint`]: Validated HTTP/HTTPS endpoint URL
//! - [`Dsn`]: `PostgreSQL` connection string (DSN)
//! - [`QueueName`]: Validated queue identifier
//! - [`GoDuration`]: Parsed Go-style duration string
//! - [`Priority`]: Validated job/task priority value (0-9)

#[cfg(test)]
mod testing;

mod cron_expression;
mod domain_parse_error;
mod dsn;
mod endpoint;
mod go_duration;
mod hostname;
mod priority;
mod queue_name;
mod webhook_url;

pub use cron_expression::{CronExpression, CronExpressionError};
pub use domain_parse_error::{DomainParseError, ParseRetryError};
pub use dsn::{Dsn, DsnError};
pub use endpoint::{Endpoint, EndpointError};
pub use go_duration::{GoDuration, GoDurationError};
pub use hostname::{Hostname, HostnameError};
pub use priority::{Priority, PriorityError};
pub use queue_name::{QueueName, QueueNameError};
pub use webhook_url::{WebhookUrl, WebhookUrlError};
