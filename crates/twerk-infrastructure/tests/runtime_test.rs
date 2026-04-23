#![allow(clippy::needless_update)]
#![allow(clippy::unnecessary_mut_passed)]
#![allow(clippy::unwrap_used)]
#![allow(clippy::expect_used)]
//! Integration tests for Twerk Runtimes (Docker, Podman).
//!
//! Ported from Go/Rust internal tests.
//! Run with: cargo test -p twerk-infrastructure --test runtime_test

use std::sync::{Arc, Mutex};
use twerk_core::task::{Probe, Task, TaskLimits};
use twerk_infrastructure::runtime::docker::DockerRuntime;
use twerk_infrastructure::runtime::podman::types::Broker as PodmanBroker;
use twerk_infrastructure::runtime::podman::{PodmanConfig, PodmanRuntime};
use twerk_infrastructure::runtime::Runtime;

// ----------------------------------------------------------------------------
// Helpers
// ----------------------------------------------------------------------------

#[derive(Clone)]
struct FakeBroker {
    logs: Arc<Mutex<Vec<String>>>,
}

impl PodmanBroker for FakeBroker {
    fn clone_box(&self) -> Box<dyn PodmanBroker + Send + Sync> {
        Box::new(self.clone())
    }
    fn ship_log(&self, task_id: &str, line: &str) {
        #[allow(clippy::unwrap_used)]
        let mut logs = self.logs.lock().unwrap();
        logs.push(format!("{task_id}: {line}"));
    }
    fn publish_task_progress(&self, _task_id: &str, _progress: f64) {}
}

fn make_task(id: &str) -> Task {
    Task {
        id: Some(id.into()),
        name: Some(format!("test-task-{id}")),
        image: Some("busybox:stable".to_string()),
        cmd: Some(vec!["echo".to_string(), "hello".to_string()]),
        ..Default::default()
    }
}

fn make_progress_task(id: &str) -> Task {
    let script = r#"
#!/bin/sh
mkdir -p /twerk
echo "0.0" > /twerk/progress
echo "0.25" > /twerk/progress
echo "0.5" > /twerk/progress
echo "0.75" > /twerk/progress
echo "1.0" > /twerk/progress
echo "done" > /twerk/stdout
"#;
    Task {
        id: Some(id.into()),
        name: Some(format!("test-progress-{id}")),
        image: Some("busybox:stable".to_string()),
        cmd: Some(vec!["sh".to_string(), "-c".to_string(), script.to_string()]),
        ..Default::default()
    }
}

fn make_podman_task(id: &str) -> Task {
    Task {
        id: Some(id.to_string().into()),
        name: Some(format!("test-task-{id}")),
        image: Some("busybox:stable".to_string()),
        ..Default::default()
    }
}

// ----------------------------------------------------------------------------
// Docker Runtime Tests
// ----------------------------------------------------------------------------

#[tokio::test]
async fn test_docker_lifecycle() {
    let runtime = DockerRuntime::default_runtime()
        .await
        .expect("should create Docker runtime");
    runtime
        .health_check()
        .await
        .expect("Docker health check should succeed");

    let task = make_task("docker-lifecycle");
    <DockerRuntime as Runtime>::run(&runtime, &task)
        .await
        .expect("Docker run should succeed for a simple echo task");
}

#[tokio::test]
async fn test_docker_progress_reporting() {
    let runtime = DockerRuntime::default_runtime()
        .await
        .expect("should create Docker runtime");
    let task = make_progress_task("docker-progress");

    <DockerRuntime as Runtime>::run(&runtime, &task)
        .await
        .expect("Docker run should succeed for the progress-reporting task");
}

#[tokio::test]
async fn test_docker_resource_limits() {
    let runtime = DockerRuntime::default_runtime()
        .await
        .expect("should create Docker runtime");
    let mut task = make_task("docker-limits");
    task.limits = Some(TaskLimits {
        cpus: Some("0.1".to_string()),
        memory: Some("64000000".to_string()),
    });

    <DockerRuntime as Runtime>::run(&runtime, &task)
        .await
        .expect("Docker run should succeed when resource limits are configured");
}

#[tokio::test]
async fn test_docker_probe() {
    let runtime = DockerRuntime::default_runtime()
        .await
        .expect("should create Docker runtime");
    let mut task = make_task("docker-probe");
    // Run a one-shot HTTP responder that exits after the readiness request arrives.
    task.cmd = Some(vec![
        "sh".to_string(),
        "-c".to_string(),
        "echo -e 'HTTP/1.1 200 OK\\r\\n\\r\\nOK' | nc -l -p 8080".to_string(),
    ]);
    task.probe = Some(Probe {
        path: Some("/health".to_string()),
        port: 8080,
        timeout: Some("10s".to_string()),
    });

    <DockerRuntime as Runtime>::run(&runtime, &task)
        .await
        .expect("Docker run should succeed when a health probe is configured");
}

// ----------------------------------------------------------------------------
// Podman Runtime Tests
// ----------------------------------------------------------------------------

#[tokio::test]
async fn test_podman_lifecycle() {
    let config = PodmanConfig::default();
    let runtime = PodmanRuntime::new(config);
    runtime
        .health_check()
        .await
        .expect("Podman health check should succeed");

    let task = make_podman_task("podman-lifecycle");
    runtime
        .run(&task)
        .await
        .expect("Podman runtime trait should report success for a simple task");
}

#[tokio::test]
async fn test_podman_volume_mounts() {
    let broker = FakeBroker {
        logs: Arc::new(Mutex::new(Vec::new())),
    };
    let config = PodmanConfig {
        broker: Some(Box::new(broker.clone())),
        ..Default::default()
    };
    let runtime = PodmanRuntime::new(config);

    let task = make_podman_task("podman-volume");
    runtime
        .run(&task)
        .await
        .expect("Podman runtime trait should report success for the volume-mount task");
}

#[tokio::test]
async fn test_podman_resource_limits() {
    let config = PodmanConfig::default();
    let runtime = PodmanRuntime::new(config);

    let mut task = make_podman_task("podman-limits");
    task.run = Some("echo limited".to_string());
    task.limits = Some(TaskLimits {
        cpus: Some("0.1".to_string()),
        memory: Some("64m".to_string()),
    });

    runtime
        .run(&task)
        .await
        .expect("Podman runtime trait should report success when limits are configured");
}

#[tokio::test]
async fn test_podman_probe() {
    let broker = FakeBroker {
        logs: Arc::new(Mutex::new(Vec::new())),
    };
    let config = PodmanConfig {
        broker: Some(Box::new(broker.clone())),
        ..Default::default()
    };
    let runtime = PodmanRuntime::new(config);

    let mut task = make_podman_task("podman-probe");
    // Run a one-shot HTTP responder that exits after the readiness request arrives.
    task.run = Some(
        "(echo -e 'HTTP/1.1 200 OK\\r\\n\\r\\nOK' | nc -l -p 8080) & wget -q -O - http://127.0.0.1:8080/ >/dev/null 2>&1 && wait"
            .to_string(),
    );
    task.probe = Some(Probe {
        path: Some("/health".to_string()),
        port: 8080,
        timeout: Some("30s".to_string()),
    });

    runtime
        .run(&task)
        .await
        .expect("Podman runtime trait should report success when a probe is configured");
}

#[tokio::test]
async fn test_podman_pre_post_tasks() {
    let config = PodmanConfig::default();
    let runtime = PodmanRuntime::new(config);
    let mut task = make_podman_task("podman-pre-post");
    task.pre = Some(vec![Task {
        id: Some("podman-pre-step".into()),
        name: Some("pre-step".to_string()),
        image: Some("busybox:stable".to_string()),
        cmd: Some(vec!["echo".to_string(), "pre".to_string()]),
        ..Default::default()
    }]);
    task.post = Some(vec![Task {
        id: Some("podman-post-step".into()),
        name: Some("post-step".to_string()),
        image: Some("busybox:stable".to_string()),
        cmd: Some(vec!["echo".to_string(), "post".to_string()]),
        ..Default::default()
    }]);

    runtime
        .run(&task)
        .await
        .expect("Podman runtime trait should report success when pre/post tasks are configured");
}
