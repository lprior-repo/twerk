//! Datastore module
//!
//! This module provides a proxy wrapper around the Datastore interface
//! that adds initialization checks, plus factory functions for creating
//! concrete datastore implementations.

mod factory;
mod proxy;

pub use factory::{create_datastore, new_inmemory_datastore, new_inmemory_datastore_arc};
pub use proxy::DatastoreProxy;
