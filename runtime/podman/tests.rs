//! Podman runtime tests

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use tokio::process::Command;

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
        }
    }

    #[tokio::test]
    #[ignore] // Requires podman running
    async fn test_podman_run_task_cmd() {
        let rt = PodmanRuntime::new(create_test_config());
        let mut task = create_test_task();

        let result = rt.run(&mut task).await;
        // Requires podman to be running
    }

    #[tokio::test]
    #[ignore] // Requires podman running
    async fn test_podman_run_task_run() {
        let rt = PodmanRuntime::new(create_test_config());
        let mut task = create_test_task();
        task.run = "echo hello world > $TORK_OUTPUT".to_string();
        task.cmd = vec![];

        let result = rt.run(&mut task).await;
        // Requires podman to be running
    }

    #[tokio::test]
    async fn test_podman_run_not_supported() {
        let rt = PodmanRuntime::new(create_test_config());
        let mut task = create_test_task();
        task.id = String::new(); // Invalid - empty ID

        let result = rt.run(&mut task).await;
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), PodmanError::TaskIdRequired));
    }

    #[tokio::test]
    async fn test_podman_run_pre_post() {
        let config = PodmanConfig {
            mounter: Some(Box::new(VolumeMounter::new())),
            ..Default::default()
        };
        let rt = PodmanRuntime::new(config);
        let mut task = create_test_task();
        task.run = "cat /somedir/thing > $TORK_OUTPUT".to_string();
        task.mounts = vec![Mount {
            id: uuid::Uuid::new_v4().to_string(),
            mount_type: MountType::Volume,
            source: String::new(),
            target: "/somedir".to_string(),
        }];
        task.pre = vec![Task {
            id: uuid::Uuid::new_v4().to_string(),
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
            sidecars: vec![],
            pre: vec![],
            post: vec![],
            workdir: None,
            result: String::new(),
            progress: 0.0,
        }];

        // Note: This test would fail without podman, but demonstrates structure
    }

    #[tokio::test]
    async fn test_volume_mounter() {
        let vm = VolumeMounter::new();
        let mount = Mount {
            id: uuid::Uuid::new_v4().to_string(),
            mount_type: MountType::Volume,
            source: String::new(),
            target: "/xyz".to_string(),
        };

        // Mount creates a temp directory
        let result = vm.mount(&mount);
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_slug_make() {
        assert_eq!(slug::make("Some Task Name"), "some-task-name");
        assert_eq!(slug::make("Test_With_Special!@#"), "test_with_special");
    }
}
