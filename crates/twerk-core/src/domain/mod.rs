//! Domain types module.
//!
//! This module contains newtype wrappers for domain primitives:
//! - [`WebhookUrl`]: Validated webhook URL with RFC 3986 compliance
//! - [`Hostname`]: Validated DNS hostname with RFC 1123 compliance
//! - [`CronExpression`]: Validated cron schedule expression (5-field or 6-field)
//! - [`Endpoint`]: Validated HTTP/HTTPS endpoint URL
//! - [`Dsn`]: PostgreSQL connection string (DSN)

#[cfg(test)]
mod testing;

mod cron_expression;
mod dsn;
mod endpoint;
mod hostname;
mod webhook_url;

pub use cron_expression::{CronExpression, CronExpressionError};
pub use dsn::{Dsn, DsnError};
pub use endpoint::{Endpoint, EndpointError};
pub use hostname::{Hostname, HostnameError};
pub use webhook_url::{WebhookUrl, WebhookUrlError};
