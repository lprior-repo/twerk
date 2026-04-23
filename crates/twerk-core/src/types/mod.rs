//! Newtype wrappers for task execution primitives.
//!
//! These types enforce validation at construction time, making illegal states
//! unrepresentable throughout the codebase.

pub mod port;
pub mod progress;
pub mod retry_attempt;
pub mod retry_limit;
pub mod task_count;
pub mod task_position;

pub use port::{Port, PortError};
pub use progress::{Progress, ProgressError};
pub use retry_attempt::{RetryAttempt, RetryAttemptError};
pub use retry_limit::{OptionalRetryLimitError, RetryLimit};
pub use task_count::{TaskCount, TaskCountError};
pub use task_position::{TaskPosition, TaskPositionError};
