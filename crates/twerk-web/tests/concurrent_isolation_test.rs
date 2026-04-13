//! Concurrent engine isolation tests - multiple engines share a single RabbitMQ
//!
//! These tests verify that engines with different engine_ids can run concurrently
//! on the same RabbitMQ without cross-talk (the original bug).
//!
//! Run with: cargo test -p twerk-web --test concurrent_isolation_test --features integration
#![cfg(feature = "integration")]
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::doc_markdown,
    clippy::uninlined_format_args
)]

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
        let postgres_dsn =
            format!("postgres://postgres:postgres@127.0.0.1:{postgres_port}/postgres");

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
    async fn wait_for_health_ready(
        client: &reqwest::Client,
        address: &str,
        attempts_left: usize,
    ) -> anyhow::Result<()> {
        if attempts_left == 0 {
            anyhow::bail!("engine health check timed out");
        }

        let response = client.get(format!("http://{address}/health")).send().await;
        if response.is_ok() {
            Ok(())
        } else {
            tokio::task::yield_now().await;
            Box::pin(Self::wait_for_health_ready(
                client,
                address,
                attempts_left - 1,
            ))
            .await
        }
    }

    async fn new(infra: &SharedInfra, engine_id: &str, port: u16) -> anyhow::Result<Self> {
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
            worker
                .run()
                .await
                .expect("worker run loop should not error");
        });

        // Yield once to allow worker task scheduling
        tokio::task::yield_now().await;

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
            axum::serve(listener, app)
                .await
                .expect("api server should run without serve errors");
        });

        // Wait for health
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(2))
            .build()?;
        Self::wait_for_health_ready(&client, &address, 50).await?;
        Ok(Self {
            _engine_id: engine_id.to_string(),
            engine,
            base_url: format!("http://{address}"),
            api_handle,
            client,
            worker_handle,
        })
    }

    async fn teardown(mut self) {
        self.api_handle.abort();
        self.worker_handle.abort();
        self.engine
            .terminate()
            .await
            .expect("engine terminate should succeed");
        clear_env();
    }
}

impl Drop for SharedInfra {
    fn drop(&mut self) {
        // testcontainers async drop will clean up
    }
}

// ── Environment Cleanup ───────────────────────────────────────────────────────

fn clear_env() {
    [
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
    ]
    .into_iter()
    .for_each(|key| {
        // SAFETY: test-only process environment cleanup for known static keys.
        unsafe { std::env::remove_var(key) }
    });
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
    async fn poll_once(
        client: &reqwest::Client,
        base_url: &str,
        job_id: &str,
        timeout: Duration,
        started_at: std::time::Instant,
    ) -> anyhow::Result<serde_json::Value> {
        if started_at.elapsed() > timeout {
            anyhow::bail!("job {} did not reach terminal within {:?}", job_id, timeout);
        }

        let resp = client
            .get(format!("{base_url}/jobs/{job_id}"))
            .send()
            .await?;
        let job: serde_json::Value = resp.json().await?;
        let state = job["state"].as_str().unwrap_or("UNKNOWN");
        if matches!(state, "COMPLETED" | "FAILED" | "CANCELLED") {
            Ok(job)
        } else {
            tokio::task::yield_now().await;
            Box::pin(poll_once(client, base_url, job_id, timeout, started_at)).await
        }
    }

    poll_once(client, base_url, job_id, timeout, start).await
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
    let env0 = EngineEnv::new(&infra, "test-0", 9000).await?;
    let env1 = EngineEnv::new(&infra, "test-1", 9001).await?;
    let env2 = EngineEnv::new(&infra, "test-2", 9002).await?;
    let env3 = EngineEnv::new(&infra, "test-3", 9003).await?;

    // Yield to give spawned tasks a scheduling point
    tokio::task::yield_now().await;

    let (status0, body0) = submit_job(&env0.client, &env0.base_url, "job-0").await?;
    assert_eq!(
        status0,
        StatusCode::OK,
        "submit to engine 0 failed: {:?}",
        body0
    );
    let (status1, body1) = submit_job(&env1.client, &env1.base_url, "job-1").await?;
    assert_eq!(
        status1,
        StatusCode::OK,
        "submit to engine 1 failed: {:?}",
        body1
    );
    let (status2, body2) = submit_job(&env2.client, &env2.base_url, "job-2").await?;
    assert_eq!(
        status2,
        StatusCode::OK,
        "submit to engine 2 failed: {:?}",
        body2
    );
    let (status3, body3) = submit_job(&env3.client, &env3.base_url, "job-3").await?;
    assert_eq!(
        status3,
        StatusCode::OK,
        "submit to engine 3 failed: {:?}",
        body3
    );

    let job0 = poll_job_until_terminal(
        &env0.client,
        &env0.base_url,
        body0["id"].as_str().expect("engine 0 should return id"),
        Duration::from_secs(60),
    )
    .await?;
    let job1 = poll_job_until_terminal(
        &env1.client,
        &env1.base_url,
        body1["id"].as_str().expect("engine 1 should return id"),
        Duration::from_secs(60),
    )
    .await?;
    let job2 = poll_job_until_terminal(
        &env2.client,
        &env2.base_url,
        body2["id"].as_str().expect("engine 2 should return id"),
        Duration::from_secs(60),
    )
    .await?;
    let job3 = poll_job_until_terminal(
        &env3.client,
        &env3.base_url,
        body3["id"].as_str().expect("engine 3 should return id"),
        Duration::from_secs(60),
    )
    .await?;

    assert_eq!(
        job0["state"].as_str().unwrap_or("UNKNOWN"),
        "COMPLETED",
        "engine 0 job should complete"
    );
    assert_eq!(
        job1["state"].as_str().unwrap_or("UNKNOWN"),
        "COMPLETED",
        "engine 1 job should complete"
    );
    assert_eq!(
        job2["state"].as_str().unwrap_or("UNKNOWN"),
        "COMPLETED",
        "engine 2 job should complete"
    );
    assert_eq!(
        job3["state"].as_str().unwrap_or("UNKNOWN"),
        "COMPLETED",
        "engine 3 job should complete"
    );

    env0.teardown().await;
    env1.teardown().await;
    env2.teardown().await;
    env3.teardown().await;

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

    tokio::task::yield_now().await;

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
