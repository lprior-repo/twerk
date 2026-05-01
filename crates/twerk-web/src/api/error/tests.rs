use axum::body::to_bytes;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use serde_json::json;

use super::super::trigger_api::TriggerUpdateError;
use super::ApiError;

const PROBLEM_CONTENT_TYPE: &str = "application/problem+json";

async fn extract_response_body(response: Response) -> (String, String) {
    let bytes = to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap_or_default();
    let body = String::from_utf8(bytes.to_vec()).unwrap_or_default();
    let content_type = response
        .headers()
        .get("content-type")
        .and_then(|v| v.to_str().ok())
        .unwrap_or_default()
        .to_string();
    (body, content_type)
}

fn parse_problem_body(body: &str) -> serde_json::Value {
    serde_json::from_str(body).expect("response body should be valid JSON")
}

#[tokio::test]
async fn into_response_behaves_as_expected() {
    let internal = ApiError::Internal(
        "secret stack trace: connection refused db://admin:pass@host".to_string(),
    )
    .into_response();
    let (internal_body, content_type) = extract_response_body(internal).await;
    assert!(!internal_body.contains("secret"));
    assert!(!internal_body.contains("db://"));
    assert!(!internal_body.contains("stack trace"));
    assert_eq!(content_type, PROBLEM_CONTENT_TYPE);
    let json = parse_problem_body(&internal_body);
    assert_eq!(json["title"], "Internal Server Error");
    assert_eq!(json["status"], 500);
    assert!(json["type"].is_string());

    let bad_request = ApiError::bad_request("invalid input").into_response();
    assert_eq!(bad_request.status(), StatusCode::BAD_REQUEST);
    let (body, content_type) = extract_response_body(bad_request).await;
    assert_eq!(content_type, PROBLEM_CONTENT_TYPE);
    let json = parse_problem_body(&body);
    assert_eq!(json["title"], "Bad Request");
    assert_eq!(json["status"], 400);
    assert_eq!(json["detail"], "invalid input");

    let not_found = ApiError::not_found("resource gone").into_response();
    assert_eq!(not_found.status(), StatusCode::NOT_FOUND);
    let (body, content_type) = extract_response_body(not_found).await;
    assert_eq!(content_type, PROBLEM_CONTENT_TYPE);
    let json = parse_problem_body(&body);
    assert_eq!(json["title"], "Not Found");
    assert_eq!(json["status"], 404);
    assert_eq!(json["detail"], "resource gone");
}

#[test]
fn datastore_errors_map_correctly() {
    let cases = [
        (
            twerk_infrastructure::datastore::Error::UserNotFound,
            ApiError::NotFound("user not found".to_string()),
        ),
        (
            twerk_infrastructure::datastore::Error::JobNotFound,
            ApiError::NotFound("job not found".to_string()),
        ),
        (
            twerk_infrastructure::datastore::Error::TaskNotFound,
            ApiError::NotFound("task not found".to_string()),
        ),
        (
            twerk_infrastructure::datastore::Error::ScheduledJobNotFound,
            ApiError::NotFound("scheduled job not found".to_string()),
        ),
        (
            twerk_infrastructure::datastore::Error::NodeNotFound,
            ApiError::NotFound("node not found".to_string()),
        ),
    ];

    for (input, expected) in cases {
        let api_err: ApiError = input.into();
        assert_eq!(api_err, expected);
    }

    let internal: ApiError =
        twerk_infrastructure::datastore::Error::Database("table not found".to_string()).into();
    assert_eq!(
        internal,
        ApiError::Internal("database error: table not found".to_string())
    );
}

#[test]
fn generic_error_conversions_map_correctly() {
    let anyhow_err: ApiError = anyhow::anyhow!("something broke").into();
    assert_eq!(
        anyhow_err,
        ApiError::Internal("something broke".to_string())
    );

    let trigger_cases = [
        (
            TriggerUpdateError::InvalidIdFormat("bad$id".to_string()),
            ApiError::BadRequest("bad$id".to_string()),
        ),
        (
            TriggerUpdateError::UnsupportedContentType("application/xml".to_string()),
            ApiError::BadRequest("application/xml".to_string()),
        ),
        (
            TriggerUpdateError::MalformedJson("unexpected token".to_string()),
            ApiError::BadRequest("unexpected token".to_string()),
        ),
        (
            TriggerUpdateError::ValidationFailed("name is required".to_string()),
            ApiError::BadRequest("name is required".to_string()),
        ),
        (
            TriggerUpdateError::TriggerNotFound("trg_123".to_string()),
            ApiError::NotFound("trg_123".to_string()),
        ),
        (
            TriggerUpdateError::VersionConflict("optimistic lock".to_string()),
            ApiError::BadRequest("optimistic lock".to_string()),
        ),
        (
            TriggerUpdateError::Persistence("db connection lost".to_string()),
            ApiError::Internal("db connection lost".to_string()),
        ),
        (
            TriggerUpdateError::Serialization("json encode failed".to_string()),
            ApiError::Internal("json encode failed".to_string()),
        ),
    ];

    for (input, expected) in trigger_cases {
        let api_err: ApiError = input.into();
        assert_eq!(api_err, expected);
    }

    let mismatch: ApiError = TriggerUpdateError::IdMismatch {
        path_id: "trg_1".to_string(),
        body_id: "trg_2".to_string(),
    }
    .into();
    assert_eq!(mismatch, ApiError::BadRequest("id mismatch".to_string()));
}

#[tokio::test]
async fn exact_payloads_are_preserved() {
    let not_found = ApiError::NotFound("missing trigger".to_string()).into_response();
    assert_eq!(not_found.status(), StatusCode::NOT_FOUND);
    let (body, _) = extract_response_body(not_found).await;
    let json = parse_problem_body(&body);
    assert_eq!(json["type"], "https://httpstatus.es/404");
    assert_eq!(json["title"], "Not Found");
    assert_eq!(json["status"], 404);
    assert_eq!(json["detail"], "missing trigger");

    let internal = ApiError::Internal("leaky detail".to_string()).into_response();
    assert_eq!(internal.status(), StatusCode::INTERNAL_SERVER_ERROR);
    let (body, _) = extract_response_body(internal).await;
    let json = parse_problem_body(&body);
    assert_eq!(json["type"], "https://httpstatus.es/500");
    assert_eq!(json["title"], "Internal Server Error");
    assert_eq!(json["status"], 500);
    assert_eq!(json["detail"], "leaky detail");
}
