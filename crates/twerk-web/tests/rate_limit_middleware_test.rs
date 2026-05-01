#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use axum::http::{header, Request, StatusCode};
use axum::Router;
use http_body_util::BodyExt;
use std::sync::Arc;
use std::time::Duration;
use tower::ServiceExt;
use twerk_app::engine::coordinator::limits::RateLimitConfig;
use twerk_infrastructure::broker::{inmemory::InMemoryBroker, Broker};
use twerk_infrastructure::datastore::{inmemory::InMemoryDatastore, Datastore};
use twerk_web::api::trigger_api::InMemoryTriggerDatastore;
use twerk_web::api::{create_router, AppState, Config};

fn create_test_state(rate_limit_config: Option<RateLimitConfig>) -> AppState {
    let ds = Arc::new(InMemoryDatastore::new()) as Arc<dyn Datastore>;
    let broker = Arc::new(InMemoryBroker::new()) as Arc<dyn Broker>;
    let trigger_ds = Arc::new(InMemoryTriggerDatastore::new());

    let config = Config {
        rate_limit: rate_limit_config,
        ..Default::default()
    };

    AppState {
        broker,
        ds,
        trigger_state: twerk_web::api::trigger_api::TriggerAppState { trigger_ds },
        config,
    }
}

fn create_router_with_rate_limit(rps: u32) -> Router {
    let state = create_test_state(Some(RateLimitConfig::new(rps)));
    create_router(state)
}

async fn send_request(router: &Router, path: &str) -> (StatusCode, Option<u64>) {
    let request = Request::builder()
        .uri(path)
        .body(axum::body::Body::empty())
        .unwrap();

    let response = router.clone().oneshot(request).await.unwrap();
    let status = response.status();
    let retry_after = response
        .headers()
        .get(header::RETRY_AFTER)
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.parse::<u64>().ok());

    (status, retry_after)
}

#[tokio::test]
async fn rate_limit_middleware_returns_429_when_limit_exceeded() {
    let router = create_router_with_rate_limit(5);

    for i in 1..=5 {
        let (status, _) = send_request(&router, "/health").await;
        assert_eq!(
            status,
            StatusCode::OK,
            "request {} should succeed (within limit)",
            i
        );
    }

    let (status, retry_after) = send_request(&router, "/health").await;
    assert_eq!(
        status,
        StatusCode::TOO_MANY_REQUESTS,
        "6th request should be rate limited"
    );
    assert!(
        retry_after.is_some_and(|v| v > 0),
        "429 response should have Retry-After header > 0, got {:?}",
        retry_after
    );
}

#[tokio::test]
async fn rate_limit_middleware_allows_requests_after_wait_period() {
    let router = create_router_with_rate_limit(5);

    for i in 1..=5 {
        let (status, _) = send_request(&router, "/health").await;
        assert_eq!(status, StatusCode::OK, "request {} should succeed", i);
    }

    let (status, _) = send_request(&router, "/health").await;
    assert_eq!(
        status,
        StatusCode::TOO_MANY_REQUESTS,
        "6th request should be rate limited"
    );

    tokio::time::sleep(Duration::from_millis(200)).await;

    let (status, _) = send_request(&router, "/health").await;
    assert_eq!(
        status,
        StatusCode::OK,
        "request after wait period should succeed"
    );
}

#[tokio::test]
async fn rate_limit_middleware_no_resource_leak_after_limit_exceeded() {
    let router = create_router_with_rate_limit(5);

    for _ in 1..=5 {
        let (status, _) = send_request(&router, "/health").await;
    }

    let (status, retry_after) = send_request(&router, "/health").await;
    assert_eq!(status, StatusCode::TOO_MANY_REQUESTS);
    assert!(retry_after.is_some());

    let (status, _) = send_request(&router, "/health").await;
    assert_eq!(
        status,
        StatusCode::TOO_MANY_REQUESTS,
        "subsequent requests should still be rate limited without resource leak"
    );
}