//! Tests for the Worker API.

mod mocks;

use super::server::{health_check_impl, new_api};
use super::types::HealthStatus;
use mocks::create_mocks;

#[tokio::test]
async fn test_worker_api_creation() {
    let (broker, datastore, runtime) = create_mocks();

    let api = new_api(broker, datastore, runtime);

    assert_eq!(api.port(), 0);
}

#[tokio::test]
async fn test_health_check_impl_all_healthy() {
    let (broker, datastore, runtime) = create_mocks();

    let response = health_check_impl(broker, datastore, runtime).await;

    assert!(matches!(response.status, HealthStatus::Up));
}
