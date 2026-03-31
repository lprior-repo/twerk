//! Container monitoring operations.
//!
//! Provides log streaming and progress reporting for running containers.

use crate::broker::Broker;
use crate::runtime::docker::error::DockerError;
use crate::runtime::docker::helpers::parse_tar_contents;
use bollard::query_parameters::{DownloadFromContainerOptions, LogsOptions};
use bollard::Docker;
use futures_util::StreamExt;
use std::sync::Arc;
use std::time::Duration;
use twerk_core::id::TaskId;
use twerk_core::task::TaskLogPart;

/// Streams container logs to the broker.
pub async fn stream_logs(
    client: Docker,
    container_id: String,
    task_id: TaskId,
    broker: Option<Arc<dyn Broker>>,
) {
    let Some(broker) = broker else { return };

    let options = LogsOptions {
        stdout: true,
        stderr: true,
        follow: true,
        tail: "all".to_string(),
        ..Default::default()
    };

    let mut stream = client.logs(&container_id, Some(options));
    let mut part_num = 0i64;

    while let Some(result) = stream.next().await {
        match result {
            Ok(bollard::container::LogOutput::StdOut { message })
            | Ok(bollard::container::LogOutput::StdErr { message }) => {
                let msg = String::from_utf8_lossy(message.as_ref()).to_string();
                if !msg.is_empty() {
                    part_num += 1;
                    let _ = broker
                        .publish_task_log_part(&TaskLogPart {
                            id: None,
                            number: part_num,
                            task_id: Some(task_id.clone()),
                            contents: Some(msg),
                            created_at: None,
                        })
                        .await;
                }
            }
            _ => {}
        }
    }
}

/// Reports container progress periodically to the broker.
pub async fn report_progress(
    client: Docker,
    container_id: String,
    task_id: TaskId,
    broker: Option<Arc<dyn Broker>>,
) {
    let Some(broker) = broker else { return };

    let mut tick = tokio::time::interval(Duration::from_secs(10));
    let mut prev: Option<f64> = None;

    loop {
        tokio::select! {
            _ = tick.tick() => {
                match read_progress_value(&client, &container_id).await {
                    Ok(p) if prev.is_none_or(|old| (old - p).abs() > 0.001) => {
                        prev = Some(p);
                        let task = twerk_core::task::Task {
                            id: Some(task_id.clone()),
                            progress: p,
                            ..Default::default()
                        };
                        if let Err(e) = broker.publish_task_progress(&task).await {
                            tracing::warn!(task_id = %task_id, error = %e, "error publishing task progress");
                        }
                    }
                    Err(_) => break,
                    _ => {}
                }
            }
        }
    }
}

/// Reads the progress value from the container's /twerk/progress file.
async fn read_progress_value(client: &Docker, cid: &str) -> Result<f64, DockerError> {
    let options = DownloadFromContainerOptions {
        path: "/twerk/progress".to_string(),
    };

    let mut stream = client.download_from_container(cid, Some(options));

    let bytes = stream
        .next()
        .await
        .ok_or_else(|| DockerError::CopyFromContainer("empty".to_string()))?
        .map_err(|e| DockerError::CopyFromContainer(e.to_string()))?;

    let contents = parse_tar_contents(&bytes);
    let s = contents.trim();

    if s.is_empty() {
        return Ok(0.0);
    }

    s.parse::<f64>()
        .map_err(|_| DockerError::CopyFromContainer("invalid progress".to_string()))
}

/// Reads the last N lines of container logs.
pub async fn read_logs_tail(
    client: &Docker,
    container_id: &str,
    lines: usize,
) -> Result<String, DockerError> {
    let options = LogsOptions {
        stdout: true,
        stderr: true,
        tail: lines.to_string(),
        ..Default::default()
    };

    let mut stream = client.logs(container_id, Some(options));
    let mut output = String::new();

    while let Some(result) = stream.next().await {
        if let Ok(chunk) = result {
            output.push_str(&chunk.to_string());
        } else {
            break;
        }
    }

    Ok(output)
}

/// Reads the stdout file from a container's runtime directory.
pub async fn read_output_file(
    client: &Docker,
    container_id: &str,
    runtime_path: &str,
) -> Result<String, DockerError> {
    let options = DownloadFromContainerOptions {
        path: runtime_path.to_string(),
    };

    let mut stream = client.download_from_container(container_id, Some(options));

    match stream.next().await {
        Some(Ok(bytes)) => Ok(parse_tar_contents(&bytes)),
        Some(Err(e)) => Err(DockerError::CopyFromContainer(e.to_string())),
        None => Ok(String::new()),
    }
}
