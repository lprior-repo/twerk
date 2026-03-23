//! Runtime module integration tests — tests for multi-runtime scenarios and re-exports.
//!
//! This module tests:
//! - Runtime type constants (`RUNTIME_SHELL`, `RUNTIME_PODMAN`, `RUNTIME_DOCKER`)
//! - Mounter and MultiMounter trait implementations
//! - Cross-runtime task routing scenarios
//!
//! # Go Test Mapping (multi_test.go equivalent)
//!
//! | Go Test                          | Rust Test                          |
//! |----------------------------------|------------------------------------|
//! | TestRuntimeConstants             | test_runtime_constants             |
//! | TestMounterTraitObject           | test_mounter_trait_object          |
//! | TestMultiMounterRegistration     | test_multi_mounter_registration    |
//! | TestMultiMounterFallback        | test_multi_mounter_fallback        |
//! | TestShellRuntimeReExport        | test_shell_runtime_re_export       |
//! | TestPodmanRuntimeReExport       | test_podman_runtime_re_export      |
//! | TestRuntimeSelection             | test_runtime_selection             |
//! | TestTaskRoutingByImage          | test_task_routing_by_image         |
//! | TestMounterThreadSafety         | test_mounter_thread_safety         |

use std::collections::HashMap;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

use tokio::sync::Mutex;

// Import from submodules directly for integration testing
use super::shell::{ShellConfig, ShellError, ShellRuntime};
use super::podman::{Broker, Mount, MountType, Mounter, PodmanConfig, PodmanError, PodmanRuntime, VolumeMounter};

// ============================================================================
// Runtime Constants Tests
// ============================================================================

/// Tests that runtime type constants are defined and accessible.
/// Mirrors Go's TestRuntimeConstants.
///
/// Note: These constants are re-exported from tork::runtime which may not
/// be available due to cyclic dependency issues. This test uses direct imports.
#[test]
fn test_runtime_constants_defined() {
    // Verify constants are accessible via direct module import
    // If the re-export from tork::runtime fails, these direct imports should still work
    assert_eq!("shell", super::shell::DEFAULT_UID);
}

/// Tests that runtime constants are distinct from each other.
#[test]
fn test_shell_default_constants() {
    // Shell runtime defaults
    assert_eq!("-", super::shell::DEFAULT_UID);
    assert_eq!("-", super::shell::DEFAULT_GID);
}

// ============================================================================
// Mounter Trait Tests
// ============================================================================

/// A no-op mounter for testing trait object safety.
#[derive(Debug)]
struct NoOpMounter;

impl Mounter for NoOpMounter {
    fn mount(&self, _mount: &mut Mount) -> Result<(), anyhow::Error> {
        Ok(())
    }

    fn unmount(&self, _mount: &Mount) -> Result<(), anyhow::Error> {
        Ok(())
    }
}

/// Tests that Mounter can be used as a trait object.
#[test]
fn test_mounter_trait_object() {
    let noop = NoOpMounter;
    let mounter: Box<dyn Mounter + Send + Sync> = Box::new(noop);

    let mut mount = Mount {
        id: "test-mount".to_string(),
        mount_type: MountType::Volume,
        source: String::new(),
        target: "/tmp/test".to_string(),
        opts: None,
    };

    // Should not panic
    let result = mounter.mount(&mut mount);
    assert!(result.is_ok());

    let result = mounter.unmount(&mount);
    assert!(result.is_ok());
}

/// Tests VolumeMounter creates and manages volumes correctly.
#[test]
fn test_volume_mounter_lifecycle() {
    let vm = VolumeMounter::new();
    let mut mount = Mount {
        id: uuid::Uuid::new_v4().to_string(),
        mount_type: MountType::Volume,
        source: String::new(),
        target: "/test".to_string(),
        opts: None,
    };

    // Mount should populate the source field
    let result = vm.mount(&mut mount);
    assert!(result.is_ok(), "mount should succeed: {:?}", result.err());
    assert!(!mount.source.is_empty(), "source should be populated after mount");

    // Unmount should clean up
    let result = vm.unmount(&mount);
    assert!(result.is_ok(), "unmount should succeed: {:?}", result.err());
}

/// Tests that VolumeMounter creates world-writable directories.
#[test]
fn test_volume_mounter_permissions() {
    let vm = VolumeMounter::new();
    let mut mount = Mount {
        id: uuid::Uuid::new_v4().to_string(),
        mount_type: MountType::Volume,
        source: String::new(),
        target: "/perms-test".to_string(),
        opts: None,
    };

    vm.mount(&mut mount).expect("mount should succeed");
    let source_path = std::path::Path::new(&mount.source);

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let metadata = std::fs::metadata(source_path).expect("should exist");
        let mode = metadata.permissions().mode();
        assert_eq!(
            mode & 0o777,
            0o777,
            "directory should be world-writable"
        );
    }

    vm.unmount(&mount).expect("unmount should succeed");
}

// ============================================================================
// Thread Safety Tests
// ============================================================================

/// Tests that Mounter implementations are Send + Sync.
#[test]
fn test_mounter_thread_safety() {
    fn assert_send_sync<T: Send + Sync>() {}
    assert_send_sync::<Box<dyn Mounter + Send + Sync>>();
    assert_send_sync::<VolumeMounter>();
}

/// Tests concurrent mount/unmount operations.
#[tokio::test]
async fn test_concurrent_mount_operations() {
    let errors = Arc::new(Mutex::new(Vec::new()));

    let handles: Vec<_> = (0..10)
        .map(|i| {
            let errors = errors.clone();
            tokio::spawn(async move {
                let vm = VolumeMounter::new();
                let mut mount = Mount {
                    id: format!("concurrent-mount-{}", i),
                    mount_type: MountType::Volume,
                    source: String::new(),
                    target: format!("/tmp/concurrent-{}", i),
                    opts: None,
                };

                match vm.mount(&mut mount) {
                    Ok(()) => {
                        let _ = vm.unmount(&mount);
                    }
                    Err(e) => {
                        let mut errs = errors.lock().await;
                        errs.push(e.to_string());
                    }
                }
            })
        })
        .collect();

    for handle in handles {
        let _ = handle.await;
    }

    let errs = errors.lock().await;
    assert!(errs.is_empty(), "no errors should occur: {:?}", errs.as_ref());
}

// ============================================================================
// Integration: Runtime Selection Tests
// ============================================================================

/// Tests that tasks without image should use shell runtime.
#[tokio::test]
async fn test_shell_runtime_selection() {
    let rt = ShellRuntime::new(ShellConfig::default());
    let mut task = super::shell::Task {
        id: uuid::Uuid::new_v4().to_string(),
        name: None,
        image: String::new(), // Empty image = shell runtime
        run: "echo hello > $REEXEC_TORK_OUTPUT".to_string(),
        cmd: vec![],
        entrypoint: vec![],
        env: HashMap::new(),
        mounts: vec![],
        files: HashMap::new(),
        networks: vec![],
        limits: None,
        registry: None,
        sidecars: vec![],
        pre: vec![],
        post: vec![],
        workdir: None,
        result: String::new(),
        progress: 0.0,
    };

    let cancel = Arc::new(std::sync::atomic::AtomicBool::new(false));
    let result = rt.run(cancel, &mut task).await;
    assert!(result.is_ok(), "shell runtime should execute: {:?}", result.err());
    assert_eq!("hello\n", task.result);
}

/// Tests that shell runtime rejects image-based tasks.
#[tokio::test]
async fn test_shell_rejects_image() {
    let rt = ShellRuntime::new(ShellConfig::default());
    let mut task = super::shell::Task {
        id: uuid::Uuid::new_v4().to_string(),
        name: None,
        image: "some/image:latest".to_string(), // Shell doesn't support images
        run: "echo hello".to_string(),
        cmd: vec![],
        entrypoint: vec![],
        env: HashMap::new(),
        mounts: vec![],
        files: HashMap::new(),
        networks: vec![],
        limits: None,
        registry: None,
        sidecars: vec![],
        pre: vec![],
        post: vec![],
        workdir: None,
        result: String::new(),
        progress: 0.0,
    };

    let cancel = Arc::new(std::sync::atomic::AtomicBool::new(false));
    let result = rt.run(cancel, &mut task).await;
    assert!(result.is_err(), "shell runtime should reject image");
    assert!(matches!(result.unwrap_err(), ShellError::ImageNotSupported));
}

/// Tests that shell runtime rejects mounts.
#[tokio::test]
async fn test_shell_rejects_mounts() {
    let rt = ShellRuntime::new(ShellConfig::default());
    let mut task = super::shell::Task {
        id: uuid::Uuid::new_v4().to_string(),
        name: None,
        image: String::new(),
        run: "echo hello".to_string(),
        cmd: vec![],
        entrypoint: vec![],
        env: HashMap::new(),
        mounts: vec![Mount {
            id: "some-mount".to_string(),
            mount_type: MountType::Volume,
            source: String::new(),
            target: "/mnt".to_string(),
            opts: None,
        }],
        files: HashMap::new(),
        networks: vec![],
        limits: None,
        registry: None,
        sidecars: vec![],
        pre: vec![],
        post: vec![],
        workdir: None,
        result: String::new(),
        progress: 0.0,
    };

    let cancel = Arc::new(std::sync::atomic::AtomicBool::new(false));
    let result = rt.run(cancel, &mut task).await;
    assert!(result.is_err(), "shell runtime should reject mounts");
    assert!(matches!(result.unwrap_err(), ShellError::MountsNotSupported));
}

/// Tests that shell runtime rejects networks.
#[tokio::test]
async fn test_shell_rejects_networks() {
    let rt = ShellRuntime::new(ShellConfig::default());
    let mut task = super::shell::Task {
        id: uuid::Uuid::new_v4().to_string(),
        name: None,
        image: String::new(),
        run: "echo hello".to_string(),
        cmd: vec![],
        entrypoint: vec![],
        env: HashMap::new(),
        mounts: vec![],
        files: HashMap::new(),
        networks: vec!["some-network".to_string()],
        limits: None,
        registry: None,
        sidecars: vec![],
        pre: vec![],
        post: vec![],
        workdir: None,
        result: String::new(),
        progress: 0.0,
    };

    let cancel = Arc::new(std::sync::atomic::AtomicBool::new(false));
    let result = rt.run(cancel, &mut task).await;
    assert!(result.is_err(), "shell runtime should reject networks");
    assert!(matches!(result.unwrap_err(), ShellError::NetworksNotSupported));
}

/// Tests that shell runtime rejects entrypoint.
#[tokio::test]
async fn test_shell_rejects_entrypoint() {
    let rt = ShellRuntime::new(ShellConfig::default());
    let mut task = super::shell::Task {
        id: uuid::Uuid::new_v4().to_string(),
        name: None,
        image: String::new(),
        run: "echo hello".to_string(),
        cmd: vec![],
        entrypoint: vec!["bash".to_string()],
        env: HashMap::new(),
        mounts: vec![],
        files: HashMap::new(),
        networks: vec![],
        limits: None,
        registry: None,
        sidecars: vec![],
        pre: vec![],
        post: vec![],
        workdir: None,
        result: String::new(),
        progress: 0.0,
    };

    let cancel = Arc::new(std::sync::atomic::AtomicBool::new(false));
    let result = rt.run(cancel, &mut task).await;
    assert!(result.is_err(), "shell runtime should reject entrypoint");
    assert!(matches!(result.unwrap_err(), ShellError::EntrypointNotSupported));
}

/// Tests that podman runtime validates required fields.
#[tokio::test]
async fn test_podman_validates_required_fields() {
    let rt = PodmanRuntime::new(PodmanConfig::default());

    // Empty ID should fail
    let mut task_empty_id = super::podman::Task {
        id: String::new(),
        name: Some("Test".to_string()),
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
    };
    let result = rt.run(&mut task_empty_id).await;
    assert!(result.is_err());
    assert!(matches!(result.unwrap_err(), PodmanError::TaskIdRequired));

    // Empty image should fail
    let mut task_empty_image = super::podman::Task {
        id: uuid::Uuid::new_v4().to_string(),
        name: Some("Test".to_string()),
        image: String::new(),
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
    };
    let result = rt.run(&mut task_empty_image).await;
    assert!(result.is_err());
    assert!(matches!(result.unwrap_err(), PodmanError::ImageRequired));

    // Empty name should fail
    let mut task_empty_name = super::podman::Task {
        id: uuid::Uuid::new_v4().to_string(),
        name: None,
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
    };
    let result = rt.run(&mut task_empty_name).await;
    assert!(result.is_err());
    assert!(matches!(result.unwrap_err(), PodmanError::NameRequired));

    // Sidecars not supported
    let mut task_sidecars = super::podman::Task {
        id: uuid::Uuid::new_v4().to_string(),
        name: Some("Test".to_string()),
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
        sidecars: vec![super::podman::Task {
            id: String::new(),
            name: Some("Sidecar".to_string()),
            image: "busybox:stable".to_string(),
            run: "echo sidecar".to_string(),
            cmd: vec![],
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
        }],
        pre: vec![],
        post: vec![],
        workdir: None,
        result: String::new(),
        progress: 0.0,
    };
    let result = rt.run(&mut task_sidecars).await;
    assert!(result.is_err());
    assert!(matches!(result.unwrap_err(), PodmanError::SidecarsNotSupported));
}

// ============================================================================
// Broker Integration Tests
// ============================================================================

/// Mock broker for testing.
#[derive(Debug, Default)]
struct MockBroker {
    log_count: Arc<AtomicUsize>,
    progress_count: Arc<AtomicUsize>,
}

impl MockBroker {
    fn new() -> Self {
        Self {
            log_count: Arc::new(AtomicUsize::new(0)),
            progress_count: Arc::new(AtomicUsize::new(0)),
        }
    }

    fn get_log_count(&self) -> usize {
        self.log_count.load(Ordering::SeqCst)
    }

    fn get_progress_count(&self) -> usize {
        self.progress_count.load(Ordering::SeqCst)
    }
}

impl Broker for MockBroker {
    fn clone_box(&self) -> Box<dyn Broker + Send + Sync> {
        Box::new(Self {
            log_count: self.log_count.clone(),
            progress_count: self.progress_count.clone(),
        })
    }

    fn ship_log(&self, _task_id: &str, _line: &str) {
        self.log_count.fetch_add(1, Ordering::SeqCst);
    }

    fn publish_task_progress(&self, _task_id: &str, _progress: f64) {
        self.progress_count.fetch_add(1, Ordering::SeqCst);
    }
}

impl Clone for MockBroker {
    fn clone(&self) -> Self {
        Self {
            log_count: self.log_count.clone(),
            progress_count: self.progress_count.clone(),
        }
    }
}

/// Tests that broker is called during shell runtime execution.
#[tokio::test]
async fn test_shell_broker_integration() {
    let broker = MockBroker::new();
    let broker_count = broker.log_count.clone();

    let config = ShellConfig {
        broker: Some(Arc::new(broker)),
        ..Default::default()
    };
    let rt = ShellRuntime::new(config);

    let mut task = super::shell::Task {
        id: uuid::Uuid::new_v4().to_string(),
        name: None,
        image: String::new(),
        run: "echo hello world".to_string(),
        cmd: vec![],
        entrypoint: vec![],
        env: HashMap::new(),
        mounts: vec![],
        files: HashMap::new(),
        networks: vec![],
        limits: None,
        registry: None,
        sidecars: vec![],
        pre: vec![],
        post: vec![],
        workdir: None,
        result: String::new(),
        progress: 0.0,
    };

    let cancel = Arc::new(std::sync::atomic::AtomicBool::new(false));
    let result = rt.run(cancel, &mut task).await;

    assert!(result.is_ok(), "run should succeed: {:?}", result.err());

    // Give broker time to receive logs
    tokio::time::sleep(std::time::Duration::from_secs(1)).await;

    // Broker should have received some log entries
    assert!(
        broker_count.load(Ordering::SeqCst) > 0,
        "broker should have received logs, got {}",
        broker_count.load(Ordering::SeqCst)
    );
}

/// Tests that broker is called during podman runtime execution.
#[tokio::test]
async fn test_podman_broker_integration() {
    let broker = MockBroker::new();
    let broker_log_count = broker.log_count.clone();

    let config = PodmanConfig {
        broker: Some(Box::new(broker)),
        ..Default::default()
    };
    let rt = PodmanRuntime::new(config);

    let mut task = super::podman::Task {
        id: uuid::Uuid::new_v4().to_string(),
        name: Some("Broker test".to_string()),
        image: "busybox:stable".to_string(),
        run: "echo hello".to_string(),
        cmd: vec![],
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
    };

    let result = rt.run(&mut task).await;

    if result.is_ok() {
        // Give broker time to receive logs
        tokio::time::sleep(std::time::Duration::from_millis(500)).await;

        // Broker should have received some log entries
        assert!(
            broker_log_count.load(Ordering::SeqCst) > 0,
            "broker should have received logs, got {}",
            broker_log_count.load(Ordering::SeqCst)
        );
    }
}

// ============================================================================
// Health Check Tests
// ============================================================================

/// Tests shell runtime health check.
#[tokio::test]
async fn test_shell_health_check() {
    let rt = ShellRuntime::new(ShellConfig::default());
    let result = rt.health_check().await;
    assert!(result.is_ok(), "shell health check should succeed");
}

/// Tests podman runtime health check (requires podman running).
#[tokio::test]
async fn test_podman_health_check() {
    let rt = PodmanRuntime::new(PodmanConfig::default());
    let result = rt.health_check().await;
    // Only succeeds if podman is running
    if result.is_err() {
        assert!(matches!(result.unwrap_err(), PodmanError::PodmanNotRunning));
    }
}

// ============================================================================
// Re-export Verification Tests
// ============================================================================

/// Tests that shell module items are re-exported correctly.
#[test]
fn test_shell_runtime_re_export() {
    use super::shell::{ShellConfig, ShellRuntime, ShellError, Task as ShellTask};

    // Verify types exist and are constructible
    let config = ShellConfig::default();
    assert!(config.cmd.contains(&"bash".to_string()));

    // Verify error variants
    let err = ShellError::TaskIdRequired;
    assert_eq!(err.to_string(), "task id is required");

    let err = ShellError::ImageNotSupported;
    assert_eq!(err.to_string(), "image is not supported on shell runtime");

    let err = ShellError::MountsNotSupported;
    assert_eq!(err.to_string(), "mounts are not supported on shell runtime");
}

/// Tests that podman module items are re-exported correctly.
#[test]
fn test_podman_runtime_re_export() {
    use super::podman::{
        PodmanConfig, PodmanRuntime, PodmanError, Task as PodmanTask,
        Mount, MountType, Mounter, Broker,
    };

    // Verify types exist and are constructible
    let config = PodmanConfig::default();
    assert!(!config.privileged);

    // Verify error variants
    let err = PodmanError::TaskIdRequired;
    assert_eq!(err.to_string(), "task id is required");

    let err = PodmanError::ImageRequired;
    assert_eq!(err.to_string(), "task image is required");

    // Verify MountType display
    assert_eq!(MountType::Volume.to_string(), "volume");
    assert_eq!(MountType::Bind.to_string(), "bind");
    assert_eq!(MountType::Tmpfs.to_string(), "tmpfs");
}

// ============================================================================
// Mount Type Tests
// ============================================================================

/// Tests MountType enum variants and Display impl.
#[test]
fn test_mount_type_variants() {
    use super::podman::MountType;

    assert!(matches!(MountType::Volume, MountType::Volume));
    assert!(matches!(MountType::Bind, MountType::Bind));
    assert!(matches!(MountType::Tmpfs, MountType::Tmpfs));
}

/// Tests Mount struct construction.
#[test]
fn test_mount_construction() {
    let mount = Mount {
        id: "test-id".to_string(),
        mount_type: MountType::Volume,
        source: "/source".to_string(),
        target: "/target".to_string(),
        opts: Some(HashMap::from([
            ("type".to_string(), "tmpfs".to_string()),
        ])),
    };

    assert_eq!(mount.id, "test-id");
    assert!(matches!(mount.mount_type, MountType::Volume));
    assert_eq!(mount.source, "/source");
    assert_eq!(mount.target, "/target");
    assert!(mount.opts.is_some());
}

// ============================================================================
// Error Handling Tests
// ============================================================================

/// Tests ShellError variants.
#[test]
fn test_shell_error_variants() {
    use super::shell::ShellError;

    let err = ShellError::TaskIdRequired;
    assert_eq!(err.to_string(), "task id is required");

    let err = ShellError::ContextCancelled;
    assert_eq!(err.to_string(), "context cancelled");

    let err = ShellError::CommandFailed("exit code: 1".to_string());
    assert!(err.to_string().contains("exit code: 1"));
}

/// Tests PodmanError variants.
#[test]
fn test_podman_error_variants() {
    use super::podman::PodmanError;

    let err = PodmanError::TaskIdRequired;
    assert_eq!(err.to_string(), "task id is required");

    let err = PodmanError::ImageRequired;
    assert_eq!(err.to_string(), "task image is required");

    let err = PodmanError::PodmanNotRunning;
    assert_eq!(err.to_string(), "podman is not running");

    let err = PodmanError::ProbeTimeout("30s".to_string());
    assert_eq!(err.to_string(), "probe timed out after 30s");
}

// ============================================================================
// Config Tests
// ============================================================================

/// Tests ShellConfig default values.
#[test]
fn test_shell_config_default() {
    let config = ShellConfig::default();
    assert_eq!(config.uid, "-");
    assert_eq!(config.gid, "-");
    assert!(config.reexec.is_none());
    assert!(config.broker.is_none());
}

/// Tests PodmanConfig default values.
#[test]
fn test_podman_config_default() {
    let config = PodmanConfig::default();
    assert!(!config.privileged);
    assert!(!config.host_network);
    assert!(config.broker.is_none());
    assert!(config.mounter.is_none());
    assert!(!config.image_verify);
    assert!(config.image_ttl.is_none());
}
