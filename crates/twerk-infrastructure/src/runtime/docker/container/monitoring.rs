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
use twerk_core::id::TaskId;
use twerk_core::task::TaskLogPart;

/// Builds the `LogsOptions` for streaming all container logs.
fn build_log_options() -> LogsOptions {
    LogsOptions {
        stdout: true,
        stderr: true,
        follow: true,
        tail: "all".to_string(),
        ..Default::default()
    }
}

/// Extracts a non-empty message from a log output, returning `None` for empty or non-stdout/stderr output.
fn extract_log_message(result: bollard::container::LogOutput) -> Option<String> {
    let (bollard::container::LogOutput::StdOut { message }
    | bollard::container::LogOutput::StdErr { message }) = result
    else {
        return None;
    };
    let msg = String::from_utf8_lossy(message.as_ref()).to_string();
    if msg.is_empty() {
        None
    } else {
        Some(msg)
    }
}

/// Creates a `TaskLogPart` from a message and part number.
fn make_log_part(task_id: &TaskId, part_num: i64, msg: String) -> TaskLogPart {
    TaskLogPart {
        id: None,
        number: part_num,
        task_id: Some(task_id.clone()),
        contents: Some(msg),
        created_at: None,
    }
}

/// Streams container logs to the broker.
pub async fn stream_logs(
    client: Docker,
    container_id: String,
    task_id: TaskId,
    broker: Option<Arc<dyn Broker>>,
) {
    let Some(broker) = broker else {
        return;
    };

    let options = build_log_options();
    let mut stream = client.logs(&container_id, Some(options));
    let mut part_num = 0i64;

    while let Some(result) = stream.next().await {
        let Ok(result) = result else {
            continue;
        };
        let Some(msg) = extract_log_message(result) else {
            continue;
        };
        part_num += 1;
        let part = make_log_part(&task_id, part_num, msg);
        let _ = broker.publish_task_log_part(&part).await;
    }
}

/// Reads the last N lines of container logs.
///
/// # Errors
///
/// Returns `DockerError` if reading logs fails.
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
///
/// # Errors
///
/// Returns `DockerError` if reading the output file fails.
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
