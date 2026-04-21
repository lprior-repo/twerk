#![deny(clippy::unwrap_used)]
#![warn(clippy::pedantic)]

mod state;

pub use state::{InMemoryTriggerDatastore, TriggerAppState, PERSISTENCE_MSG};

#[cfg(test)]
mod tests;
