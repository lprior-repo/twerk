use axum::http::StatusCode;

use super::shared::setup_state_with_triggers;
use twerk_web::api::trigger_api::TriggerId;

#[tokio::test]
async fn delete_trigger_returns_204_on_success() {
    let (state, trigger_ds) = setup_state_with_triggers().await;
    let response = crate::support::call(
        &twerk_web::api::create_router(state),
        crate::support::request(axum::http::Method::DELETE, "/api/v1/triggers/trg_test_1"),
    )
    .await;

    assert_eq!(response.status(), StatusCode::NO_CONTENT);
    assert!(trigger_ds
        .get_trigger_by_id(&TriggerId::parse("trg_test_1").unwrap())
        .is_err());
}

#[tokio::test]
async fn delete_trigger_returns_404_when_not_found() {
    let (state, _) = setup_state_with_triggers().await;
    let response = crate::support::call(
        &twerk_web::api::create_router(state),
        crate::support::request(
            axum::http::Method::DELETE,
            "/api/v1/triggers/non_existent_trigger",
        ),
    )
    .await;
    assert_eq!(response.status(), StatusCode::NOT_FOUND);
    assert_eq!(response.json()["error"], "TriggerNotFound");
}

#[tokio::test]
async fn delete_trigger_returns_400_for_invalid_id_format() {
    let (state, _) = setup_state_with_triggers().await;
    let response = crate::support::call(
        &twerk_web::api::create_router(state),
        crate::support::request(axum::http::Method::DELETE, "/api/v1/triggers/bad$id"),
    )
    .await;
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    assert_eq!(response.json()["error"], "InvalidIdFormat");
}

#[tokio::test]
async fn list_triggers_returns_empty_array_when_no_triggers() {
    let response = crate::support::TestHarness::new()
        .await
        .get("/api/v1/triggers")
        .await;
    assert_eq!(response.status(), StatusCode::OK);
    assert!(response.json().as_array().unwrap().is_empty());
}

#[tokio::test]
async fn list_triggers_returns_triggers_when_exist() {
    let (state, _) = setup_state_with_triggers().await;
    let response = crate::support::call(
        &twerk_web::api::create_router(state),
        crate::support::request(axum::http::Method::GET, "/api/v1/triggers"),
    )
    .await;
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(response.json().as_array().unwrap().len(), 2);
}

#[tokio::test]
async fn get_trigger_returns_trigger_when_exists() {
    let (state, _) = setup_state_with_triggers().await;
    let response = crate::support::call(
        &twerk_web::api::create_router(state),
        crate::support::request(axum::http::Method::GET, "/api/v1/triggers/trg_test_1"),
    )
    .await;
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(response.json()["id"], "trg_test_1");
    assert_eq!(response.json()["name"], "test-trigger");
}

#[tokio::test]
async fn get_trigger_returns_404_when_not_found() {
    let response = crate::support::TestHarness::new()
        .await
        .get("/api/v1/triggers/non-existent-trigger")
        .await;
    assert_eq!(response.status(), StatusCode::NOT_FOUND);
    assert_eq!(response.json()["error"], "TriggerNotFound");
}

#[tokio::test]
async fn get_trigger_returns_400_for_invalid_id_format() {
    let response = crate::support::TestHarness::new()
        .await
        .get("/api/v1/triggers/bad$id")
        .await;
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    assert_eq!(response.json()["error"], "InvalidIdFormat");
}
