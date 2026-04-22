use axum::extract::Path;
use axum::http::StatusCode;
use axum::{routing::get, Json, Router};
use serde_json::{json, Value};
use time::{format_description::well_known::Rfc3339, OffsetDateTime};
use tokio::net::TcpListener;
use tokio::sync::oneshot;
use twerk_cli::handlers::trigger::{
    trigger_create, trigger_delete, trigger_get, trigger_list, trigger_update,
};
use twerk_cli::CliError;
use twerk_web::helpers::start_test_server;

struct HttpTestServer {
    endpoint: String,
    shutdown_tx: oneshot::Sender<()>,
}

impl HttpTestServer {
    async fn shutdown(self) {
        let _ = self.shutdown_tx.send(());
    }
}

async fn spawn_router(router: Router) -> HttpTestServer {
    let listener = TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind test listener");
    let addr = listener.local_addr().expect("listener addr");
    let (shutdown_tx, shutdown_rx) = oneshot::channel::<()>();

    tokio::spawn(async move {
        axum::serve(listener, router)
            .with_graceful_shutdown(async {
                let _ = shutdown_rx.await;
            })
            .await
            .expect("serve test router");
    });

    HttpTestServer {
        endpoint: format!("http://{addr}"),
        shutdown_tx,
    }
}

fn parse_json(body: &str) -> Value {
    serde_json::from_str(body).expect("valid JSON output")
}

fn assert_rfc3339_field(value: &Value, field: &str) {
    let timestamp = value[field].as_str().expect("timestamp field is string");
    OffsetDateTime::parse(timestamp, &Rfc3339).expect("timestamp parses as RFC3339");
}

fn assert_timestamp_fields(value: &Value) {
    assert_rfc3339_field(value, "created_at");
    assert_rfc3339_field(value, "updated_at");
}

fn bad_timestamp_body(id: &str) -> Value {
    json!({
        "id": id,
        "name": "bad-trigger",
        "enabled": true,
        "event": "order.created",
        "condition": null,
        "action": "notify",
        "metadata": {},
        "version": 1,
        "created_at": [2026, 4, 22],
        "updated_at": [2026, 4, 22]
    })
}

async fn get_bad_trigger(Path(id): Path<String>) -> Json<Value> {
    Json(bad_timestamp_body(&id))
}

async fn put_bad_trigger(Path(id): Path<String>) -> (StatusCode, Json<Value>) {
    (StatusCode::OK, Json(bad_timestamp_body(&id)))
}

async fn post_bad_trigger() -> (StatusCode, Json<Value>) {
    (StatusCode::CREATED, Json(bad_timestamp_body("trg_created")))
}

async fn list_bad_triggers() -> Json<Value> {
    Json(json!([bad_timestamp_body("trg_listed")]))
}

async fn delete_no_content() -> StatusCode {
    StatusCode::NO_CONTENT
}

async fn start_bad_timestamp_server() -> HttpTestServer {
    let router = Router::new()
        .route(
            "/api/v1/triggers",
            get(list_bad_triggers).post(post_bad_trigger),
        )
        .route(
            "/api/v1/triggers/{id}",
            get(get_bad_trigger)
                .put(put_bad_trigger)
                .delete(delete_no_content),
        );

    spawn_router(router).await
}

#[tokio::test]
async fn trigger_handlers_accept_rfc3339_timestamps_from_live_server() {
    let server = start_test_server().await.expect("start test server");
    let endpoint = format!("http://{}", server.addr);

    let created_body = trigger_create(
        &endpoint,
        r#"{"name":"qa-trigger","enabled":true,"event":"order.created","action":"notify"}"#,
        true,
    )
    .await
    .expect("create succeeds");
    let created = parse_json(&created_body);
    let id = created["id"].as_str().expect("created id").to_string();
    assert_timestamp_fields(&created);

    let fetched = parse_json(
        &trigger_get(&endpoint, &id, true)
            .await
            .expect("get succeeds"),
    );
    assert_timestamp_fields(&fetched);

    let listed = parse_json(&trigger_list(&endpoint, true).await.expect("list succeeds"));
    let listed_trigger = listed
        .as_array()
        .expect("trigger list is array")
        .iter()
        .find(|trigger| trigger["id"].as_str() == Some(id.as_str()))
        .expect("created trigger present in list");
    assert_timestamp_fields(listed_trigger);

    let updated = parse_json(
        &trigger_update(
            &endpoint,
            &id,
            r#"{"name":"qa-trigger-updated","enabled":false,"event":"order.updated","action":"notify"}"#,
            true,
        )
        .await
        .expect("update succeeds"),
    );
    assert_timestamp_fields(&updated);

    let deleted = trigger_delete(&endpoint, &id, true)
        .await
        .expect("delete succeeds");
    assert!(
        deleted.is_empty(),
        "delete should forward 204 no-content body"
    );

    server.shutdown().await;
}

#[tokio::test]
async fn trigger_list_rejects_non_rfc3339_timestamp_payloads() {
    let server = start_bad_timestamp_server().await;
    let result = trigger_list(&server.endpoint, true).await;
    assert!(matches!(result, Err(CliError::InvalidBody(_))));
    server.shutdown().await;
}

#[tokio::test]
async fn trigger_get_rejects_non_rfc3339_timestamp_payloads() {
    let server = start_bad_timestamp_server().await;
    let result = trigger_get(&server.endpoint, "trg_bad", true).await;
    assert!(matches!(result, Err(CliError::InvalidBody(_))));
    server.shutdown().await;
}

#[tokio::test]
async fn trigger_create_rejects_non_rfc3339_timestamp_payloads() {
    let server = start_bad_timestamp_server().await;
    let result = trigger_create(&server.endpoint, "{}", true).await;
    assert!(matches!(result, Err(CliError::InvalidBody(_))));
    server.shutdown().await;
}

#[tokio::test]
async fn trigger_update_rejects_non_rfc3339_timestamp_payloads() {
    let server = start_bad_timestamp_server().await;
    let result = trigger_update(&server.endpoint, "trg_bad", "{}", true).await;
    assert!(matches!(result, Err(CliError::InvalidBody(_))));
    server.shutdown().await;
}

#[tokio::test]
async fn trigger_delete_json_mode_forwards_no_content_response() {
    let server = start_bad_timestamp_server().await;
    let deleted = trigger_delete(&server.endpoint, "trg_bad", true)
        .await
        .expect("delete succeeds");
    assert!(
        deleted.is_empty(),
        "delete should not fabricate JSON output"
    );
    server.shutdown().await;
}
