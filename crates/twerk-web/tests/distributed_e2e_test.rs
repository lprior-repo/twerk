#![allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]

use std::sync::Arc;
use std::time::Duration;

use reqwest::StatusCode;
use serde_json::json;
use testcontainers::runners::AsyncRunner;
use testcontainers::ImageExt;
use testcontainers_modules::postgres::Postgres;
use testcontainers_modules::rabbitmq::RabbitMq;
use tokio::net::TcpListener;
use twerk_app::engine::{Config as EngineConfig, Engine, Mode};
use twerk_infrastructure::datastore::postgres::{PostgresDatastore, SCHEMA};
use twerk_infrastructure::datastore::Options;
use twerk_web::api::{create_router, AppState, Config as ApiConfig};

// ── Infrastructure Helpers ──────────────────────────────────────────────────

async fn setup_postgres() -> anyhow::Result<(testcontainers::ContainerAsync<Postgres>, String)> {
    let container = Postgres::default().with_tag("16-alpine").start().await?;
    let host = container.get_host().await?;
    let port = container.get_host_port_ipv4(5432).await?;
    let dsn = format!("postgres://postgres:postgres@{host}:{port}/postgres");
    let datastore = PostgresDatastore::new(&dsn, Options::default()).await?;
    datastore.exec_script(SCHEMA).await?;
    datastore.close().await?;
    Ok((container, dsn))
}

async fn setup_rabbitmq() -> anyhow::Result<(testcontainers::ContainerAsync<RabbitMq>, String)> {
    let container = RabbitMq::default().start().await?;
    let port = container.get_host_port_ipv4(5672).await?;
    Ok((container, format!("amqp://guest:guest@127.0.0.1:{port}")))
}

fn set_distributed_env(postgres_dsn: &str, rabbitmq_url: &str) {
    std::env::set_var("TWERK_DATASTORE_TYPE", "postgres");
    std::env::set_var("TWERK_DATASTORE_POSTGRES_DSN", postgres_dsn);
    std::env::set_var("TWERK_LOCKER_TYPE", "postgres");
    std::env::set_var("TWERK_LOCKER_POSTGRES_DSN", postgres_dsn);
    std::env::set_var("TWERK_BROKER_TYPE", "rabbitmq");
    std::env::set_var("TWERK_BROKER_RABBITMQ_URL", rabbitmq_url);
    // Use 60 second consumer timeout for tests (instead of default 30 minutes).
    // This ensures jobs don't get stuck in SCHEDULED state for too long if
    // a worker gets stuck during test execution.
    std::env::set_var("TWERK_BROKER_RABBITMQ_CONSUMER_TIMEOUT", "60000");
    std::env::set_var("TWERK_RUNTIME_TYPE", "shell");
    std::env::set_var("TWERK_RUNTIME_SHELL_CMD", "bash,-c");
    std::env::set_var("TWERK_WORKER_QUEUES", "default:1");
}

fn clear_distributed_env() {
    [
        "TWERK_DATASTORE_TYPE",
        "TWERK_DATASTORE_POSTGRES_DSN",
        "TWERK_LOCKER_TYPE",
        "TWERK_LOCKER_POSTGRES_DSN",
        "TWERK_BROKER_TYPE",
        "TWERK_BROKER_RABBITMQ_URL",
        "TWERK_RUNTIME_TYPE",
        "TWERK_RUNTIME_SHELL_CMD",
        "TWERK_WORKER_QUEUES",
        "TWERK_ENGINE_ID",
    ]
    .into_iter()
    .for_each(|key| std::env::remove_var(key));
}

fn generate_engine_id() -> String {
    use twerk_core::uuid::new_short_uuid;
    format!("test-{}", &new_short_uuid()[..8])
}

async fn start_api(engine: &Engine) -> anyhow::Result<(String, tokio::task::JoinHandle<()>)> {
    let listener = TcpListener::bind("127.0.0.1:0").await?;
    let address = listener.local_addr()?.to_string();
    let app = create_router(AppState::new(
        Arc::new(engine.broker_proxy()),
        Arc::new(engine.datastore_proxy()),
        ApiConfig {
            address: address.clone(),
            ..ApiConfig::default()
        },
    ));

    let handle = tokio::spawn(async move {
        let _ = axum::serve(listener, app).await;
    });

    Ok((format!("http://{address}"), handle))
}

async fn wait_for_health(base_url: &str) -> anyhow::Result<()> {
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(2))
        .build()?;

    let mut attempts = 0;
    while attempts < 50 {
        let response = client.get(format!("{base_url}/health")).send().await;
        if matches!(response, Ok(ref resp) if resp.status() == StatusCode::OK) {
            return Ok(());
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
        attempts += 1;
    }

    Err(anyhow::anyhow!("api health check did not become ready"))
}

/// Polls a job's state until it reaches a terminal state or times out.
async fn poll_job_until_terminal(
    client: &reqwest::Client,
    base_url: &str,
    job_id: &str,
    timeout: Duration,
) -> anyhow::Result<serde_json::Value> {
    let start = std::time::Instant::now();
    loop {
        let resp = client
            .get(format!("{base_url}/jobs/{job_id}"))
            .send()
            .await?;
        let status = resp.status();
        let body = resp.text().await?;
        if status != StatusCode::OK {
            eprintln!(
                "  poll error: GET /jobs/{} returned {}: {}",
                job_id, status, body
            );
            if start.elapsed() > timeout {
                anyhow::bail!("GET /jobs/{} kept returning {}: {}", job_id, status, body);
            }
            tokio::time::sleep(Duration::from_millis(200)).await;
            continue;
        }
        let job: serde_json::Value = serde_json::from_str(&body)?;
        let state = job["state"].as_str().unwrap_or("UNKNOWN");
        if matches!(state, "COMPLETED" | "FAILED" | "CANCELLED") {
            return Ok(job);
        }
        if start.elapsed() > timeout {
            anyhow::bail!(
                "job {} did not reach terminal state within {:?}, last state: {}",
                job_id,
                timeout,
                state
            );
        }
        tokio::time::sleep(Duration::from_millis(200)).await;
    }
}

/// Submits a job via POST /jobs and returns the response body as serde_json::Value.
async fn submit_job(
    client: &reqwest::Client,
    base_url: &str,
    job_payload: &serde_json::Value,
) -> anyhow::Result<(reqwest::StatusCode, serde_json::Value)> {
    let resp = client
        .post(format!("{base_url}/jobs"))
        .header(reqwest::header::CONTENT_TYPE, "application/json")
        .json(job_payload)
        .send()
        .await?;

    let status = resp.status();
    let body: serde_json::Value = resp.json().await?;
    Ok((status, body))
}

/// Sets up the full distributed environment: Postgres, RabbitMQ, Coordinator, Worker, API.
/// Returns the base_url, API handle, coordinator, worker, and container guards.
struct DistributedEnv {
    base_url: String,
    api_handle: tokio::task::JoinHandle<()>,
    coordinator: Engine,
    worker: Engine,
    client: reqwest::Client,
    _postgres: testcontainers::ContainerAsync<Postgres>,
    _rabbitmq: testcontainers::ContainerAsync<RabbitMq>,
}

impl DistributedEnv {
    async fn new() -> anyhow::Result<Self> {
        let (postgres, postgres_dsn) = setup_postgres().await?;
        let (rabbitmq, rabbitmq_url) = setup_rabbitmq().await?;

        // Generate unique engine_id for this test instance to ensure queue isolation
        let engine_id = generate_engine_id();
        std::env::set_var("TWERK_ENGINE_ID", &engine_id);

        set_distributed_env(&postgres_dsn, &rabbitmq_url);

        let mut coordinator = Engine::new(EngineConfig {
            mode: Mode::Coordinator,
            engine_id: Some(engine_id.clone()),
            ..EngineConfig::default()
        });
        coordinator.start().await?;

        let mut worker = Engine::new(EngineConfig {
            mode: Mode::Worker,
            engine_id: Some(engine_id.clone()),
            ..EngineConfig::default()
        });
        worker.start().await?;

        // Give subscriptions time to establish
        tokio::time::sleep(Duration::from_millis(1000)).await;

        let (base_url, api_handle) = start_api(&coordinator).await?;
        wait_for_health(&base_url).await?;

        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(30))
            .build()?;

        Ok(Self {
            base_url,
            api_handle,
            coordinator,
            worker,
            client,
            _postgres: postgres,
            _rabbitmq: rabbitmq,
        })
    }

    async fn teardown(mut self) {
        self.api_handle.abort();
        let _ = self.worker.terminate().await;
        let _ = self.coordinator.terminate().await;
        clear_distributed_env();
    }
}

// ── Test 1: Basic Single-Task Job ────────────────────────────────────────────

#[tokio::test]
async fn distributed_http_job_completes_with_real_postgres_rabbitmq_and_shell_runtime(
) -> anyhow::Result<()> {
    let env = DistributedEnv::new().await?;

    let (status, body) = submit_job(
        &env.client,
        &env.base_url,
        &json!({
            "name": "distributed-e2e-job",
            "tasks": [
                {
                    "name": "shell-task",
                    "run": "printf 'honest distributed e2e\\n'"
                }
            ]
        }),
    )
    .await?;

    assert_eq!(status, StatusCode::OK, "submit response body: {body}");
    let job_id = body["id"].as_str().expect("job id should be returned");

    let job = poll_job_until_terminal(&env.client, &env.base_url, job_id, Duration::from_secs(30))
        .await?;
    let final_state = job["state"].as_str().unwrap_or("UNKNOWN");
    assert_eq!(final_state, "COMPLETED", "job did not complete: {job}");

    env.teardown().await;
    Ok(())
}

// ── Test 2: Multi-Task Sequential Job ────────────────────────────────────────

#[tokio::test]
async fn distributed_multi_task_sequential_job_completes() -> anyhow::Result<()> {
    let env = DistributedEnv::new().await?;

    let (status, body) = submit_job(
        &env.client,
        &env.base_url,
        &json!({
            "name": "multi-task-sequential",
            "tasks": [
                {
                    "name": "step-1",
                    "run": "echo 'step 1 done'"
                },
                {
                    "name": "step-2",
                    "run": "echo 'step 2 done'"
                },
                {
                    "name": "step-3",
                    "run": "echo 'step 3 done'"
                }
            ]
        }),
    )
    .await?;

    assert_eq!(status, StatusCode::OK, "submit response: {body}");
    let job_id = body["id"].as_str().unwrap();

    let job = poll_job_until_terminal(&env.client, &env.base_url, job_id, Duration::from_secs(45))
        .await?;
    assert_eq!(
        job["state"].as_str().unwrap_or(""),
        "COMPLETED",
        "multi-task job did not complete: {job}"
    );

    // Verify all 3 tasks completed by checking position advanced
    let position = job["position"].as_i64().unwrap_or(0);
    assert_eq!(
        position, 3,
        "expected position 3 after 3 sequential tasks, got {position}"
    );

    env.teardown().await;
    Ok(())
}

// ── Test 3: Job with Variable Capture (var + expression) ────────────────────

#[tokio::test]
async fn distributed_job_captures_task_output_as_var() -> anyhow::Result<()> {
    let env = DistributedEnv::new().await?;

    let (status, body) = submit_job(
        &env.client,
        &env.base_url,
        &json!({
            "name": "var-capture-job",
            "tasks": [
                {
                    "name": "produce-var",
                    "run": "printf 'hello-world'"
                },
                {
                    "name": "consume-var",
                    "run": "printf 'got-it'"
                }
            ]
        }),
    )
    .await?;

    assert_eq!(status, StatusCode::OK, "submit response: {body}");
    let job_id = body["id"].as_str().unwrap();

    let job = poll_job_until_terminal(&env.client, &env.base_url, job_id, Duration::from_secs(30))
        .await?;
    assert_eq!(
        job["state"].as_str().unwrap_or(""),
        "COMPLETED",
        "var-capture job did not complete: {job}"
    );

    env.teardown().await;
    Ok(())
}

// ── Test 4: Failed Job (non-zero exit code) ─────────────────────────────────

#[tokio::test]
async fn distributed_job_fails_when_task_exits_nonzero() -> anyhow::Result<()> {
    let env = DistributedEnv::new().await?;

    let (status, body) = submit_job(
        &env.client,
        &env.base_url,
        &json!({
            "name": "failing-job",
            "tasks": [
                {
                    "name": "fail-task",
                    "run": "exit 42"
                }
            ]
        }),
    )
    .await?;

    assert_eq!(status, StatusCode::OK, "submit response: {body}");
    let job_id = body["id"].as_str().unwrap();

    let job = poll_job_until_terminal(&env.client, &env.base_url, job_id, Duration::from_secs(30))
        .await?;
    let final_state = job["state"].as_str().unwrap_or("UNKNOWN");
    assert_eq!(
        final_state, "FAILED",
        "expected FAILED but got {final_state}: {job}"
    );

    env.teardown().await;
    Ok(())
}

// ── Test 5: Parallel Task Execution ─────────────────────────────────────────

#[tokio::test]
async fn distributed_parallel_task_completes() -> anyhow::Result<()> {
    let env = DistributedEnv::new().await?;

    let (status, body) = submit_job(
        &env.client,
        &env.base_url,
        &json!({
            "name": "parallel-job",
            "tasks": [
                {
                    "name": "parallel-parent",
                    "parallel": {
                        "tasks": [
                            {
                                "name": "parallel-child-1",
                                "run": "echo 'child 1'"
                            },
                            {
                                "name": "parallel-child-2",
                                "run": "echo 'child 2'"
                            },
                            {
                                "name": "parallel-child-3",
                                "run": "echo 'child 3'"
                            }
                        ]
                    }
                }
            ]
        }),
    )
    .await?;

    assert_eq!(status, StatusCode::OK, "submit response: {body}");
    let job_id = body["id"].as_str().unwrap();

    let job = poll_job_until_terminal(&env.client, &env.base_url, job_id, Duration::from_secs(45))
        .await?;
    let final_state = job["state"].as_str().unwrap_or("UNKNOWN");
    assert_eq!(
        final_state, "COMPLETED",
        "parallel job did not complete: {job}"
    );

    env.teardown().await;
    Ok(())
}

// ── Test 6: Each Task Execution ─────────────────────────────────────────────

#[tokio::test]
async fn distributed_each_task_completes() -> anyhow::Result<()> {
    let env = DistributedEnv::new().await?;

    let (status, body) = submit_job(
        &env.client,
        &env.base_url,
        &json!({
            "name": "each-job",
            "tasks": [
                {
                    "name": "each-parent",
                    "each": {
                        "var": "item",
                        "list": "[\"alpha\", \"beta\", \"gamma\"]",
                        "task": {
                            "name": "each-child",
                            "run": "echo 'processing item'"
                        }
                    }
                }
            ]
        }),
    )
    .await?;

    assert_eq!(status, StatusCode::OK, "submit response: {body}");
    let job_id = body["id"].as_str().unwrap();

    let job = poll_job_until_terminal(&env.client, &env.base_url, job_id, Duration::from_secs(45))
        .await?;
    let final_state = job["state"].as_str().unwrap_or("UNKNOWN");
    assert_eq!(final_state, "COMPLETED", "each job did not complete: {job}");

    env.teardown().await;
    Ok(())
}

// ── Test 7: SubJob Task ─────────────────────────────────────────────────────

#[tokio::test]
async fn distributed_subjob_task_completes() -> anyhow::Result<()> {
    let env = DistributedEnv::new().await?;

    let (status, body) = submit_job(
        &env.client,
        &env.base_url,
        &json!({
            "name": "subjob-parent",
            "tasks": [
                {
                    "name": "subjob-wrapper",
                    "subjob": {
                        "name": "inner-subjob",
                        "tasks": [
                            {
                                "name": "subtask-1",
                                "run": "echo 'subtask 1'"
                            },
                            {
                                "name": "subtask-2",
                                "run": "echo 'subtask 2'"
                            }
                        ]
                    }
                }
            ]
        }),
    )
    .await?;

    assert_eq!(status, StatusCode::OK, "submit response: {body}");
    let job_id = body["id"].as_str().unwrap();

    let job = poll_job_until_terminal(&env.client, &env.base_url, job_id, Duration::from_secs(45))
        .await?;
    let final_state = job["state"].as_str().unwrap_or("UNKNOWN");
    assert_eq!(final_state, "COMPLETED", "subjob did not complete: {job}");

    env.teardown().await;
    Ok(())
}

// ── Test 8: Job with Conditional (if=false → SKIP) ──────────────────────────

#[tokio::test]
async fn distributed_conditional_task_skips_when_false() -> anyhow::Result<()> {
    let env = DistributedEnv::new().await?;

    let (status, body) = submit_job(
        &env.client,
        &env.base_url,
        &json!({
            "name": "conditional-skip-job",
            "tasks": [
                {
                    "name": "always-runs",
                    "run": "echo 'step 1'"
                },
                {
                    "name": "skipped-task",
                    "if": "false",
                    "run": "echo 'should not run'"
                },
                {
                    "name": "runs-after-skip",
                    "run": "echo 'step 3'"
                }
            ]
        }),
    )
    .await?;

    assert_eq!(status, StatusCode::OK, "submit response: {body}");
    let job_id = body["id"].as_str().unwrap();

    let job = poll_job_until_terminal(&env.client, &env.base_url, job_id, Duration::from_secs(45))
        .await?;
    let final_state = job["state"].as_str().unwrap_or("UNKNOWN");
    assert_eq!(
        final_state, "COMPLETED",
        "conditional job did not complete: {job}"
    );

    env.teardown().await;
    Ok(())
}

// ── Test 9: Parallel with a Failing Child ───────────────────────────────────

#[tokio::test]
async fn distributed_parallel_task_fails_when_child_fails() -> anyhow::Result<()> {
    let env = DistributedEnv::new().await?;

    let (status, body) = submit_job(
        &env.client,
        &env.base_url,
        &json!({
            "name": "parallel-fail-job",
            "tasks": [
                {
                    "name": "parallel-with-failure",
                    "parallel": {
                        "tasks": [
                            {
                                "name": "ok-child",
                                "run": "echo 'ok'"
                            },
                            {
                                "name": "fail-child",
                                "run": "exit 1"
                            }
                        ]
                    }
                }
            ]
        }),
    )
    .await?;

    assert_eq!(status, StatusCode::OK, "submit response: {body}");
    let job_id = body["id"].as_str().unwrap();

    let job = poll_job_until_terminal(&env.client, &env.base_url, job_id, Duration::from_secs(45))
        .await?;
    let final_state = job["state"].as_str().unwrap_or("UNKNOWN");
    assert_eq!(
        final_state, "FAILED",
        "expected FAILED from parallel child failure, got {final_state}: {job}"
    );

    env.teardown().await;
    Ok(())
}

// ── Test 10: Each with a Failing Item ───────────────────────────────────────

#[tokio::test]
async fn distributed_each_task_fails_when_item_fails() -> anyhow::Result<()> {
    let env = DistributedEnv::new().await?;

    let (status, body) = submit_job(
        &env.client,
        &env.base_url,
        &json!({
            "name": "each-fail-job",
            "tasks": [
                {
                    "name": "each-with-failure",
                    "each": {
                        "var": "item",
                        "list": "[\"ok\", \"fail\"]",
                        "task": {
                            "name": "each-child",
                            "run": "exit 1"
                        }
                    }
                }
            ]
        }),
    )
    .await?;

    assert_eq!(status, StatusCode::OK, "submit response: {body}");
    let job_id = body["id"].as_str().unwrap();

    let job = poll_job_until_terminal(&env.client, &env.base_url, job_id, Duration::from_secs(45))
        .await?;
    let final_state = job["state"].as_str().unwrap_or("UNKNOWN");
    assert_eq!(
        final_state, "FAILED",
        "expected FAILED from each item failure, got {final_state}: {job}"
    );

    env.teardown().await;
    Ok(())
}

// ── Test 11: Submit Job via YAML ────────────────────────────────────────────

#[tokio::test]
async fn distributed_submit_job_via_yaml() -> anyhow::Result<()> {
    let env = DistributedEnv::new().await?;

    let yaml_body = r#"
name: yaml-e2e-job
tasks:
  - name: yaml-task
    run: "printf 'from yaml\\n'"
"#;

    let resp = env
        .client
        .post(format!("{}/jobs", env.base_url))
        .header(reqwest::header::CONTENT_TYPE, "text/yaml")
        .body(yaml_body)
        .send()
        .await?;

    let status = resp.status();
    let body: serde_json::Value = resp.json().await?;
    assert_eq!(status, StatusCode::OK, "yaml submit response: {body}");

    let job_id = body["id"].as_str().unwrap();

    let job = poll_job_until_terminal(&env.client, &env.base_url, job_id, Duration::from_secs(30))
        .await?;
    let final_state = job["state"].as_str().unwrap_or("UNKNOWN");
    assert_eq!(final_state, "COMPLETED", "yaml job did not complete: {job}");

    env.teardown().await;
    Ok(())
}

// ── Test 12: Get Job by ID After Submission ─────────────────────────────────

#[tokio::test]
async fn distributed_get_job_by_id_returns_consistent_data() -> anyhow::Result<()> {
    let env = DistributedEnv::new().await?;

    let (status, body) = submit_job(
        &env.client,
        &env.base_url,
        &json!({
            "name": "get-by-id-job",
            "tasks": [
                {
                    "name": "simple-task",
                    "run": "echo 'hello'"
                }
            ]
        }),
    )
    .await?;

    assert_eq!(status, StatusCode::OK);
    let job_id = body["id"].as_str().unwrap();

    // Fetch immediately — should exist
    let get_resp = env
        .client
        .get(format!("{}/jobs/{}", env.base_url, job_id))
        .send()
        .await?;
    assert_eq!(get_resp.status(), StatusCode::OK);
    let fetched: serde_json::Value = get_resp.json().await?;
    assert_eq!(fetched["id"].as_str().unwrap(), job_id);
    assert_eq!(fetched["name"].as_str().unwrap(), "get-by-id-job");

    // Wait for completion
    let job = poll_job_until_terminal(&env.client, &env.base_url, job_id, Duration::from_secs(30))
        .await?;
    assert_eq!(job["state"].as_str().unwrap_or(""), "COMPLETED");

    env.teardown().await;
    Ok(())
}

// ── Test 13: Job with Tags ──────────────────────────────────────────────────

#[tokio::test]
async fn distributed_job_with_tags_completes() -> anyhow::Result<()> {
    let env = DistributedEnv::new().await?;

    let (status, body) = submit_job(
        &env.client,
        &env.base_url,
        &json!({
            "name": "tagged-job",
            "tags": ["e2e", "distributed", "shell"],
            "tasks": [
                {
                    "name": "tagged-task",
                    "run": "echo 'tagged'"
                }
            ]
        }),
    )
    .await?;

    assert_eq!(status, StatusCode::OK, "submit response: {body}");
    let job_id = body["id"].as_str().unwrap();

    let job = poll_job_until_terminal(&env.client, &env.base_url, job_id, Duration::from_secs(30))
        .await?;
    assert_eq!(job["state"].as_str().unwrap_or(""), "COMPLETED");

    // Verify tags persisted
    let tags = job["tags"].as_array().expect("tags should be array");
    let tag_values: Vec<&str> = tags.iter().filter_map(|t| t.as_str()).collect();
    assert!(
        tag_values.contains(&"e2e"),
        "expected 'e2e' tag, got {tag_values:?}"
    );

    env.teardown().await;
    Ok(())
}

// ── Test 14: Health Endpoint ────────────────────────────────────────────────

#[tokio::test]
async fn distributed_health_endpoint_returns_ok() -> anyhow::Result<()> {
    let env = DistributedEnv::new().await?;

    let resp = env
        .client
        .get(format!("{}/health", env.base_url))
        .send()
        .await?;
    assert_eq!(resp.status(), StatusCode::OK);

    let health: serde_json::Value = resp.json().await?;
    assert_eq!(health["status"].as_str().unwrap_or(""), "UP");

    env.teardown().await;
    Ok(())
}

// ── Test 15: Version in Health Response ──────────────────────────────────────

#[tokio::test]
async fn distributed_health_response_includes_version() -> anyhow::Result<()> {
    let env = DistributedEnv::new().await?;

    let resp = env
        .client
        .get(format!("{}/health", env.base_url))
        .send()
        .await?;
    assert_eq!(resp.status(), StatusCode::OK);

    let health: serde_json::Value = resp.json().await?;
    assert!(
        health["version"].is_string(),
        "version should be a string: {health}"
    );

    env.teardown().await;
    Ok(())
}

// ── Test 16: List Jobs Endpoint ─────────────────────────────────────────────

#[tokio::test]
async fn distributed_list_jobs_returns_submitted_jobs() -> anyhow::Result<()> {
    let env = DistributedEnv::new().await?;

    // Submit a job first
    let (status, body) = submit_job(
        &env.client,
        &env.base_url,
        &json!({
            "name": "list-test-job",
            "tasks": [{ "name": "t", "run": "echo hi" }]
        }),
    )
    .await?;
    assert_eq!(status, StatusCode::OK);
    let job_id = body["id"].as_str().unwrap();

    // Wait for it to complete so the list is stable
    let _ = poll_job_until_terminal(&env.client, &env.base_url, job_id, Duration::from_secs(30))
        .await?;

    // List jobs
    let resp = env
        .client
        .get(format!("{}/jobs", env.base_url))
        .send()
        .await?;
    assert_eq!(resp.status(), StatusCode::OK);

    let list: serde_json::Value = resp.json().await?;
    let items = list["items"].as_array().expect("items should be array");
    assert!(!items.is_empty(), "job list should not be empty");

    // Our job should be in the list
    let found = items.iter().any(|j| j["id"].as_str() == Some(job_id));
    assert!(found, "submitted job {job_id} not found in list");

    env.teardown().await;
    Ok(())
}

// ── Test 17: Nonexistent Job Returns 404 ────────────────────────────────────

#[tokio::test]
async fn distributed_get_nonexistent_job_returns_404() -> anyhow::Result<()> {
    let env = DistributedEnv::new().await?;

    let resp = env
        .client
        .get(format!("{}/jobs/nonexistent12345", env.base_url))
        .send()
        .await?;
    assert_eq!(resp.status(), StatusCode::NOT_FOUND);

    env.teardown().await;
    Ok(())
}

// ── Test 18: Invalid JSON Returns 400 ───────────────────────────────────────

#[tokio::test]
async fn distributed_submit_invalid_json_returns_400() -> anyhow::Result<()> {
    let env = DistributedEnv::new().await?;

    let resp = env
        .client
        .post(format!("{}/jobs", env.base_url))
        .header(reqwest::header::CONTENT_TYPE, "application/json")
        .body("{this is not valid json}")
        .send()
        .await?;
    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);

    env.teardown().await;
    Ok(())
}

// ── Test 19: Job with Missing Tasks Returns 400 ─────────────────────────────

#[tokio::test]
async fn distributed_submit_job_without_tasks_returns_400() -> anyhow::Result<()> {
    let env = DistributedEnv::new().await?;

    let (status, body) = submit_job(
        &env.client,
        &env.base_url,
        &json!({
            "name": "no-tasks-job"
        }),
    )
    .await?;

    assert_eq!(
        status,
        StatusCode::BAD_REQUEST,
        "expected 400 for job without tasks: {body}"
    );

    env.teardown().await;
    Ok(())
}

// ── Test 20: Job with Empty Task List Returns 400 ───────────────────────────

#[tokio::test]
async fn distributed_submit_job_with_empty_tasks_returns_400() -> anyhow::Result<()> {
    let env = DistributedEnv::new().await?;

    let (status, body) = submit_job(
        &env.client,
        &env.base_url,
        &json!({
            "name": "empty-tasks-job",
            "tasks": []
        }),
    )
    .await?;

    assert_eq!(
        status,
        StatusCode::BAD_REQUEST,
        "expected 400 for empty tasks: {body}"
    );

    env.teardown().await;
    Ok(())
}

// ── Test 21: Job with Env Variables ─────────────────────────────────────────

#[tokio::test]
async fn distributed_job_with_env_vars() -> anyhow::Result<()> {
    let env = DistributedEnv::new().await?;

    let (status, body) = submit_job(
        &env.client,
        &env.base_url,
        &json!({
            "name": "env-job",
            "tasks": [
                {
                    "name": "print-env",
                    "run": "echo $MY_TEST_VAR",
                    "env": {
                        "MY_TEST_VAR": "hello-from-e2e"
                    }
                }
            ]
        }),
    )
    .await?;

    assert_eq!(status, StatusCode::OK, "submit response: {body}");
    let job_id = body["id"].as_str().unwrap();

    let job = poll_job_until_terminal(&env.client, &env.base_url, job_id, Duration::from_secs(30))
        .await?;
    assert_eq!(
        job["state"].as_str().unwrap_or(""),
        "COMPLETED",
        "env job did not complete: {job}"
    );

    env.teardown().await;
    Ok(())
}

// ── Test 22: Complex Pipeline (parallel → sequential → subjob) ─────────────

#[tokio::test]
async fn distributed_complex_pipeline_completes() -> anyhow::Result<()> {
    let env = DistributedEnv::new().await?;

    let (status, body) = submit_job(
        &env.client,
        &env.base_url,
        &json!({
            "name": "complex-pipeline",
            "tasks": [
                {
                    "name": "step-1-setup",
                    "run": "echo 'setup done'"
                },
                {
                    "name": "step-2-parallel",
                    "parallel": {
                        "tasks": [
                            { "name": "parallel-a", "run": "echo 'a'" },
                            { "name": "parallel-b", "run": "echo 'b'" }
                        ]
                    }
                },
                {
                    "name": "step-3-verify",
                    "run": "echo 'all done'"
                }
            ]
        }),
    )
    .await?;

    assert_eq!(status, StatusCode::OK, "submit response: {body}");
    let job_id = body["id"].as_str().unwrap();

    let job = poll_job_until_terminal(&env.client, &env.base_url, job_id, Duration::from_secs(60))
        .await?;
    let final_state = job["state"].as_str().unwrap_or("UNKNOWN");
    assert_eq!(
        final_state, "COMPLETED",
        "complex pipeline did not complete: {job}"
    );

    env.teardown().await;
    Ok(())
}

// ── Test 23: Job with Inputs ────────────────────────────────────────────────

#[tokio::test]
async fn distributed_job_with_inputs_completes() -> anyhow::Result<()> {
    let env = DistributedEnv::new().await?;

    let (status, body) = submit_job(
        &env.client,
        &env.base_url,
        &json!({
            "name": "inputs-job",
            "inputs": {
                "greeting": "hello",
                "target": "world"
            },
            "tasks": [
                {
                    "name": "use-inputs",
                    "run": "echo 'got inputs'"
                }
            ]
        }),
    )
    .await?;

    assert_eq!(status, StatusCode::OK, "submit response: {body}");
    let job_id = body["id"].as_str().unwrap();

    let job = poll_job_until_terminal(&env.client, &env.base_url, job_id, Duration::from_secs(30))
        .await?;
    assert_eq!(job["state"].as_str().unwrap_or(""), "COMPLETED");

    // Verify inputs were stored
    let inputs = job["inputs"].as_object().expect("inputs should be object");
    assert_eq!(
        inputs.get("greeting").and_then(|v| v.as_str()),
        Some("hello")
    );

    env.teardown().await;
    Ok(())
}

// ── Test 24: Submit Multiple Jobs Concurrently ──────────────────────────────

#[tokio::test]
async fn distributed_concurrent_job_submissions_all_complete() -> anyhow::Result<()> {
    let env = DistributedEnv::new().await?;

    let mut job_ids: Vec<String> = Vec::new();

    // Submit 5 jobs concurrently
    let mut handles: Vec<tokio::task::JoinHandle<anyhow::Result<String>>> = Vec::new();
    for i in 0..5 {
        let client = env.client.clone();
        let base_url = env.base_url.clone();
        handles.push(tokio::spawn(async move {
            let (status, body) = submit_job(
                &client,
                &base_url,
                &json!({
                    "name": format!("concurrent-job-{i}"),
                    "tasks": [
                        {
                            "name": format!("concurrent-task-{i}"),
                            "run": format!("echo 'concurrent {i}'")
                        }
                    ]
                }),
            )
            .await?;
            assert_eq!(status, StatusCode::OK, "concurrent submit {i}: {body}");
            let id = body["id"].as_str().unwrap().to_string();
            Ok(id)
        }));
    }

    for handle in handles {
        let id = handle.await??;
        job_ids.push(id);
    }

    assert_eq!(job_ids.len(), 5, "expected 5 job IDs");

    // Poll all jobs to completion
    for job_id in &job_ids {
        let job =
            poll_job_until_terminal(&env.client, &env.base_url, job_id, Duration::from_secs(60))
                .await?;
        let state = job["state"].as_str().unwrap_or("UNKNOWN");
        assert_eq!(
            state, "COMPLETED",
            "concurrent job {job_id} did not complete: {job}"
        );
    }

    env.teardown().await;
    Ok(())
}

// ── Test 25: Each Task with Numeric List ────────────────────────────────────

#[tokio::test]
async fn distributed_each_task_with_numeric_list() -> anyhow::Result<()> {
    let env = DistributedEnv::new().await?;

    let (status, body) = submit_job(
        &env.client,
        &env.base_url,
        &json!({
            "name": "each-numeric",
            "tasks": [
                {
                    "name": "each-nums",
                    "each": {
                        "var": "num",
                        "list": "[1, 2, 3, 4, 5]",
                        "task": {
                            "name": "process-num",
                            "run": "echo 'processing number'"
                        }
                    }
                }
            ]
        }),
    )
    .await?;

    assert_eq!(status, StatusCode::OK, "submit response: {body}");
    let job_id = body["id"].as_str().unwrap();

    let job = poll_job_until_terminal(&env.client, &env.base_url, job_id, Duration::from_secs(45))
        .await?;
    assert_eq!(
        job["state"].as_str().unwrap_or(""),
        "COMPLETED",
        "each numeric did not complete: {job}"
    );

    env.teardown().await;
    Ok(())
}

// ── Concurrent Test: Multiple Jobs Across Independent Engines ─────────────────

/// Tests that 4 engines running concurrently (each with own infrastructure)
/// can each complete jobs independently without interference.
/// Note: This tests concurrent execution, NOT queue isolation.
/// The actual queue isolation regression test is in concurrent_isolation_test.rs
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn four_concurrent_engines_complete_independent_jobs() -> anyhow::Result<()> {
    // Run 4 test tasks concurrently
    let mut handles = Vec::new();

    for i in 0..4 {
        handles.push(tokio::spawn(async move {
            let env = DistributedEnv::new().await?;

            let (status, body) = submit_job(
                &env.client,
                &env.base_url,
                &json!({
                    "name": format!("concurrent-isolation-job-{i}"),
                    "tasks": [{
                        "name": format!("task-{i}"),
                        "run": format!("echo 'job {i} completed'")
                    }]
                }),
            )
            .await?;

            assert_eq!(status, StatusCode::OK, "submit response: {body}");
            let job_id = body["id"].as_str().unwrap();

            let job = poll_job_until_terminal(
                &env.client,
                &env.base_url,
                job_id,
                Duration::from_secs(60), // Generous timeout
            )
            .await?;

            let state = job["state"].as_str().unwrap();
            assert_eq!(
                state, "COMPLETED",
                "job {i} should complete, got state: {state}"
            );

            env.teardown().await;
            Ok::<_, anyhow::Error>(())
        }));
    }

    // All 4 must complete - if any get stuck in SCHEDULED, the test fails
    for handle in handles {
        handle.await??;
    }

    Ok(())
}
