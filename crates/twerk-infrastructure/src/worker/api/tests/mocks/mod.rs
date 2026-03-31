//! Mock implementations for Worker API tests.

mod broker;
mod datastore;
mod runtime;

use std::sync::Arc;

pub use broker::MockBroker;
pub use datastore::MockDatastore;
pub use runtime::MockRuntime;

/// Create a complete set of mocks for testing
#[must_use]
pub fn create_mocks() -> (Arc<MockBroker>, Arc<MockDatastore>, Arc<MockRuntime>) {
    (
        Arc::new(MockBroker),
        Arc::new(MockDatastore),
        Arc::new(MockRuntime),
    )
}
