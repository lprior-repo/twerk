#![deny(clippy::unwrap_used)]
#![warn(clippy::pedantic)]

pub mod command;
pub mod parsing;
pub mod query;
pub mod response;

pub use command::{create_trigger_handler, update_trigger_handler};
pub use query::{delete_trigger_handler, get_trigger_handler, list_triggers_handler};

pub const MAX_BODY_BYTES: usize = 16 * 1024;
pub const BODY_TOO_LARGE_MSG: &str = "request body exceeds max size";
