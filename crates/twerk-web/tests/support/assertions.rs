use axum::http::StatusCode;
use serde_json::{json, Value};

use super::TestResponse;

pub fn assert_health_up(response: &TestResponse) {
    response
        .assert_status(StatusCode::OK)
        .assert_json_content_type();

    let body = response.json();
    assert_eq!(body, &json!({ "status": "UP", "version": body["version"] }));
    assert!(body["version"]
        .as_str()
        .is_some_and(|version| !version.is_empty()));
}

pub fn assert_json_message(response: &TestResponse, status: StatusCode, message: &str) {
    response.assert_status(status).assert_json_content_type();
    assert_eq!(response.json(), &json!({ "message": message }));
}

pub fn assert_queue_state(response: &TestResponse, name: &str, size: i64) {
    response
        .assert_status(StatusCode::OK)
        .assert_json_content_type();
    assert_eq!(
        response.json(),
        &json!({ "name": name, "size": size, "subscribers": 0, "unacked": 0 })
    );
}

pub fn assert_metrics(
    response: &TestResponse,
    jobs_running: i64,
    tasks_running: i64,
    nodes_online: i64,
    cpu_percent: f64,
) {
    response
        .assert_status(StatusCode::OK)
        .assert_json_content_type();
    assert_eq!(
        response.json(),
        &json!({
            "jobs": { "running": jobs_running },
            "tasks": { "running": tasks_running },
            "nodes": { "online": nodes_online, "cpuPercent": cpu_percent },
        })
    );
}

pub fn assert_job_summary(response: &TestResponse, expected_name: &str, expected_state: &str) {
    response
        .assert_status(StatusCode::OK)
        .assert_json_content_type();

    let body = response.json();
    assert_eq!(body["name"], expected_name);
    assert_eq!(body["state"], expected_state);
    assert!(body["id"].as_str().is_some_and(|id| !id.is_empty()));
}

pub fn assert_node_entry(
    node: &Value,
    expected_id: &str,
    expected_name: &str,
    expected_status: &str,
) {
    assert_eq!(node["id"], expected_id);
    assert_eq!(node["name"], expected_name);
    assert_eq!(node["status"], expected_status);
}

pub fn assert_empty_body(response: &TestResponse, status: StatusCode) {
    response.assert_status(status);
    assert!(
        response.is_empty(),
        "expected empty body, got: {}",
        response.text()
    );
}
