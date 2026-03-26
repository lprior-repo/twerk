//! Podman runtime lifecycle and execution tests.
//!
//! Tests container creation, execution, and cleanup.

use super::*;

fn create_test_task() -> Task {
    Task {
        id: uuid::Uuid::new_v4().to_string(),
        name: Some("Test task".to_string()),
        image: "busybox:stable".to_string(),
        run: String::new(),
        cmd: vec!["ls".to_string()],
        entrypoint: vec![],
        env: HashMap::new(),
        mounts: vec![],
        files: HashMap::new(),
        networks: vec![],
        limits: None,
        registry: None,
        gpus: None,
        probe: None,
        sidecars: vec![],
        pre: vec![],
        post: vec![],
        workdir: None,
        result: String::new(),
        progress: 0.0,
    }
}

fn create_test_config() -> PodmanConfig {
    PodmanConfig {
        broker: None,
        privileged: false,
        host_network: false,
        mounter: None,
        image_verify: false,
        image_ttl: None,
    }
}

// ── Core execution tests ───────────────────────────────────────────

/// Mirrors Go's TestPodmanRunTaskCMD.
#[tokio::test]
async fn test_podman_run_task_cmd() {
    let rt = PodmanRuntime::new(create_test_config());
    let mut task = create_test_task();

    let result = rt.run(&mut task).await;
    assert!(
        result.is_ok(),
        "run with CMD should succeed: {:?}",
        result.err()
    );
}

/// Mirrors Go's TestPodmanRunTaskRun.
#[tokio::test]
async fn test_podman_run_task_run() {
    let rt = PodmanRuntime::new(create_test_config());
    let mut task = create_test_task();
    task.run = "echo hello world > $TWERK_OUTPUT".to_string();
    task.cmd = vec![];

    let result = rt.run(&mut task).await;
    assert!(
        result.is_ok(),
        "run with run script should succeed: {:?}",
        result.err()
    );
    assert_eq!("hello world\n", task.result);
}

/// Mirrors Go's TestPodmanCustomEntrypoint.
#[tokio::test]
async fn test_podman_custom_entrypoint() {
    let rt = PodmanRuntime::new(create_test_config());
    let mut task = create_test_task();
    task.run = "echo hello world > $TWERK_OUTPUT".to_string();
    task.cmd = vec![];
    task.entrypoint = vec!["/bin/sh".to_string(), "-c".to_string()];

    let result = rt.run(&mut task).await;
    assert!(
        result.is_ok(),
        "run with custom entrypoint should succeed: {:?}",
        result.err()
    );
    assert_eq!("hello world\n", task.result);
}

/// Mirrors Go's TestPodmanRunTaskWithError.
#[tokio::test]
async fn test_podman_run_task_with_error() {
    let rt = PodmanRuntime::new(create_test_config());
    let mut task = create_test_task();
    task.run = "not_a_thing".to_string();
    task.cmd = vec![];

    let result = rt.run(&mut task).await;
    assert!(
        result.is_err(),
        "run with bad command should fail: {:?}",
        result
    );
}

/// Mirrors Go's TestPodmanRunTaskWithTimeout.
#[tokio::test]
async fn test_podman_run_task_with_timeout() {
    let rt = PodmanRuntime::new(create_test_config());
    let mut task = create_test_task();
    task.cmd = vec!["sleep".to_string(), "10".to_string()];

    let result = tokio::time::timeout(std::time::Duration::from_secs(2), rt.run(&mut task)).await;

    assert!(result.is_err(), "run should timeout");
}

/// Test that container start waits for container to complete.
#[tokio::test]
async fn test_podman_container_start_and_wait() {
    let rt = PodmanRuntime::new(create_test_config());
    let mut task = create_test_task();
    task.run = "sleep 1 && echo wait_test > $TWERK_OUTPUT".to_string();
    task.cmd = vec![];

    let start = std::time::Instant::now();
    let result = rt.run(&mut task).await;
    let elapsed = start.elapsed();

    assert!(
        result.is_ok(),
        "container start+wait should succeed: {:?}",
        result.err()
    );
    assert_eq!("wait_test\n", task.result);
    assert!(elapsed >= std::time::Duration::from_secs(1));
}

/// Test exit code is properly captured.
#[tokio::test]
async fn test_podman_exit_code() {
    let rt = PodmanRuntime::new(create_test_config());
    let mut task = create_test_task();
    task.run = "exit 42".to_string();
    task.cmd = vec![];

    let result = rt.run(&mut task).await;
    assert!(result.is_err(), "container with exit 42 should fail");
    if let Err(PodmanError::ContainerExitCode(code)) = result {
        assert_eq!(code, "42");
    } else {
        panic!("expected ContainerExitCode error");
    }
}
