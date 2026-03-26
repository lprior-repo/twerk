//! Logging utilities
//!
//! Provides logging setup and a writer that adapts to tracing.

mod error;
mod setup;
mod writer;

pub use error::LoggingError;
pub use setup::setup_logging;
pub use writer::{Level, TracingWriter};
