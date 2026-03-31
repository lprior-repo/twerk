//! Concurrent engine isolation tests - multiple engines share a single RabbitMQ
//!
//! These tests verify that engines with different engine_ids can run concurrently
//! on the same RabbitMQ without cross-talk (the original bug).

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::sync::Arc;
use std::time::Duration;

use reqwest::StatusCode;
use serde_json::json;
use testcontainers::runners::AsyncRunner;
use testcontainers::ImageExt;
use testcontainers_modules::postgres::Postgres;
use testcontainers_modules::rabbitmq::RabbitMq;
use tokio::net::TcpListener;
use tokio::task::JoinHandle;
use twerk_app::engine::{Config as EngineConfig, Engine, Mode};
use twerk_infrastructure::datastore::postgres::{PostgresDatastore, SCHEMA};
use twerk_infrastructure::datastore::Options;
use twerk_web::api::{create_router, AppState, Config as ApiConfig};

// ── Shared Infrastructure ────────────────────────────────────────────────────

/// Shared infrastructure for concurrent engine tests.
/// ONE Postgres + ONE RabbitMQ shared by all engines.
struct SharedInfra {
    _postgres: testcontainers::ContainerAsync<Postgres>,
    _rabbitmq: testcontainers::ContainerAsync<RabbitMq>,
    postgres_dsn: String,
    rabbitmq_url: String,
}

impl SharedInfra {
    async fn new() -> anyhow::Result<Self> {
        let postgres = Postgres::default().with_tag("16-alpine").start().await?;
        let postgres_port = postgres.get_host_port_ipv4(5432).await?;
        let postgres_dsn = format!("postgres://postgres:postgres@127.0.0.1:{postgres_port}/postgres");

        // Initialize schema
        let datastore = PostgresDatastore::new(&postgres_dsn, Options::default()).await?;
        datastore.exec_script(SCHEMA).await?;
        datastore.close().await?;

        let rabbitmq = RabbitMq::default().start().await?;
        let rabbitmq_port = rabbitmq.get_host_port_ipv4(5672).await?;
        let rabbitmq_url = format!("amqp://guest:guest@127.0.0.1:{rabbitmq_port}");

        Ok(Self {
            _postgres: postgres,
            _rabbitmq: rabbitmq,
            postgres_dsn,
            rabbitmq_url,
        })
    }
}

// ── Per-Engine Environment ────────────────────────────────────────────────────

/// Per-engine environment: one coordinator + one worker + API server.
/// All share the same SharedInfra but have unique engine_ids.
struct EngineEnv {
    _engine_id: String,
    engine: Engine,
    base_url: String,
    api_handle: JoinHandle<()>,
    client: reqwest::Client,
    worker_handle: JoinHandle<()>,
}

impl EngineEnv {
    async fn new(
        infra: &SharedInfra,
        engine_id: &str,
        port: u16,
    ) -> anyhow::Result<Self> {
        // Set env vars for THIS engine only
        std::env::set_var("TWERK_DATASTORE_TYPE", "postgres");
        std::env::set_var("TWERK_DATASTORE_POSTGRES_DSN", &infra.postgres_dsn);
        std::env::set_var("TWERK_LOCKER_TYPE", "postgres");
        std::env::set_var("TWERK_LOCKER_POSTGRES_DSN", &infra.postgres_dsn);
        std::env::set_var("TWERK_BROKER_TYPE", "rabbitmq");
        std::env::set_var("TWERK_BROKER_RABBITMQ_URL", &infra.rabbitmq_url);
        std::env::set_var("TWERK_BROKER_RABBITMQ_CONSUMER_TIMEOUT", "60000");
        std::env::set_var("TWERK_RUNTIME_TYPE", "shell");
        std::env::set_var("TWERK_RUNTIME_SHELL_CMD", "bash,-c");
        std::env::set_var("TWERK_WORKER_QUEUES", "default:1");
        std::env::set_var("TWERK_ENGINE_ID", engine_id);

        // Create coordinator
        let mut engine = Engine::new(EngineConfig {
            mode: Mode::Coordinator,
            engine_id: Some(engine_id.to_string()),
            ..Default::default()
        });
        engine.start().await?;

        // Create worker
        let mut worker = Engine::new(EngineConfig {
            mode: Mode::Worker,
            engine_id: Some(engine_id.to_string()),
            ..Default::default()
        });
        worker.start().await?;
        let worker_handle = tokio::spawn(async move {
            // Worker runs in background - owned by this task forever
            let _ = worker.run().await;
        });

        // Give subscriptions time to establish
        tokio::time::sleep(Duration::from_millis(500)).await;

        // Start API server
        let listener = TcpListener::bind(format!("127.0.0.1:{port}")).await?;
        let address = listener.local_addr()?.to_string();
        let app = create_router(AppState::new(
            Arc::new(engine.broker_proxy()),
            Arc::new(engine.datastore_proxy()),
            ApiConfig {
                address: address.clone(),
                ..ApiConfig::default()
            },
        ));
        let api_handle = tokio::spawn(async move {
            let _ = axum::serve(listener, app).await;
        });

        // Wait for health
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(2))
            .build()?;
        for _ in 0..50 {
            if client
                .get(format!("http://{address}/health"))
                .send()
                .await
                .is_ok()
            {
                return Ok(Self {
                    _engine_id: engine_id.to_string(),
                    engine,
                    base_url: format!("http://{address}"),
                    api_handle,
                    client,
                    worker_handle,
                });
            }
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
        anyhow::bail!("engine health check timed out")
    }

    async fn teardown(mut self) {
        self.api_handle.abort();
        self.worker_handle.abort();
        let _ = self.engine.terminate().await;
        clear_env();
    }
}

// ── Environment Cleanup ───────────────────────────────────────────────────────

fn clear_env() {
    for key in [
        "TWERK_DATASTORE_TYPE",
        "TWERK_DATASTORE_POSTGRES_DSN",
        "TWERK_LOCKER_TYPE",
        "TWERK_LOCKER_POSTGRES_DSN",
        "TWERK_BROKER_TYPE",
        "TWERK_BROKER_RABBITMQ_URL",
        "TWERK_BROKER_RABBITMQ_CONSUMER_TIMEOUT",
        "TWERK_RUNTIME_TYPE",
        "TWERK_RUNTIME_SHELL_CMD",
        "TWERK_WORKER_QUEUES",
        "TWERK_ENGINE_ID",
    ] {
        std::env::remove_var(key);
    }
}

// ── Test Helpers ─────────────────────────────────────────────────────────────

async fn submit_job(
    client: &reqwest::Client,
    base_url: &str,
    name: &str,
) -> anyhow::Result<(StatusCode, serde_json::Value)> {
    let resp = client
        .post(format!("{base_url}/jobs"))
        .header(reqwest::header::CONTENT_TYPE, "application/json")
        .json(&json!({
            "name": name,
            "tasks": [{
                "name": format!("{}-task", name),
                "run": format!("echo '{}-done'", name)
            }]
        }))
        .send()
        .await?;
    let status = resp.status();
    let body: serde_json::Value = resp.json().await?;
    Ok((status, body))
}

async fn poll_job_until_terminal(
    client: &reqwest::Client,
    base_url: &str,
    job_id: &str,
    timeout: Duration,
) -> anyhow::Result<serde_json::Value> {
    let start = std::time::Instant::now();
    loop {
        let resp = client.get(format!("{base_url}/jobs/{job_id}")).send().await?;
        let job: serde_json::Value = resp.json().await?;
        let state = job["state"].as_str().unwrap_or("UNKNOWN");
        if matches!(state, "COMPLETED" | "FAILED" | "CANCELLED") {
            return Ok(job);
        }
        if start.elapsed() > timeout {
            anyhow::bail!(
                "job {} did not reach terminal within {:?}, last state: {}",
                job_id,
                timeout,
                state
            );
        }
        tokio::time::sleep(Duration::from_millis(200)).await;
    }
}

// ── Tests ────────────────────────────────────────────────────────────────────

/// REGRESSION TEST: Verifies that multiple engines sharing one RabbitMQ
/// do NOT cross-talk (the original bug caused jobs to get stuck in SCHEDULED).
///
/// This test creates 4 engines with different engine_ids, all sharing the SAME
/// Postgres and RabbitMQ infrastructure. If queue isolation is broken, jobs
/// from one engine would be picked up by workers of another engine, causing
/// them to get stuck in SCHEDULED state.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn four_engines_on_shared_rabbitmq_complete_independent_jobs() -> anyhow::Result<()> {
    // Setup: ONE RabbitMQ + ONE Postgres shared by all 4 engines
    let infra = SharedInfra::new().await?;

    // Create 4 engines on different ports, all sharing the SAME infra
    let mut envs = Vec::new();
    for i in 0..4 {
        let port = 9000 + i;
        let env = EngineEnv::new(&infra, &format!("test-{}", i), port).await?;
        envs.push(env);
    }

    // Give all engines time to establish subscriptions
    tokio::time::sleep(Duration::from_secs(1)).await;

    // Submit jobs to each engine
    let mut job_ids = Vec::new();
    for (i, env) in envs.iter().enumerate() {
        let (status, body) =
            submit_job(&env.client, &env.base_url, &format!("job-{}", i)).await?;
        assert_eq!(
            status,
            StatusCode::OK,
            "submit to engine {} failed: {:?}",
            i,
            body
        );
        job_ids.push((i, body["id"].as_str().unwrap().to_string()));
    }

    // Poll all jobs to completion
    let mut results = Vec::new();
    for (i, job_id) in job_ids {
        let env = &envs[i];
        let job = poll_job_until_terminal(
            &env.client,
            &env.base_url,
            &job_id,
            Duration::from_secs(60),
        )
        .await?;
        results.push((i, job["state"].as_str().unwrap().to_string()));
    }

    // Verify all completed
    for (i, state) in results {
        assert_eq!(
            state, "COMPLETED",
            "engine {} job state: {} (should be COMPLETED, indicating no cross-talk)",
            i,
            state
        );
    }

    // Cleanup
    for env in envs {
        env.teardown().await;
    }

    Ok(())
}

/// Verifies that engines with the same engine_id still work correctly
/// (i.e., we didn't break the normal case).
#[tokio::test]
async fn two_engines_same_id_do_not_conflict() -> anyhow::Result<()> {
    let infra = SharedInfra::new().await?;

    // Both engines share the SAME engine_id - this is the normal single-engine case
    let env1 = EngineEnv::new(&infra, "shared-id", 9100).await?;
    let env2 = EngineEnv::new(&infra, "shared-id", 9101).await?;

    tokio::time::sleep(Duration::from_secs(1)).await;

    // Submit job to first engine
    let (status, body) = submit_job(&env1.client, &env1.base_url, "shared-job").await?;
    assert_eq!(status, StatusCode::OK, "submit failed: {:?}", body);
    let job_id = body["id"].as_str().unwrap().to_string();

    // Poll to completion
    let job = poll_job_until_terminal(
        &env1.client,
        &env1.base_url,
        &job_id,
        Duration::from_secs(60),
    )
    .await?;
    assert_eq!(
        job["state"].as_str().unwrap(),
        "COMPLETED",
        "job should complete with shared engine_id"
    );

    env1.teardown().await;
    env2.teardown().await;

    Ok(())
}