//! Podman runtime execution tests with mounts, files, env, etc.

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

// ── Pre/Post task tests ────────────────────────────────────────────

/// Mirrors Go's TestPodmanRunPrePost.
#[tokio::test]
async fn test_podman_run_pre_post() {
    let config = PodmanConfig {
        mounter: Some(Box::new(VolumeMounter::new())),
        ..Default::default()
    };
    let rt = PodmanRuntime::new(config);
    let mut task = create_test_task();
    task.run = "cat /somedir/thing > $TWERK_OUTPUT".to_string();
    task.cmd = vec![];
    task.mounts = vec![Mount {
        id: uuid::Uuid::new_v4().to_string(),
        mount_type: MountType::Volume,
        source: String::new(),
        target: "/somedir".to_string(),
        opts: None,
    }];
    task.pre = vec![Task {
        id: String::new(),
        name: Some("Pre task".to_string()),
        image: "busybox:stable".to_string(),
        run: "echo hello > /somedir/thing".to_string(),
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
    }];
    task.post = vec![Task {
        id: String::new(),
        name: Some("Post task".to_string()),
        image: "busybox:stable".to_string(),
        run: "echo post".to_string(),
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
    }];

    let result = rt.run(&mut task).await;
    assert!(
        result.is_ok(),
        "pre/post should succeed: {:?}",
        result.err()
    );
    assert_eq!("hello\n", task.result);
}

/// Test pre and post tasks with volume mounts preserved.
#[tokio::test]
async fn test_podman_pre_post_with_volume() {
    let config = PodmanConfig {
        mounter: Some(Box::new(VolumeMounter::new())),
        ..Default::default()
    };
    let rt = PodmanRuntime::new(config);
    let mut task = create_test_task();
    task.run = "cat /shared/data.txt > $TWERK_OUTPUT".to_string();
    task.cmd = vec![];
    task.mounts = vec![Mount {
        id: uuid::Uuid::new_v4().to_string(),
        mount_type: MountType::Volume,
        source: String::new(),
        target: "/shared".to_string(),
        opts: None,
    }];
    task.pre = vec![Task {
        id: String::new(),
        name: Some("Pre task".to_string()),
        image: "busybox:stable".to_string(),
        run: "echo pre_data > /shared/data.txt".to_string(),
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
    }];
    task.post = vec![Task {
        id: String::new(),
        name: Some("Post task".to_string()),
        image: "busybox:stable".to_string(),
        run: "echo post_data >> /shared/data.txt".to_string(),
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
    }];

    let result = rt.run(&mut task).await;
    assert!(
        result.is_ok(),
        "pre/post with volume should succeed: {:?}",
        result.err()
    );
    assert_eq!("pre_data\n", task.result);
}

// ── Mount tests ────────────────────────────────────────────────────

/// Mirrors Go's TestPodmanRunTaskWithVolume.
#[tokio::test]
async fn test_podman_run_task_with_volume() {
    let rt = PodmanRuntime::new(create_test_config());
    let mut task = create_test_task();
    task.run = "echo hello world > /xyz/thing".to_string();
    task.cmd = vec![];
    task.mounts = vec![Mount {
        id: uuid::Uuid::new_v4().to_string(),
        mount_type: MountType::Volume,
        source: String::new(),
        target: "/xyz".to_string(),
        opts: None,
    }];

    let result = rt.run(&mut task).await;
    assert!(
        result.is_ok(),
        "volume mount should succeed: {:?}",
        result.err()
    );
}

/// Mirrors Go's TestPodmanRunTaskWithVolumeAndCustomWorkdir.
#[tokio::test]
async fn test_podman_run_volume_custom_workdir() {
    let rt = PodmanRuntime::new(create_test_config());
    let mut task = create_test_task();
    task.run = "echo hello world > /xyz/thing\nls > $TWERK_OUTPUT".to_string();
    task.cmd = vec![];
    task.mounts = vec![Mount {
        id: uuid::Uuid::new_v4().to_string(),
        mount_type: MountType::Volume,
        source: String::new(),
        target: "/xyz".to_string(),
        opts: None,
    }];
    task.workdir = Some("/xyz".to_string());

    let result = rt.run(&mut task).await;
    assert!(
        result.is_ok(),
        "volume+workdir should succeed: {:?}",
        result.err()
    );
    assert_eq!("thing\n", task.result);
}

/// Mirrors Go's TestPodmanRunTaskWithVolumeAndWorkdir.
#[tokio::test]
async fn test_podman_run_volume_and_workdir() {
    let rt = PodmanRuntime::new(create_test_config());
    let mut task = create_test_task();
    task.run = "echo hello world > ./thing".to_string();
    task.cmd = vec![];
    task.mounts = vec![Mount {
        id: uuid::Uuid::new_v4().to_string(),
        mount_type: MountType::Volume,
        source: String::new(),
        target: "/xyz".to_string(),
        opts: None,
    }];
    task.workdir = Some("/xyz".to_string());

    let result = rt.run(&mut task).await;
    assert!(
        result.is_ok(),
        "volume+workdir should succeed: {:?}",
        result.err()
    );
}

/// Test bind mount type.
#[tokio::test]
async fn test_podman_bind_mount() {
    let rt = PodmanRuntime::new(create_test_config());
    let mut task = create_test_task();
    task.run = "echo bind_mount > /mnt/testfile && cat /mnt/testfile > $TWERK_OUTPUT".to_string();
    task.cmd = vec![];
    task.mounts = vec![Mount {
        id: uuid::Uuid::new_v4().to_string(),
        mount_type: MountType::Bind,
        source: String::new(),
        target: "/mnt".to_string(),
        opts: None,
    }];

    let result = rt.run(&mut task).await;
    assert!(
        result.is_ok(),
        "bind mount should succeed: {:?}",
        result.err()
    );
    assert_eq!("bind_mount\n", task.result);
}

/// Test tmpfs mount type.
#[tokio::test]
async fn test_podman_tmpfs_mount() {
    let rt = PodmanRuntime::new(create_test_config());
    let mut task = create_test_task();
    task.run = "echo tmpfs_test > /tmpfs/data.txt && cat /tmpfs/data.txt > $TWERK_OUTPUT".to_string();
    task.cmd = vec![];
    task.mounts = vec![Mount {
        id: uuid::Uuid::new_v4().to_string(),
        mount_type: MountType::Tmpfs,
        source: String::new(),
        target: "/tmpfs".to_string(),
        opts: Some(HashMap::from([("size".to_string(), "10m".to_string())])),
    }];

    let result = rt.run(&mut task).await;
    assert!(
        result.is_ok(),
        "tmpfs mount should succeed: {:?}",
        result.err()
    );
    assert_eq!("tmpfs_test\n", task.result);
}

/// Test multiple volume mounts.
#[tokio::test]
async fn test_podman_multiple_volume_mounts() {
    let rt = PodmanRuntime::new(create_test_config());
    let mut task = create_test_task();
    task.run = "echo volume1 > /vol1/data.txt && echo volume2 > /vol2/data.txt && cat /vol1/data.txt /vol2/data.txt > $TWERK_OUTPUT".to_string();
    task.cmd = vec![];
    task.mounts = vec![
        Mount {
            id: uuid::Uuid::new_v4().to_string(),
            mount_type: MountType::Volume,
            source: String::new(),
            target: "/vol1".to_string(),
            opts: None,
        },
        Mount {
            id: uuid::Uuid::new_v4().to_string(),
            mount_type: MountType::Volume,
            source: String::new(),
            target: "/vol2".to_string(),
            opts: None,
        },
    ];

    let result = rt.run(&mut task).await;
    assert!(
        result.is_ok(),
        "multiple volume mounts should succeed: {:?}",
        result.err()
    );
    assert_eq!("volume1\nvolume2\n", task.result);
}

/// Test volume mount with no options.
#[tokio::test]
async fn test_podman_volume_no_opts() {
    let rt = PodmanRuntime::new(create_test_config());
    let mut task = create_test_task();
    task.run = "echo volume_baseline > /baseline/data.txt && cat /baseline/data.txt > $TWERK_OUTPUT"
        .to_string();
    task.cmd = vec![];
    task.mounts = vec![Mount {
        id: uuid::Uuid::new_v4().to_string(),
        mount_type: MountType::Volume,
        source: String::new(),
        target: "/baseline".to_string(),
        opts: None,
    }];

    let result = rt.run(&mut task).await;
    assert!(
        result.is_ok(),
        "volume without opts should succeed: {:?}",
        result.err()
    );
    assert_eq!("volume_baseline\n", task.result);
}
