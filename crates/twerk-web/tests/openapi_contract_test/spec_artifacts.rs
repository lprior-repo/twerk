#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use axum::body::Body;
use axum::http::{header, Method, Request, StatusCode};
use serde_json::json;
use twerk_web::api::openapi::{documented_route_specs, mounted_route_specs};

use crate::support::{
    assert_job_summary, mirrored_web_spec_json, request_body_content, request_body_schema_ref,
    tracked_spec_json, tracked_spec_yaml_as_json, TestHarness,
};

#[tokio::test]
async fn tracked_openapi_artifacts_match_live_endpoint() {
    let harness = TestHarness::with_queue("default").await;

    let response = harness
        .call(
            Request::builder()
                .uri("/openapi.json")
                .body(Body::empty())
                .unwrap(),
        )
        .await;

    response
        .assert_status(StatusCode::OK)
        .assert_json_content_type();
    assert_eq!(response.json(), &tracked_spec_json());
    assert_eq!(response.json(), &mirrored_web_spec_json());
    assert_eq!(response.json(), &tracked_spec_yaml_as_json());
}

#[tokio::test]
async fn documented_routes_match_mounted_routes() {
    assert_eq!(documented_route_specs(), mounted_route_specs());
}

#[tokio::test]
async fn tracked_spec_documents_jobs_and_scheduled_jobs_request_media_types_and_schema_refs() {
    let spec = tracked_spec_json();
    assert_eq!(
        spec["components"]["schemas"]["JobId"]["pattern"],
        "^(?:[0-9a-fA-F]{8}-[0-9a-fA-F]{4}-[0-9a-fA-F]{4}-[0-9a-fA-F]{4}-[0-9a-fA-F]{12}|[23456789ABCDEFGHJKLMNPQRSTUVWXYZabcdefghijkmnopqrstuvwxyz]{22})$"
    );
    assert!(spec["components"]["schemas"]["JobId"]["description"]
        .as_str()
        .unwrap()
        .contains("22-character base57 short job ID"));

    let jobs_content = request_body_content(&spec, "/jobs");
    assert_eq!(
        jobs_content.keys().cloned().collect::<Vec<_>>(),
        vec![
            "application/json".to_string(),
            "application/x-yaml".to_string(),
            "application/yaml".to_string(),
            "text/yaml".to_string(),
        ]
    );
    [
        "application/json",
        "application/x-yaml",
        "application/yaml",
        "text/yaml",
    ]
    .into_iter()
    .for_each(|media_type| {
        assert_eq!(
            request_body_schema_ref(&spec, "/jobs", media_type),
            "#/components/schemas/Job"
        );
    });

    let scheduled_content = request_body_content(&spec, "/scheduled-jobs");
    assert_eq!(
        scheduled_content.keys().cloned().collect::<Vec<_>>(),
        vec![
            "application/json".to_string(),
            "application/x-yaml".to_string(),
            "application/yaml".to_string(),
            "text/yaml".to_string(),
        ]
    );
    [
        "application/json",
        "application/x-yaml",
        "application/yaml",
        "text/yaml",
    ]
    .into_iter()
    .for_each(|media_type| {
        assert_eq!(
            request_body_schema_ref(&spec, "/scheduled-jobs", media_type),
            "#/components/schemas/CreateScheduledJobBody"
        );
    });
}

#[tokio::test]
async fn documented_jobs_and_scheduled_jobs_media_types_are_accepted_including_charset_variants() {
    let harness = TestHarness::with_queue("default").await;

    let job_json = json!({
        "name": "charset-job-json",
        "tasks": [{ "name": "task-json", "image": "alpine", "run": "echo hi" }]
    });
    let job_json_response = harness
        .call(
            Request::builder()
                .method(Method::POST)
                .uri("/jobs")
                .header(header::CONTENT_TYPE, "application/json; charset=utf-8")
                .body(Body::from(serde_json::to_vec(&job_json).unwrap()))
                .unwrap(),
        )
        .await;
    assert_job_summary(&job_json_response, "charset-job-json", "PENDING");

    let job_yaml = r#"
name: charset-job-yaml
tasks:
  - name: task-yaml
    image: alpine
    run: echo hi
"#;
    let job_yaml_response = harness
        .call(
            Request::builder()
                .method(Method::POST)
                .uri("/jobs")
                .header(header::CONTENT_TYPE, "application/yaml; charset=utf-8")
                .body(Body::from(job_yaml))
                .unwrap(),
        )
        .await;
    assert_job_summary(&job_yaml_response, "charset-job-yaml", "PENDING");

    let scheduled_json = json!({
        "name": "charset-scheduled-json",
        "cron": "0 * * * * *",
        "tasks": [{ "name": "task-json", "image": "alpine", "run": "echo hi" }]
    });
    let scheduled_json_response = harness
        .call(
            Request::builder()
                .method(Method::POST)
                .uri("/scheduled-jobs")
                .header(header::CONTENT_TYPE, "application/json; charset=utf-8")
                .body(Body::from(serde_json::to_vec(&scheduled_json).unwrap()))
                .unwrap(),
        )
        .await;
    scheduled_json_response
        .assert_status(StatusCode::OK)
        .assert_json_content_type();
    assert_eq!(
        scheduled_json_response.json()["name"],
        "charset-scheduled-json"
    );

    let scheduled_yaml = r#"
name: charset-scheduled-yaml
cron: "0 * * * * *"
tasks:
  - name: task-yaml
    image: alpine
    run: echo hi
"#;
    let scheduled_yaml_response = harness
        .call(
            Request::builder()
                .method(Method::POST)
                .uri("/scheduled-jobs")
                .header(header::CONTENT_TYPE, "text/yaml; charset=utf-8")
                .body(Body::from(scheduled_yaml))
                .unwrap(),
        )
        .await;
    scheduled_yaml_response
        .assert_status(StatusCode::OK)
        .assert_json_content_type();
    assert_eq!(
        scheduled_yaml_response.json()["name"],
        "charset-scheduled-yaml"
    );
}
