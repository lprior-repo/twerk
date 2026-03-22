//! Shell runtime tests — matching Go's shell_test.go 1:1.
//!
//! # Go Test Mapping
//!
//! | Go Test                          | Rust Test                          |
//! |----------------------------------|------------------------------------|
//! | TestShellRuntimeRunResult        | test_shell_runtime_run_result       |
//! | TestShellRuntimeRunPath          | test_shell_runtime_run_path         |
//! | TestShellRuntimeRunFile          | test_shell_runtime_run_file         |
//! | TestShellRuntimeRunNotSupported  | test_shell_runtime_run_not_supported|
//! | TestShellRuntimeRunError         | test_shell_runtime_run_error        |
//! | TestShellRuntimeRunTimeout       | test_shell_runtime_run_timeout      |
//! | TestRunTaskCMDLogger             | test_run_task_cmd_logger            |
//! | TestBuildEnv                     | test_build_env                      |
//! | TestRunTaskWithPrePost           | test_run_task_with_pre_post         |

use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

use tokio::process::Command;

use super::*;

fn create_test_task() -> Task {
    Task {
        id: uuid::Uuid::new_v4().to_string(),
        name: None,
        image: String::new(),
        run: String::new(),
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
            cmd
        })),
        broker: None,
    }
}

fn create_no_cancel() -> Arc<AtomicBool> {
    Arc::new(AtomicBool::new(false))
}

#[tokio::test]
async fn test_shell_runtime_run_result() {
    let rt = ShellRuntime::new(create_test_config());
    let mut task = create_test_task();
    task.run = "echo -n hello world > $REEXEC_TORK_OUTPUT".to_string();

    let result = rt.run(create_no_cancel(), &mut task).await;

    assert!(result.is_ok(), "run should succeed: {:?}", result.err());
    assert_eq!("hello world", task.result);
}

#[tokio::test]
async fn test_shell_runtime_run_path() {
    let rt = ShellRuntime::new(create_test_config());
    let mut task = create_test_task();
    task.run = "echo -n $PATH > $REEXEC_TORK_OUTPUT".to_string();

    let result = rt.run(create_no_cancel(), &mut task).await;

    assert!(result.is_ok(), "run should succeed: {:?}", result.err());
    assert!(!task.result.is_empty(), "PATH result should not be empty");
}

#[tokio::test]
async fn test_shell_runtime_run_file() {
    let rt = ShellRuntime::new(create_test_config());
    let mut task = create_test_task();
    task.run = "cat hello.txt > $REEXEC_TORK_OUTPUT".to_string();
    task.files
        .insert("hello.txt".to_string(), "hello world".to_string());

    let result = rt.run(create_no_cancel(), &mut task).await;

    assert!(result.is_ok(), "run should succeed: {:?}", result.err());
    assert_eq!("hello world", task.result);
}

#[tokio::test]
async fn test_shell_runtime_run_not_supported() {
    let rt = ShellRuntime::new(ShellConfig::default());
    let mut task = create_test_task();
    task.run = "echo hello world".to_string();
    task.networks = vec!["some-network".to_string()];

    let result = rt.run(create_no_cancel(), &mut task).await;

    assert!(result.is_err());
    assert!(matches!(result.unwrap_err(), ShellError::NetworksNotSupported));
}

#[tokio::test]
async fn test_shell_runtime_run_error() {
    let rt = ShellRuntime::new(create_test_config());
    let mut task = create_test_task();
    task.run = "no_such_command_xyz_12345".to_string();

    let result = rt.run(create_no_cancel(), &mut task).await;

    assert!(result.is_err(), "run with invalid command should fail");
}

#[tokio::test]
async fn test_shell_runtime_run_timeout() {
    let rt = ShellRuntime::new(create_test_config());
    let cancel = Arc::new(AtomicBool::new(false));
    let mut task = create_test_task();
    task.run = "sleep 30".to_string();

    let cancel_clone = cancel.clone();
    tokio::spawn(async move {
        tokio::time::sleep(Duration::from_secs(1)).await;
        cancel_clone.store(true, Ordering::SeqCst);
    });

    let result = rt.run(cancel, &mut task).await;

    assert!(result.is_err(), "run should be cancelled: {:?}", result);
    assert!(matches!(result.unwrap_err(), ShellError::ContextCancelled));
}

#[tokio::test]
async fn test_run_task_cmd_logger() {
    use std::sync::Mutex;

    let broker = crate::broker::inmemory::new_in_memory_broker();
    let received = Arc::new(Mutex::new(false));
    let received_clone = received.clone();

    let handler: tork::broker::TaskLogPartHandler =
        Arc::new(move |_part: tork::task::TaskLogPart| {
            let flag = received_clone.clone();
            Box::pin(async move {
                let mut guard = flag.lock().expect("lock poisoned");
                *guard = true;
            })
        });

    broker
        .subscribe_for_task_log_part(handler)
        .await
        .expect("subscribe should succeed");

    let rt = ShellRuntime::new(ShellConfig {
        cmd: vec!["bash".to_string(), "-c".to_string()],
        uid: DEFAULT_UID.to_string(),
        gid: DEFAULT_GID.to_string(),
        reexec: Some(Box::new(|args: &[String]| {
            let mut cmd = Command::new(&args[5]);
            cmd.args(&args[6..]);
            cmd
        })),
        broker: Some(Arc::new(broker)),
    });

    let mut task = create_test_task();
    task.run = "echo hello".to_string();

    let result = rt.run(create_no_cancel(), &mut task).await;

    assert!(result.is_ok(), "run should succeed: {:?}", result.err());

    tokio::time::sleep(Duration::from_secs(2)).await;

    let guard = received.lock().expect("lock poisoned");
    assert!(*guard, "log part should have been received");
}

#[test]
fn test_build_env() {
    std::env::set_var("REEXEC_VAR1", "value1");
    std::env::set_var("REEXEC_VAR2", "value2");
    std::env::set_var("NON_REEXEC_VAR", "should_not_be_included");

    let env = build_env();

    std::env::remove_var("REEXEC_VAR1");
    std::env::remove_var("REEXEC_VAR2");
    std::env::remove_var("NON_REEXEC_VAR");

    assert!(
        env.contains(&("VAR1".to_string(), "value1".to_string())),
        "env should contain VAR1=value1"
    );
    assert!(
        env.contains(&("VAR2".to_string(), "value2".to_string())),
        "env should contain VAR2=value2"
    );
    assert!(
        !env.contains(&("NON_REEXEC_VAR".to_string(), "should_not_be_included".to_string())),
        "env should not contain NON_REEXEC_VAR"
    );
}

#[tokio::test]
async fn test_run_task_with_pre_post() {
    let rt = ShellRuntime::new(create_test_config());
    let mut task = create_test_task();
    task.run = "cat /tmp/pre.txt > $REEXEC_TORK_OUTPUT".to_string();
    task.pre.push(Task {
        id: String::new(), name: None, image: String::new(),
        run: "echo hello pre > /tmp/pre.txt".to_string(),
        cmd: vec![], entrypoint: vec![], env: HashMap::new(),
        mounts: vec![], files: HashMap::new(), networks: vec![],
        limits: None, registry: None, sidecars: vec![],
        pre: vec![], post: vec![], workdir: None,
        result: String::new(), progress: 0.0,
    });
    task.post.push(Task {
        id: String::new(), name: None, image: String::new(),
        run: "echo bye bye".to_string(),
        cmd: vec![], entrypoint: vec![], env: HashMap::new(),
        mounts: vec![], files: HashMap::new(), networks: vec![],
        limits: None, registry: None, sidecars: vec![],
        pre: vec![], post: vec![], workdir: None,
        result: String::new(), progress: 0.0,
    });

    let result = rt.run(create_no_cancel(), &mut task).await;

    assert!(result.is_ok(), "run should succeed: {:?}", result.err());
    assert_eq!("hello pre\n", task.result);
}

#[tokio::test]
async fn test_health_check() {
    let rt = ShellRuntime::new(ShellConfig::default());
    assert!(rt.health_check().await.is_ok());
}

#[tokio::test]
async fn test_empty_task_id_rejected() {
    let rt = ShellRuntime::new(create_test_config());
    let mut task = create_test_task();
    task.id = String::new();

    let result = rt.run(create_no_cancel(), &mut task).await;

    assert!(result.is_err());
    assert!(matches!(result.unwrap_err(), ShellError::TaskIdRequired));
}
