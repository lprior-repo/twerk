//! Podman runtime validation tests — no podman required.
//!
//! Tests input validation, error cases, and configuration checks.

#![allow(clippy::redundant_pattern_matching)]

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

// ── Validation tests (no podman required) ──────────────────────────

#[tokio::test]
async fn test_podman_run_not_supported_empty_id() {
    let rt = PodmanRuntime::new(create_test_config());
    let mut task = create_test_task();
    task.id = String::new();

    let result = rt.run(&mut task).await;
    assert!(matches!(result, Err(_)));
    assert!(matches!(result.unwrap_err(), PodmanError::TaskIdRequired));
}

#[tokio::test]
async fn test_podman_run_not_supported_empty_image() {
    let rt = PodmanRuntime::new(create_test_config());
    let mut task = create_test_task();
    task.image = String::new();

    let result = rt.run(&mut task).await;
    assert!(matches!(result, Err(_)));
    assert!(matches!(result.unwrap_err(), PodmanError::ImageRequired));
}

#[tokio::test]
async fn test_podman_run_not_supported_empty_name() {
    let rt = PodmanRuntime::new(create_test_config());
    let mut task = create_test_task();
    task.name = None;

    let result = rt.run(&mut task).await;
    assert!(matches!(result, Err(_)));
    assert!(matches!(result.unwrap_err(), PodmanError::NameRequired));
}

#[tokio::test]
async fn test_podman_run_not_supported_sidecars() {
    let rt = PodmanRuntime::new(create_test_config());
    let mut task = create_test_task();
    task.sidecars.push(create_test_task());

    let result = rt.run(&mut task).await;
    assert!(matches!(result, Err(_)));
    assert!(matches!(
        result.unwrap_err(),
        PodmanError::SidecarsNotSupported
    ));
}

#[tokio::test]
async fn test_podman_host_network_disabled() {
    let rt = PodmanRuntime::new(create_test_config());
    let mut task = create_test_task();
    task.networks = vec!["host".to_string()];

    let result = rt.run(&mut task).await;
    assert!(matches!(result, Err(_)));
    assert!(matches!(
        result.unwrap_err(),
        PodmanError::HostNetworkingDisabled
    ));
}

// ── GAP3: Network name validation ──────────────────────────────────

/// GAP3: When networks are specified but name is None, should return NameRequiredForNetwork
#[tokio::test]
async fn test_podman_runtime_returns_name_required_for_network_when_networks_specified_without_name() {
    let rt = PodmanRuntime::new(create_test_config());

    let mut task = create_test_task();
    task.name = None;
    task.networks = vec!["mynet".to_string()];

    let result = rt.run(&mut task).await;

    assert!(matches!(result, Err(_)), "should fail when networks specified but name is empty");
    let err = result.unwrap_err();
    
    match err {
        PodmanError::NameRequiredForNetwork => {}, // Correct after fix
        PodmanError::NameRequired => {
            panic!("Got NameRequired but expected NameRequiredForNetwork for GAP3 fix");
        }
        other => {
            panic!("Got unexpected error: {:?}", other);
        }
    }
}

/// GAP3: When networks are specified with empty name string (Some(""))
#[tokio::test]
async fn test_podman_runtime_returns_name_required_for_network_when_networks_specified_with_empty_name() {
    let rt = PodmanRuntime::new(create_test_config());

    let mut task = create_test_task();
    task.name = Some("".to_string());
    task.networks = vec!["mynet".to_string()];

    let result = rt.run(&mut task).await;

    assert!(matches!(result, Err(_)), "should fail when networks specified but name is empty");
    let err = result.unwrap_err();
    
    match err {
        PodmanError::NameRequiredForNetwork => {}, // Correct after fix
        PodmanError::NameRequired => {
            panic!("Got NameRequired but expected NameRequiredForNetwork for GAP3 fix");
        }
        other => {
            panic!("Got unexpected error: {:?}", other);
        }
    }
}

// ── GAP7: sidecars not supported ───────────────────────────────────

/// GAP7: PodmanRuntime does NOT support sidecars
#[tokio::test]
async fn test_podman_runtime_returns_sidecars_not_supported_when_sidecars_specified() {
    let rt = PodmanRuntime::new(create_test_config());

    let mut task = create_test_task();
    task.sidecars.push(Task {
        id: String::new(),
        name: Some("sidecar".to_string()),
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
    });

    let result = rt.run(&mut task).await;

    assert!(matches!(result, Err(_)), "should fail when sidecars specified");
    let err = result.unwrap_err();
    assert!(
        matches!(err, PodmanError::SidecarsNotSupported),
        "expected SidecarsNotSupported error, got: {:?}",
        err
    );
}
