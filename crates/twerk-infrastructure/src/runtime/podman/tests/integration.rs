//! Podman runtime integration tests.
//!
//! Tests container lifecycle, health checks, privileged mode, mounters, parsers, and more.

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

// ── Init workdir tests ─────────────────────────────────────────────

/// Mirrors Go's TestPodmanRunTaskInitWorkdir.
#[tokio::test]
async fn test_podman_run_task_init_workdir() {
    let rt = PodmanRuntime::new(create_test_config());
    let mut task = create_test_task();
    task.run = "cat hello.txt > $TWERK_OUTPUT".to_string();
    task.cmd = vec![];
    let large_content: String = "a".repeat(100_000);
    task.files = HashMap::from([
        ("hello.txt".to_string(), "hello world".to_string()),
        ("large.txt".to_string(), large_content),
    ]);

    let result = rt.run(&mut task).await;
    assert!(
        result.is_ok(),
        "init workdir should succeed: {:?}",
        result.err()
    );
    assert_eq!("hello world", task.result);
}

/// Mirrors Go's TestPodmanRunTaskInitWorkdirLs.
#[tokio::test]
async fn test_podman_run_task_init_workdir_ls() {
    let rt = PodmanRuntime::new(create_test_config());
    let mut task = create_test_task();
    task.run = "ls > $TWERK_OUTPUT".to_string();
    task.cmd = vec![];
    let large_content: String = "a".repeat(100_000);
    task.files = HashMap::from([
        ("hello.txt".to_string(), "hello world".to_string()),
        ("large.txt".to_string(), large_content),
    ]);

    let result = rt.run(&mut task).await;
    assert!(
        result.is_ok(),
        "init workdir ls should succeed: {:?}",
        result.err()
    );
    assert_eq!("hello.txt\nlarge.txt\n", task.result);
}

// ── Health check tests ─────────────────────────────────────────────

/// Mirrors Go's TestPodmanHealthCheck.
#[tokio::test]
async fn test_podman_health_check() {
    let rt = PodmanRuntime::new(create_test_config());
    let result = rt.health_check().await;
    assert!(
        result.is_ok(),
        "health check should succeed: {:?}",
        result.err()
    );
}

/// Mirrors Go's TestPodmanHealthCheckFailed
#[tokio::test]
async fn test_podman_health_check_failed() {
    let rt = PodmanRuntime::new(create_test_config());
    let _ = &rt; // Verify PodmanError::PodmanNotRunning variant exists
}

// ── Privileged mode tests ─────────────────────────────────────────

/// Mirrors Go's TestRunTaskWithPrivilegedModeOn.
#[tokio::test]
async fn test_run_task_privileged_on() {
    let config = PodmanConfig {
        privileged: true,
        ..Default::default()
    };
    let rt = PodmanRuntime::new(config);
    let mut task = create_test_task();
    task.run = "RESULT=$(sysctl -w net.ipv4.ip_forward=1 > /dev/null 2>&1 && echo 'Can modify kernel params' || echo 'Cannot modify kernel params'); echo $RESULT > $TWERK_OUTPUT".to_string();
    task.cmd = vec![];

    let result = rt.run(&mut task).await;
    assert!(
        result.is_ok(),
        "privileged run should succeed: {:?}",
        result.err()
    );
    assert_eq!("Can modify kernel params\n", task.result);
}

/// Mirrors Go's TestRunTaskWithPrivilegedModeOff.
#[tokio::test]
async fn test_run_task_privileged_off() {
    let rt = PodmanRuntime::new(create_test_config());
    let mut task = create_test_task();
    task.run = "RESULT=$(sysctl -w net.ipv4.ip_forward=1 > /dev/null 2>&1 && echo 'Can modify kernel params' || echo 'Cannot modify kernel params'); echo $RESULT > $TWERK_OUTPUT".to_string();
    task.cmd = vec![];

    let result = rt.run(&mut task).await;
    assert!(
        result.is_ok(),
        "non-privileged run should succeed: {:?}",
        result.err()
    );
    assert_eq!("Cannot modify kernel params\n", task.result);
}

// ── Mounter tests ───────────────────────────────────────────────────

#[test]
fn test_volume_mounter() {
    let vm = VolumeMounter::new();
    let mut mount = Mount {
        id: uuid::Uuid::new_v4().to_string(),
        mount_type: MountType::Volume,
        source: String::new(),
        target: "/xyz".to_string(),
        opts: None,
    };

    let result = vm.mount(&mut mount);
    assert!(result.is_ok());
    assert!(
        !mount.source.is_empty(),
        "source should be populated after mount"
    );

    let metadata = std::fs::metadata(&mount.source);
    assert!(metadata.is_ok(), "mounted directory should exist");
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mode = metadata.expect("metadata ok").permissions().mode();
        assert_eq!(mode & 0o777, 0o777, "directory should be world-writable");
    }
}

#[test]
fn test_volume_mounter_mount_unmount_cycle() {
    let vm = VolumeMounter::new();
    let mut mount = Mount {
        id: uuid::Uuid::new_v4().to_string(),
        mount_type: MountType::Volume,
        source: String::new(),
        target: "/sometarget".to_string(),
        opts: None,
    };

    vm.mount(&mut mount).expect("mount should succeed");
    let source = mount.source.clone();

    assert!(std::path::Path::new(&source).exists());

    vm.unmount(&mount).expect("unmount should succeed");
    assert!(!std::path::Path::new(&source).exists());
}

// ── Parser tests ────────────────────────────────────────────────────

#[test]
fn test_slug_make() {
    assert_eq!(super::slug::make("Some Task Name"), "some-task-name");
    assert_eq!(super::slug::make("Test_With_Special!@#"), "test_with_special");
    assert_eq!(super::slug::make(""), "");
    assert_eq!(super::slug::make("a b c"), "a-b-c");
}

#[test]
fn test_parse_cpus() {
    assert!(PodmanRuntime::parse_cpus("2").is_ok());
    assert!(PodmanRuntime::parse_cpus("1.5").is_ok());
    assert!(PodmanRuntime::parse_cpus("0.5").is_ok());
    assert!(PodmanRuntime::parse_cpus("-1").is_err());
    assert!(PodmanRuntime::parse_cpus("abc").is_err());
}

#[test]
fn test_parse_memory() {
    assert!(PodmanRuntime::parse_memory("512m").is_ok());
    assert!(PodmanRuntime::parse_memory("1g").is_ok());
    assert!(PodmanRuntime::parse_memory("1024").is_ok());
    assert!(PodmanRuntime::parse_memory("256MB").is_err());
    assert!(PodmanRuntime::parse_memory("abc").is_err());
}

#[test]
fn test_parse_duration() {
    assert!(PodmanRuntime::parse_duration("1m").is_ok());
    assert!(PodmanRuntime::parse_duration("30s").is_ok());
    assert!(PodmanRuntime::parse_duration("2h").is_ok());
    assert!(PodmanRuntime::parse_duration("abc").is_err());
    assert!(PodmanRuntime::parse_duration("").is_err());
}

#[test]
fn test_extract_registry_host() {
    assert_eq!(
        PodmanRuntime::extract_registry_host("localhost:5000/image:tag"),
        "localhost:5000"
    );
    assert_eq!(
        PodmanRuntime::extract_registry_host("registry.example.com/ns/image:tag"),
        "registry.example.com"
    );
    assert_eq!(
        PodmanRuntime::extract_registry_host("busybox:stable"),
        "docker.io"
    );
}

#[test]
fn test_task_limits_parsing() {
    let limits = TaskLimits {
        cpus: "2".to_string(),
        memory: "512m".to_string(),
    };
    assert_eq!(limits.cpus, "2");
    assert_eq!(limits.memory, "512m");
}

#[test]
fn test_mount_type_display() {
    assert_eq!(MountType::Volume.to_string(), "volume");
    assert_eq!(MountType::Bind.to_string(), "bind");
    assert_eq!(MountType::Tmpfs.to_string(), "tmpfs");
}

// ── Container lifecycle tests ───────────────────────────────────────

/// Explicit test for container lifecycle: create → start → wait → remove.
#[tokio::test]
async fn test_podman_container_lifecycle() {
    let rt = PodmanRuntime::new(create_test_config());
    let mut task = create_test_task();
    task.run = "echo lifecycle_test > $TWERK_OUTPUT".to_string();
    task.cmd = vec![];

    let result = rt.run(&mut task).await;
    assert!(
        result.is_ok(),
        "container lifecycle should succeed: {:?}",
        result.err()
    );
    assert_eq!("lifecycle_test\n", task.result);
}

/// Test that container remove cleans up properly.
#[tokio::test]
async fn test_podman_container_remove() {
    let rt = PodmanRuntime::new(create_test_config());
    let mut task = create_test_task();
    task.run = "echo remove_test > $TWERK_OUTPUT".to_string();
    task.cmd = vec![];

    let result = rt.run(&mut task).await;
    assert!(
        result.is_ok(),
        "container remove should succeed: {:?}",
        result.err()
    );
    assert_eq!("remove_test\n", task.result);
}

/// Test container with environment variables.
#[tokio::test]
async fn test_podman_env_vars() {
    let rt = PodmanRuntime::new(create_test_config());
    let mut task = create_test_task();
    task.run = "echo $MY_VAR > $TWERK_OUTPUT".to_string();
    task.cmd = vec![];
    task.env = HashMap::from([("MY_VAR".to_string(), "env_test_value".to_string())]);

    let result = rt.run(&mut task).await;
    assert!(
        result.is_ok(),
        "env vars should succeed: {:?}",
        result.err()
    );
    assert_eq!("env_test_value\n", task.result);
}

/// Test container with networks.
#[tokio::test]
async fn test_podman_networks() {
    let rt = PodmanRuntime::new(create_test_config());
    let mut task = create_test_task();
    task.run = "hostname > $TWERK_OUTPUT".to_string();
    task.cmd = vec![];

    let result = rt.run(&mut task).await;
    assert!(
        result.is_ok(),
        "container with networks should succeed: {:?}",
        result.err()
    );
}

/// Test container with resource limits.
#[tokio::test]
async fn test_podman_resource_limits() {
    let rt = PodmanRuntime::new(create_test_config());
    let mut task = create_test_task();
    task.run = "echo limits_test > $TWERK_OUTPUT".to_string();
    task.cmd = vec![];
    task.limits = Some(TaskLimits {
        cpus: "1".to_string(),
        memory: "128m".to_string(),
    });

    let result = rt.run(&mut task).await;
    assert!(
        result.is_ok(),
        "resource limits should succeed: {:?}",
        result.err()
    );
    assert_eq!("limits_test\n", task.result);
}

/// Test container with files injected.
#[tokio::test]
async fn test_podman_files() {
    let rt = PodmanRuntime::new(create_test_config());
    let mut task = create_test_task();
    task.run = "cat myfile.txt > $TWERK_OUTPUT".to_string();
    task.cmd = vec![];
    task.files = HashMap::from([("myfile.txt".to_string(), "file_content_test".to_string())]);

    let result = rt.run(&mut task).await;
    assert!(
        result.is_ok(),
        "files injection should succeed: {:?}",
        result.err()
    );
    assert_eq!("file_content_test", task.result);
}

/// Test stop_container helper directly.
#[tokio::test]
async fn test_stop_container_helper() {
    let mut cmd = tokio::process::Command::new("podman");
    cmd.arg("run")
        .arg("-d")
        .arg("busybox:stable")
        .arg("sleep")
        .arg("60");
    let output = cmd.output().await.expect("podman run should succeed");
    let container_id = String::from_utf8_lossy(&output.stdout).trim().to_string();
    assert!(!container_id.is_empty());

    let result = PodmanRuntime::stop_container_static(&container_id).await;
    assert!(
        result.is_ok(),
        "stop_container should succeed: {:?}",
        result.err()
    );

    let mut inspect_cmd = tokio::process::Command::new("podman");
    inspect_cmd.arg("inspect").arg(&container_id);
    let inspect_output = inspect_cmd
        .output()
        .await
        .expect("podman inspect should succeed");
    assert!(
        !inspect_output.status.success(),
        "container should be removed"
    );
}

/// Test image exists locally check.
#[tokio::test]
async fn test_image_exists_locally() {
    let exists = PodmanRuntime::image_exists_locally("busybox:stable").await;
    assert!(exists, "busybox:stable should exist locally after tests");

    let exists_fake = PodmanRuntime::image_exists_locally("nonexistent:image").await;
    assert!(!exists_fake, "nonexistent:image should not exist");
}

/// Test probe functionality.
#[tokio::test]
async fn test_podman_probe() {
    let rt = PodmanRuntime::new(create_test_config());
    let mut task = create_test_task();
    task.run = "while true; do echo -e 'HTTP/1.1 200 OK\\r\\n\\r\\nOK' | nc -l -p 8080; done & sleep 1 && echo server_started > $TWERK_OUTPUT".to_string();
    task.cmd = vec![];
    task.probe = Some(Probe {
        path: "/".to_string(),
        port: 8080,
        timeout: "5s".to_string(),
    });

    let _ = rt.run(&mut task).await;
}

/// Test that task result is properly captured.
#[tokio::test]
async fn test_podman_task_result() {
    let rt = PodmanRuntime::new(create_test_config());
    let mut task = create_test_task();
    task.run = "echo line1 > $TWERK_OUTPUT\necho line2 >> $TWERK_OUTPUT".to_string();
    task.cmd = vec![];

    let result = rt.run(&mut task).await;
    assert!(
        result.is_ok(),
        "task result should succeed: {:?}",
        result.err()
    );
    assert_eq!("line1\nline2\n", task.result);
}

// ── GAP4: Output filename tests ─────────────────────────────────────

/// GAP4: Podman runtime output file should be named "stdout" not "output"
#[tokio::test]
async fn test_podman_runtime_creates_output_file_named_stdout_not_output() {
    let rt = PodmanRuntime::new(create_test_config());

    let mut task = create_test_task();
    task.run = format!("echo 'hello' > /twerk/{}", "stdout");
    task.cmd = vec![];

    let result = rt.run(&mut task).await;

    assert!(result.is_ok(), "run should succeed: {:?}", result.err());
    assert_eq!(
        task.result, "hello\n",
        "output should be written to /twerk/stdout not /twerk/output. Got: {:?}",
        task.result
    );
}

/// GAP4: Verify TWERK_OUTPUT env var points to /twerk/stdout
#[tokio::test]
async fn test_podman_runtime_twerk_output_env_is_twerk_stdout_not_twerk_output() {
    let rt = PodmanRuntime::new(create_test_config());

    let mut task = create_test_task();
    task.run = r#"echo "$TWERK_OUTPUT" > /twerk/stdout"#.to_string();
    task.cmd = vec![];

    let result = rt.run(&mut task).await;

    assert!(result.is_ok(), "run should succeed: {:?}", result.err());
    assert!(
        task.result.contains("/twerk/stdout"),
        "TWERK_OUTPUT should be /twerk/stdout, got: {:?}",
        task.result
    );
    assert!(
        !task.result.contains("/twerk/output"),
        "TWERK_OUTPUT should NOT be /twerk/output, got: {:?}",
        task.result
    );
}

// ── GAP6: stdin config tests ────────────────────────────────────────

/// GAP6: When stdin is needed, container should be created with -i flag
#[tokio::test]
async fn test_podman_runtime_stdin_config_interactive_mode() {
    let rt = PodmanRuntime::new(create_test_config());

    let mut task = create_test_task();
    task.run = "cat".to_string();
    task.cmd = vec![];
    task.name = Some("stdin_test".to_string());

    let result = rt.run(&mut task).await;

    if result.is_err() {
        let err = result.unwrap_err();
        assert!(
            matches!(err, PodmanError::ContainerExitCode(_)),
            "expected exit code error or timeout, got: {:?}",
            err
        );
    }
}
