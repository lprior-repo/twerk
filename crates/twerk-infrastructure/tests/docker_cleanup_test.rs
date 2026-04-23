#![allow(clippy::unwrap_used)]
#![allow(clippy::expect_used)]
//! Integration tests verifying Docker container and volume cleanup.
//!
//! Strategy: Snapshot containers and volumes before/after each task execution.
//! Assert that nothing new remains. Auto-clean up any leaks on failure.
//!
//! Run with: cargo test -p twerk-infrastructure --test docker_cleanup_test -- --ignored --test-threads=1

use bollard::query_parameters::{
    CreateImageOptions, ListContainersOptions, ListImagesOptions, RemoveContainerOptions,
    RemoveVolumeOptions,
};
use bollard::Docker;
use futures_util::StreamExt;
use std::collections::HashSet;
use twerk_core::task::Task;
use twerk_infrastructure::runtime::docker::DockerRuntime;
use twerk_infrastructure::runtime::Runtime;

fn make_echo_task(id: &str) -> Task {
    Task {
        id: Some(id.into()),
        name: Some(format!("test-cleanup-{id}")),
        image: Some("busybox:stable".to_string()),
        cmd: Some(vec!["echo".to_string(), "hello".to_string()]),
        ..Default::default()
    }
}

fn make_sidecar_task(id: &str) -> Task {
    Task {
        id: Some(id.into()),
        name: Some(format!("test-sidecar-{id}")),
        image: Some("busybox:stable".to_string()),
        cmd: Some(vec!["echo".to_string(), "main".to_string()]),
        sidecars: Some(vec![Task {
            id: Some(format!("{id}-sidecar").into()),
            name: Some(format!("sidecar-{id}")),
            image: Some("busybox:stable".to_string()),
            cmd: Some(vec!["echo".to_string(), "sidecar".to_string()]),
            ..Default::default()
        }]),
        ..Default::default()
    }
}

fn make_multi_sidecar_task(id: &str, count: usize) -> Task {
    let sidecars: Vec<Task> = (0..count)
        .map(|i| Task {
            id: Some(format!("{id}-sc-{i}").into()),
            name: Some(format!("sc-{id}-{i}")),
            image: Some("busybox:stable".to_string()),
            cmd: Some(vec!["echo".to_string(), format!("sidecar-{i}")]),
            ..Default::default()
        })
        .collect();

    Task {
        id: Some(id.into()),
        name: Some(format!("test-multi-sc-{id}")),
        image: Some("busybox:stable".to_string()),
        cmd: Some(vec!["echo".to_string(), "main".to_string()]),
        sidecars: Some(sidecars),
        ..Default::default()
    }
}

fn make_failing_task(id: &str) -> Task {
    Task {
        id: Some(id.into()),
        name: Some(format!("test-fail-{id}")),
        image: Some("busybox:stable".to_string()),
        cmd: Some(vec![
            "sh".to_string(),
            "-c".to_string(),
            "exit 1".to_string(),
        ]),
        ..Default::default()
    }
}

struct DockerSnapshot {
    containers: HashSet<String>,
    volumes: HashSet<String>,
}

impl DockerSnapshot {
    async fn capture(client: &Docker) -> Self {
        let containers: HashSet<String> = client
            .list_containers(Some(ListContainersOptions {
                all: true,
                ..Default::default()
            }))
            .await
            .unwrap_or_default()
            .iter()
            .filter_map(|c| c.id.clone())
            .collect();

        let volumes: HashSet<String> = client
            .list_volumes(None::<bollard::query_parameters::ListVolumesOptions>)
            .await
            .map(|v| {
                v.volumes
                    .unwrap_or_default()
                    .into_iter()
                    .map(|vol| vol.name)
                    .collect()
            })
            .unwrap_or_default();

        Self {
            containers,
            volumes,
        }
    }

    fn leaked_from(&self, later: &Self) -> (Vec<String>, Vec<String>) {
        let leaked_c: Vec<String> = later
            .containers
            .difference(&self.containers)
            .cloned()
            .collect();
        let leaked_v: Vec<String> = later.volumes.difference(&self.volumes).cloned().collect();
        (leaked_c, leaked_v)
    }
}

async fn ensure_busybox(client: &Docker) {
    let images = client
        .list_images(None::<ListImagesOptions>)
        .await
        .unwrap_or_default();
    let has_busybox = images
        .iter()
        .any(|i| i.repo_tags.iter().any(|t| t.starts_with("busybox:")));
    if !has_busybox {
        let mut stream = client.create_image(
            Some(CreateImageOptions {
                from_image: Some("busybox:stable".to_string()),
                ..Default::default()
            }),
            None,
            None,
        );
        while stream.next().await.is_some() {}
    }
}

async fn cleanup(client: &Docker, containers: &[String], volumes: &[String]) {
    for id in containers {
        let _ = client
            .remove_container(
                id,
                Some(RemoveContainerOptions {
                    force: true,
                    ..Default::default()
                }),
            )
            .await;
    }
    for v in volumes {
        let _ = client.remove_volume(v, None::<RemoveVolumeOptions>).await;
    }
}

fn uid() -> String {
    uuid::Uuid::new_v4().to_string()
}

async fn run_cleanup_test(label: &str, task: Task) {
    let client = Docker::connect_with_local_defaults().expect("docker client");
    ensure_busybox(&client).await;

    let before = DockerSnapshot::capture(&client).await;

    let runtime = DockerRuntime::default_runtime().await.expect("runtime");
    let _ = Runtime::run(&runtime, &task).await;

    tokio::time::sleep(std::time::Duration::from_millis(1000)).await;

    let after = DockerSnapshot::capture(&client).await;
    let (leaked_c, leaked_v) = before.leaked_from(&after);

    if !leaked_c.is_empty() || !leaked_v.is_empty() {
        cleanup(&client, &leaked_c, &leaked_v).await;
    }

    assert!(
        leaked_c.is_empty(),
        "{label}: {} containers leaked: {:?}",
        leaked_c.len(),
        leaked_c
    );
    assert!(
        leaked_v.is_empty(),
        "{label}: {} volumes leaked: {:?}",
        leaked_v.len(),
        leaked_v
    );
}

#[tokio::test]
#[ignore = "requires Docker daemon"]
async fn test_basic_task_no_leaks() {
    run_cleanup_test("basic task", make_echo_task(&uid())).await;
}

#[tokio::test]
#[ignore = "requires Docker daemon"]
async fn test_sidecar_task_no_leaks() {
    run_cleanup_test("sidecar task", make_sidecar_task(&uid())).await;
}

#[tokio::test]
#[ignore = "requires Docker daemon"]
async fn test_multi_sidecar_3_no_leaks() {
    run_cleanup_test("multi-sidecar(3)", make_multi_sidecar_task(&uid(), 3)).await;
}

#[tokio::test]
#[ignore = "requires Docker daemon"]
async fn test_failing_task_no_leaks() {
    run_cleanup_test("failing task", make_failing_task(&uid())).await;
}

#[tokio::test]
#[ignore = "requires Docker daemon"]
async fn test_concurrent_5_tasks_no_leaks() {
    let client = Docker::connect_with_local_defaults().expect("docker client");
    ensure_busybox(&client).await;

    let before = DockerSnapshot::capture(&client).await;

    let mut handles = Vec::new();
    for _ in 0..5 {
        let rt = DockerRuntime::default_runtime().await.expect("runtime");
        let task = make_echo_task(&uid());
        handles.push(tokio::spawn(async move { Runtime::run(&rt, &task).await }));
    }

    for handle in handles {
        let result = handle.await.expect("join");
        assert!(result.is_ok(), "task failed: {:?}", result.err());
    }

    tokio::time::sleep(std::time::Duration::from_secs(1)).await;

    let after = DockerSnapshot::capture(&client).await;
    let (leaked_c, leaked_v) = before.leaked_from(&after);

    if !leaked_c.is_empty() || !leaked_v.is_empty() {
        cleanup(&client, &leaked_c, &leaked_v).await;
    }

    assert!(
        leaked_c.is_empty(),
        "concurrent: {} containers leaked: {:?}",
        leaked_c.len(),
        leaked_c
    );
    assert!(
        leaked_v.is_empty(),
        "concurrent: {} volumes leaked: {:?}",
        leaked_v.len(),
        leaked_v
    );
}
