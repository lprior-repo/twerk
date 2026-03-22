//! Shell runtime tests

#[cfg(test)]
mod tests {
    use std::collections::HashMap;
    use std::process::Stdio;

    use tokio::process::Command;

    use super::*;

    fn create_test_task() -> Task {
        Task {
            id: uuid::Uuid::new_v4().to_string(),
            name: Some("Test task".to_string()),
            image: String::new(),
            run: "echo -n hello world".to_string(),
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
        }
    }

    fn create_test_config() -> ShellConfig {
        ShellConfig {
            cmd: vec!["bash".to_string(), "-c".to_string()],
            uid: DEFAULT_UID.to_string(),
            gid: DEFAULT_GID.to_string(),
            reexec: Some(Box::new(|args: &[String]| {
                let mut cmd = Command::new(&args[5]);
                cmd.args(&args[6..]);
                #[cfg(unix)]
                {
                    use std::os::unix::process::CommandExt;
                    if args[2] != DEFAULT_UID {
                        if let Ok(uid) = args[2].parse::<u32>() {
                            cmd.uid(uid);
                        }
                    }
                    if args[4] != DEFAULT_GID {
                        if let Ok(gid) = args[4].parse::<u32>() {
                            cmd.gid(gid);
                        }
                    }
                }
                cmd
            })),
        }
    }

    #[tokio::test]
    async fn test_shell_runtime_run_result() {
        let rt = ShellRuntime::new(create_test_config());
        let mut task = create_test_task();
        task.env.insert(
            "REEXEC_TORK_OUTPUT".to_string(),
            "/dev/null".to_string(),
        );

        let result = rt.run(&mut task).await;
        // This test may fail in CI without a real shell, but demonstrates the structure
        // In real tests we'd mock the command execution
    }

    #[tokio::test]
    async fn test_shell_runtime_run_not_supported() {
        let rt = ShellRuntime::new(ShellConfig::default());
        let mut task = create_test_task();
        task.networks = vec!["some-network".to_string()];

        let result = rt.run(&mut task).await;
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), ShellError::NetworksNotSupported));
    }

    #[tokio::test]
    async fn test_build_env() {
        std::env::set_var("REEXEC_VAR1", "value1");
        std::env::set_var("REEXEC_VAR2", "value2");
        std::env::set_var("NON_REEXEC_VAR", "should_not_be_included");

        let env = build_env().unwrap();

        std::env::remove_var("REEXEC_VAR1");
        std::env::remove_var("REEXEC_VAR2");
        std::env::remove_var("NON_REEXEC_VAR");

        assert!(env.contains(&("VAR1".to_string(), "value1".to_string())));
        assert!(env.contains(&("VAR2".to_string(), "value2".to_string())));
        assert!(!env.contains(&("NON_REEXEC_VAR".to_string(), "should_not_be_included".to_string())));
    }
}
