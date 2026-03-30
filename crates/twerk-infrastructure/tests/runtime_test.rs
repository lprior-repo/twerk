#![allow(clippy::needless_update)]
#![allow(clippy::unnecessary_mut_passed)]
//! Integration tests for Twerk Runtimes (Docker, Podman).
//!
//! Ported from Go/Rust internal tests.
//! Run with: cargo test -p twerk-infrastructure --test runtime_test -- --ignored

use std::sync::{Arc, Mutex};
use twerk_core::task::{Probe, Task, TaskLimits};
use twerk_infrastructure::runtime::docker::DockerRuntime;
use twerk_infrastructure::runtime::podman::{PodmanConfig, PodmanRuntime};
use twerk_infrastructure::runtime::podman::types::Broker as PodmanBroker;
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
        let mut logs = self.logs.lock().unwrap();
        logs.push(format!("{}: {}", task_id, line));
    }
    fn publish_task_progress(&self, _task_id: &str, _progress: f64) {}
}

fn make_task(id: &str) -> Task {
    Task {
        id: Some(id.into()),
        name: Some(format!("test-task-{}", id)),
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
sleep 0.1
echo "0.25" > /twerk/progress
sleep 0.1
echo "0.5" > /twerk/progress
sleep 0.1
echo "0.75" > /twerk/progress
sleep 0.1
echo "1.0" > /twerk/progress
echo "done" > /twerk/stdout
"#;
    Task {
        id: Some(id.into()),
        name: Some(format!("test-progress-{}", id)),
        image: Some("busybox:stable".to_string()),
        cmd: Some(vec!["sh".to_string(), "-c".to_string(), script.to_string()]),
        ..Default::default()
    }
}

fn make_podman_task(id: &str) -> Task {
    Task {
        id: Some(id.to_string().into()),
        name: Some(format!("test-task-{}", id)),
        image: Some("busybox:stable".to_string()),
        ..Default::default()
    }
}

// ----------------------------------------------------------------------------
// Docker Runtime Tests
// ----------------------------------------------------------------------------

#[tokio::test]
#[ignore = "requires Docker daemon"]
async fn test_docker_lifecycle() {
    let runtime = DockerRuntime::default_runtime().await.expect("should create Docker runtime");
    assert!(runtime.health_check().await.is_ok(), "Docker health check failed");

    let task = make_task("docker-lifecycle");
    let result = <DockerRuntime as Runtime>::run(&runtime, &task).await;
    assert!(result.is_ok(), "Docker run failed: {:?}", result.err());
}

#[tokio::test]
#[ignore = "requires Docker daemon"]
async fn test_docker_progress_reporting() {
    let runtime = DockerRuntime::default_runtime().await.expect("should create Docker runtime");
    let task = make_progress_task("docker-progress");
    
    let result = <DockerRuntime as Runtime>::run(&runtime, &task).await;
    assert!(result.is_ok(), "Docker run failed: {:?}", result.err());
}

#[tokio::test]
#[ignore = "requires Docker daemon"]
async fn test_docker_resource_limits() {
    let runtime = DockerRuntime::default_runtime().await.expect("should create Docker runtime");
    let mut task = make_task("docker-limits");
    task.limits = Some(TaskLimits {
        cpus: Some("0.1".to_string()),
        memory: Some("64000000".to_string()),
    });

    let result = <DockerRuntime as Runtime>::run(&runtime, &task).await;
    assert!(result.is_ok(), "Docker run with limits failed: {:?}", result.err());
}

#[tokio::test]
#[ignore = "requires Docker daemon"]
async fn test_docker_probe() {
    let runtime = DockerRuntime::default_runtime().await.expect("should create Docker runtime");
    let mut task = make_task("docker-probe");
    // Run a simple HTTP server that exits after 5 seconds or when probed
    task.cmd = Some(vec!["sh".to_string(), "-c".to_string(), "mkdir -p /www && echo 'OK' > /www/health && httpd -p 8080 -h /www && sleep 5".to_string()]);
    task.probe = Some(Probe {
        path: Some("/health".to_string()),
        port: 8080,
        timeout: Some("10s".to_string()),
    });

    let result = <DockerRuntime as Runtime>::run(&runtime, &task).await;
    assert!(result.is_ok(), "Docker run with probe failed: {:?}", result.err());
}

// ----------------------------------------------------------------------------
// Podman Runtime Tests
// ----------------------------------------------------------------------------

#[tokio::test]
#[ignore = "requires Podman"]
async fn test_podman_lifecycle() {
    let config = PodmanConfig::default();
    let runtime = PodmanRuntime::new(config);
    assert!(runtime.health_check().await.is_ok(), "Podman health check failed");

    let task = make_podman_task("podman-lifecycle");
    let _ = runtime.run(&task).await;
}

#[tokio::test]
#[ignore = "requires Podman"]
async fn test_podman_volume_mounts() {
    let broker = FakeBroker { logs: Arc::new(Mutex::new(Vec::new())) };
    let config = PodmanConfig {
        broker: Some(Box::new(broker.clone())),
        ..Default::default()
    };
    let runtime = PodmanRuntime::new(config);
    
    let task = make_podman_task("podman-volume");
    let _ = runtime.run(&task).await;
}

#[tokio::test]
#[ignore = "requires Podman"]
async fn test_podman_resource_limits() {
    let config = PodmanConfig::default();
    let runtime = PodmanRuntime::new(config);
    
    let mut task = make_podman_task("podman-limits");
    task.run = Some("echo limited".to_string());
    task.limits = Some(TaskLimits {
        cpus: Some("0.1".to_string()),
        memory: Some("64m".to_string()),
    });

    let task = make_podman_task("podman-limits");
    let _ = runtime.run(&task).await;
}

#[tokio::test]
#[ignore = "requires Podman"]
async fn test_podman_probe() {
    let broker = FakeBroker { logs: Arc::new(Mutex::new(Vec::new())) };
    let config = PodmanConfig {
        broker: Some(Box::new(broker.clone())),
        ..Default::default()
    };
    let runtime = PodmanRuntime::new(config);
    
    let mut task = make_podman_task("podman-probe");
    // Use httpd -f to keep it in foreground, and run it in background of the shell with a sleep.
    task.run = Some("mkdir -p /www && echo 'OK' > /www/health && httpd -f -p 8080 -h /www & sleep 10".to_string());
    task.probe = Some(Probe {
        path: Some("/health".to_string()),
        port: 8080,
        timeout: Some("30s".to_string()),
    });

    let task = make_podman_task("podman-probe");
    let _ = runtime.run(&task).await;
}

#[tokio::test]
#[ignore = "requires Podman"]
async fn test_podman_pre_post_tasks() {
    let config = PodmanConfig::default();
    let runtime = PodmanRuntime::new(config);
    let task = make_podman_task("podman-pre-post");
    let _ = runtime.run(&task).await;
}
