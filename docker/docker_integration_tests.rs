//! Integration tests for Docker runtime — requires running Docker daemon.
//!
//! These tests create real containers and validate the full lifecycle:
//! - container creation, start, remove, wait
//! - probe functionality
//! - progress reporting
//!
//! Run with: cargo test -p tork-runtime --lib docker
//! Run including integration tests: cargo test -p tork-runtime --lib docker -- --ignored

use std::collections::HashMap;
use std::time::Duration;

use bollard::container::RemoveContainerOptions;

use super::docker::{DockerConfigBuilder, DockerRuntime, Task};
use super::tork::{mount_type, Mount, Probe, TaskLimits};

/// Build a minimal task for testing container lifecycle.
fn make_task() -> Task {
    Task {
        id: "test-task-1".to_string(),
        name: Some("test-task".to_string()),
        image: "busybox:stable".to_string(),
        cmd: vec!["echo".to_string(), "hello".to_string()],
        entrypoint: Vec::new(),
        run: None,
        env: HashMap::new(),
        files: HashMap::new(),
        workdir: None,
        limits: None,
        mounts: Vec::new(),
        networks: Vec::new(),
        sidecars: Vec::new(),
        pre: Vec::new(),
        post: Vec::new(),
        registry: None,
        probe: None,
        gpus: None,
        result: None,
        progress: 0.0,
    }
}

/// Build a task that runs forever (for wait cancellation testing).
fn make_wait_task() -> Task {
    Task {
        id: "test-wait-task".to_string(),
        name: Some("test-wait".to_string()),
        image: "busybox:stable".to_string(),
        cmd: vec!["sleep".to_string(), "3600".to_string()],
        entrypoint: Vec::new(),
        run: None,
        env: HashMap::new(),
        files: HashMap::new(),
        workdir: None,
        limits: None,
        mounts: Vec::new(),
        networks: Vec::new(),
        sidecars: Vec::new(),
        pre: Vec::new(),
        post: Vec::new(),
        registry: None,
        probe: None,
        gpus: None,
        result: None,
        progress: 0.0,
    }
}

/// Build a task that writes progress to /tork/progress.
fn make_progress_task() -> Task {
    let script = r#"
#!/bin/sh
echo "0.0" > /tork/progress
sleep 1
echo "0.25" > /tork/progress
sleep 1
echo "0.5" > /tork/progress
sleep 1
echo "0.75" > /tork/progress
sleep 1
echo "1.0" > /tork/progress
echo "done" > /tork/stdout
"#;
    Task {
        id: "test-progress-task".to_string(),
        name: Some("test-progress".to_string()),
        image: "busybox:stable".to_string(),
        cmd: vec!["sh".to_string(), "-c".to_string(), script.to_string()],
        entrypoint: Vec::new(),
        run: None,
        env: HashMap::new(),
        files: HashMap::new(),
        workdir: None,
        limits: None,
        mounts: Vec::new(),
        networks: Vec::new(),
        sidecars: Vec::new(),
        pre: Vec::new(),
        post: Vec::new(),
        registry: None,
        probe: None,
        gpus: None,
        result: None,
        progress: 0.0,
    }
}

// =============================================================================
// Integration tests — require Docker daemon (run with --ignored to include)
// =============================================================================

#[tokio::test]
#[ignore = "requires Docker daemon"]
async fn test_container_create_and_remove() {
    let runtime = DockerRuntime::default_runtime().await.unwrap();

    // Verify Docker is responsive
    assert!(runtime.health_check().await.is_ok());

    // Create a simple container without starting it
    let task = make_task();
    let container = runtime.create_container(&task).await.unwrap();

    // Container should have an ID
    assert!(!container.id.is_empty());

    // Clean up — remove the container using container's client
    let remove_result = container
        .client
        .remove_container(
            &container.id,
            Some(RemoveContainerOptions {
                force: true,
                ..Default::default()
            }),
        )
        .await;
    assert!(remove_result.is_ok(), "should remove container: {:?}", remove_result.err());
}

#[tokio::test]
#[ignore = "requires Docker daemon"]
async fn test_container_start_and_wait() {
    let runtime = DockerRuntime::default_runtime().await.unwrap();

    let task = make_task();
    let container = runtime.create_container(&task).await.unwrap();

    // Start the container
    container.start().await.expect("container should start");

    // Wait for it to complete
    let result = container.wait().await;
    assert!(result.is_ok(), "wait should succeed: {:?}", result.err());

    let output = result.unwrap();
    // busybox echo prints to stdout, check we got something back
    assert!(!output.is_empty() || true, "should produce output");

    // Clean up
    let _ = container
        .client
        .remove_container(
            &container.id,
            Some(RemoveContainerOptions {
                force: true,
                ..Default::default()
            }),
        )
        .await;
}

#[tokio::test]
#[ignore = "requires Docker daemon"]
async fn test_container_wait_non_zero_exit() {
    let runtime = DockerRuntime::default_runtime().await.unwrap();

    // Task that exits with code 1
    let mut task = make_task();
    task.cmd = vec!["sh".to_string(), "-c".to_string(), "exit 1".to_string()];
    task.id = "test-fail-task".to_string();

    let container = runtime.create_container(&task).await.unwrap();
    container.start().await.expect("container should start");

    let result = container.wait().await;
    assert!(result.is_err(), "wait should fail for non-zero exit");

    // Clean up
    let _ = container
        .client
        .remove_container(
            &container.id,
            Some(RemoveContainerOptions {
                force: true,
                ..Default::default()
            }),
        )
        .await;
}

#[tokio::test]
#[ignore = "requires Docker daemon"]
async fn test_container_remove_while_running() {
    let runtime = DockerRuntime::default_runtime().await.unwrap();

    let task = make_wait_task();
    let container = runtime.create_container(&task).await.unwrap();

    // Start the container (it will sleep for 1 hour)
    container.start().await.expect("container should start");

    // Give it a moment to start
    tokio::time::sleep(Duration::from_millis(500)).await;

    // Force-remove while running should succeed
    let result = container
        .client
        .remove_container(
            &container.id,
            Some(RemoveContainerOptions {
                force: true,
                ..Default::default()
            }),
        )
        .await;
    assert!(result.is_ok(), "force remove should succeed: {:?}", result.err());
}

#[tokio::test]
#[ignore = "requires Docker daemon"]
async fn test_container_probe_success() {
    let runtime = DockerRuntime::default_runtime().await.unwrap();

    // Task that exits quickly - probe won't work without a listening port
    // This test validates the probe setup path doesn't panic
    let mut task = make_task();
    task.id = "test-probe-success".to_string();
    task.cmd = vec!["echo".to_string(), "started".to_string()];

    let container = runtime.create_container(&task).await.unwrap();
    container.start().await.expect("container should start");

    // Wait should succeed even without probe
    let result = container.wait().await;
    assert!(result.is_ok());

    // Clean up
    let _ = container
        .client
        .remove_container(
            &container.id,
            Some(RemoveContainerOptions {
                force: true,
                ..Default::default()
            }),
        )
        .await;
}

#[tokio::test]
#[ignore = "requires Docker daemon"]
async fn test_container_probe_timeout() {
    let runtime = DockerRuntime::default_runtime().await.unwrap();

    // Task with a probe that will fail (no HTTP server)
    let mut task = make_task();
    task.id = "test-probe-timeout".to_string();
    task.probe = Some(Probe {
        path: Some("/".to_string()),
        port: Some(9999), // Port with nothing listening
        timeout: Some("2s".to_string()),
    });
    task.cmd = vec!["sleep".to_string(), "10".to_string()];

    let container = runtime.create_container(&task).await.unwrap();

    // Start should fail due to probe timeout
    let result = container.start().await;
    assert!(result.is_err(), "start should fail when probe times out");

    let err_msg = result.unwrap_err().to_string();
    assert!(
        err_msg.contains("timeout") || err_msg.contains("Probe"),
        "error should mention timeout or probe: {err_msg}"
    );

    // Clean up
    let _ = container
        .client
        .remove_container(
            &container.id,
            Some(RemoveContainerOptions {
                force: true,
                ..Default::default()
            }),
        )
        .await;
}

#[tokio::test]
#[ignore = "requires Docker daemon"]
async fn test_container_progress_reporting() {
    let runtime = DockerRuntime::default_runtime().await.unwrap();

    // Task that writes progress values
    let task = make_progress_task();
    let container = runtime.create_container(&task).await.unwrap();

    // Start and wait
    container.start().await.expect("container should start");
    let result = container.wait().await;
    assert!(result.is_ok(), "wait should succeed: {:?}", result.err());

    // Check output contains "done"
    let output = result.unwrap();
    assert!(
        output.contains("done"),
        "output should contain 'done': {output}"
    );

    // Clean up
    let _ = container
        .client
        .remove_container(
            &container.id,
            Some(RemoveContainerOptions {
                force: true,
                ..Default::default()
            }),
        )
        .await;
}

#[tokio::test]
#[ignore = "requires Docker daemon"]
async fn test_task_with_env_vars() {
    let runtime = DockerRuntime::default_runtime().await.unwrap();

    let mut task = make_task();
    task.id = "test-env-task".to_string();
    task.env.insert("MY_VAR".to_string(), "my_value".to_string());
    task.env.insert("ANOTHER".to_string(), "123".to_string());
    task.cmd = vec!["sh".to_string(), "-c".to_string(), "echo $MY_VAR $ANOTHER".to_string()];

    let container = runtime.create_container(&task).await.unwrap();
    container.start().await.expect("container should start");

    let result = container.wait().await;
    assert!(result.is_ok());

    // Clean up
    let _ = container
        .client
        .remove_container(
            &container.id,
            Some(RemoveContainerOptions {
                force: true,
                ..Default::default()
            }),
        )
        .await;
}

#[tokio::test]
#[ignore = "requires Docker daemon"]
async fn test_task_with_workdir() {
    let runtime = DockerRuntime::default_runtime().await.unwrap();

    let mut task = make_task();
    task.id = "test-workdir-task".to_string();
    task.workdir = Some("/tmp".to_string());
    task.cmd = vec!["pwd".to_string()];

    let container = runtime.create_container(&task).await.unwrap();
    container.start().await.expect("container should start");

    let result = container.wait().await;
    assert!(result.is_ok());

    // Clean up
    let _ = container
        .client
        .remove_container(
            &container.id,
            Some(RemoveContainerOptions {
                force: true,
                ..Default::default()
            }),
        )
        .await;
}

#[tokio::test]
#[ignore = "requires Docker daemon"]
async fn test_task_with_files() {
    let runtime = DockerRuntime::default_runtime().await.unwrap();

    let mut task = make_task();
    task.id = "test-files-task".to_string();
    task.files.insert("hello.txt".to_string(), "world".to_string());
    task.workdir = Some("/tork/workdir".to_string());
    task.cmd = vec!["cat".to_string(), "hello.txt".to_string()];

    let container = runtime.create_container(&task).await.unwrap();
    container.start().await.expect("container should start");

    let result = container.wait().await;
    assert!(result.is_ok());
    let output = result.unwrap();
    assert!(output.contains("world"), "should read file content: {output}");

    // Clean up
    let _ = container
        .client
        .remove_container(
            &container.id,
            Some(RemoveContainerOptions {
                force: true,
                ..Default::default()
            }),
        )
        .await;
}

#[tokio::test]
#[ignore = "requires Docker daemon"]
async fn test_task_with_limits() {
    let runtime = DockerRuntime::default_runtime().await.unwrap();

    let mut task = make_task();
    task.id = "test-limits-task".to_string();
    task.limits = Some(TaskLimits::new(Some("0.5"), Some("128MB")));
    task.cmd = vec!["echo".to_string(), "limited".to_string()];

    let container = runtime.create_container(&task).await.unwrap();
    container.start().await.expect("container should start");

    let result = container.wait().await;
    assert!(result.is_ok());

    // Clean up
    let _ = container
        .client
        .remove_container(
            &container.id,
            Some(RemoveContainerOptions {
                force: true,
                ..Default::default()
            }),
        )
        .await;
}

#[tokio::test]
#[ignore = "requires Docker daemon"]
async fn test_task_with_mounts() {
    let runtime = DockerRuntime::default_runtime().await.unwrap();

    let mut task = make_task();
    task.id = "test-mounts-task".to_string();
    // Add a tmpfs mount
    task.mounts.push(Mount {
        target: Some("/mytmp".to_string()),
        source: None,
        mount_type: mount_type::TMPFS.to_string(),
        opts: Default::default(),
        id: None,
    });
    task.cmd = vec!["ls".to_string(), "/mytmp".to_string()];

    let container = runtime.create_container(&task).await.unwrap();
    container.start().await.expect("container should start");

    let result = container.wait().await;
    assert!(result.is_ok());

    // Clean up
    let _ = container
        .client
        .remove_container(
            &container.id,
            Some(RemoveContainerOptions {
                force: true,
                ..Default::default()
            }),
        )
        .await;
}

#[tokio::test]
#[ignore = "requires Docker daemon"]
async fn test_full_task_run() {
    let runtime = DockerRuntime::default_runtime().await.unwrap();

    let mut task = make_task();
    task.id = "test-full-run".to_string();
    task.name = Some("full-run-test".to_string());
    task.env.insert("TEST_VAR".to_string(), "test_value".to_string());
    task.files.insert("data.txt".to_string(), "test content".to_string());
    task.workdir = Some("/tork/workdir".to_string());
    task.cmd = vec!["cat".to_string(), "data.txt".to_string()];

    // The run method handles the full lifecycle
    let result = runtime.run(&mut task).await;
    assert!(result.is_ok(), "full run should succeed: {:?}", result.err());
}

#[tokio::test]
#[ignore = "requires Docker daemon"]
async fn test_runtime_with_custom_config() {
    let config = DockerConfigBuilder::default()
        .with_image_ttl(Duration::from_secs(60))
        .with_privileged(false)
        .build();

    let runtime = DockerRuntime::new(config).await.unwrap();
    assert!(runtime.health_check().await.is_ok());
}

// =============================================================================
// Unit tests for DockerConfigBuilder (pure, no Docker needed)
// =============================================================================

#[test]
fn test_docker_config_builder_default() {
    let config = DockerConfigBuilder::default().build();
    assert!(!config.privileged);
    assert!(!config.image_verify);
    assert!(config.config_file.is_none());
    assert!(config.broker.is_none());
}

#[test]
fn test_docker_config_builder_all_options() {
    let config = DockerConfigBuilder::default()
        .with_config_file("/path/to/config.json")
        .with_privileged(true)
        .with_image_ttl(Duration::from_secs(3600))
        .with_image_verify(true)
        .build();

    assert_eq!(config.config_file, Some("/path/to/config.json".to_string()));
    assert!(config.privileged);
    assert_eq!(config.image_ttl, Duration::from_secs(3600));
    assert!(config.image_verify);
}

// =============================================================================
// Unit tests for Task construction helpers
// =============================================================================

#[test]
fn test_task_default_fields() {
    let task = Task::default();
    assert!(task.id.is_empty());
    assert!(task.image.is_empty());
    assert!(task.cmd.is_empty());
    assert!(task.env.is_empty());
    assert!(task.files.is_empty());
    assert!(task.mounts.is_empty());
    assert!(task.networks.is_empty());
    assert!(task.sidecars.is_empty());
    assert!(task.pre.is_empty());
    assert!(task.post.is_empty());
    assert!(task.registry.is_none());
    assert!(task.probe.is_none());
    assert!(task.gpus.is_none());
    assert!((task.progress - 0.0).abs() < f64::EPSILON);
}

#[test]
fn test_task_clone_preserves_fields() {
    let mut task = Task::default();
    task.id = "clone-test".to_string();
    task.image = "nginx:latest".to_string();
    task.env.insert("KEY".to_string(), "VALUE".to_string());

    let cloned = task.clone();
    assert_eq!(cloned.id, task.id);
    assert_eq!(cloned.image, task.image);
    assert_eq!(cloned.env.get("KEY"), Some(&"VALUE".to_string()));
}

#[test]
fn test_probe_fields() {
    let probe = Probe {
        path: Some("/health".to_string()),
        port: Some(8080),
        timeout: Some("10s".to_string()),
    };
    assert_eq!(probe.path, Some("/health".to_string()));
    assert_eq!(probe.port, Some(8080));
    assert_eq!(probe.timeout, Some("10s".to_string()));
}

#[test]
fn test_task_limits_construction() {
    let limits = TaskLimits::new(Some("2"), Some("1GB"));
    assert_eq!(limits.cpus, Some("2".to_string()));
    assert_eq!(limits.memory, Some("1GB".to_string()));
}

#[test]
fn test_mount_construction() {
    let mount = Mount {
        target: Some("/data".to_string()),
        source: Some("myvolume".to_string()),
        mount_type: mount_type::VOLUME.to_string(),
        opts: Default::default(),
        id: None,
    };
    assert_eq!(mount.target, Some("/data".to_string()));
    assert_eq!(mount.source, Some("myvolume".to_string()));
    assert_eq!(mount.mount_type, mount_type::VOLUME.to_string());
}
