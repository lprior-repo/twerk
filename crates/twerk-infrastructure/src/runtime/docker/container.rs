//! Container operations for Docker runtime.

use std::collections::HashMap;
use std::io::Read;
use std::sync::Arc;
use bollard::query_parameters::{DownloadFromContainerOptions, LogsOptions, RemoveContainerOptions, UploadToContainerOptions, WaitContainerOptions};
use bollard::{body_full, Docker};
use futures_util::StreamExt;
use tokio::time::sleep;
use crate::runtime::docker::archive::Archive;
use crate::runtime::docker::error::DockerError;
use crate::runtime::docker::helpers::{parse_go_duration, parse_tar_contents};
use twerk_core::id::TaskId;
use twerk_core::task::Probe;

const DEFAULT_PROBE_PATH: &str = "/";
const DEFAULT_PROBE_TIMEOUT: &str = "1m";

#[allow(dead_code)]
 pub struct Container {
     pub id: String,
     pub client: Docker,
     pub twerkdir_source: Option<String>,
     pub task_id: TaskId,
     pub probe: Option<Probe>,
     pub broker: Option<Arc<dyn crate::broker::Broker>>,
}

impl Container {
    #[allow(dead_code)]
    pub async fn start(&self) -> Result<(), DockerError> {
        tracing::debug!(container_id = %self.id, "Starting container");
        self.client.start_container(&self.id, None).await
            .map_err(|e| DockerError::ContainerStart(format!("{}: {}", self.id, e)))?;
        self.probe_container().await?;
        Ok(())
    }

    #[allow(dead_code)]
    pub async fn wait(&self) -> Result<String, DockerError> {
        let options = WaitContainerOptions { condition: "not-running".to_string() };
        let result = self.client.wait_container(&self.id, Some(options)).next().await
            .ok_or_else(|| DockerError::ContainerWait("no wait result".to_string()))?
            .map_err(|e| DockerError::ContainerWait(e.to_string()))?;
        let status_code: i64 = result.status_code;
        if status_code != 0 {
            return Err(DockerError::NonZeroExit(
                status_code,
                self.read_logs_tail(10)
                    .await
                    .map_or_else(|_| String::new(), |v| v),
            ));
        }
        let stdout = self.read_output().await?;
        tracing::debug!(status_code, task_id = %self.task_id, "task completed");
        Ok(stdout)
    }

    #[allow(dead_code)]
    pub fn start_monitoring(&self) {
        let progress_client = self.client.clone();
        let progress_id = self.id.clone();
        let progress_task_id = self.task_id.clone();
        let progress_broker = self.broker.clone();
        tokio::spawn(async move { Self::report_progress(progress_client, progress_id, progress_task_id, progress_broker).await; });
        
        let log_client = self.client.clone();
        let log_id = self.id.clone();
        let log_task_id = self.task_id.clone();
        let log_broker = self.broker.clone();
        tokio::spawn(async move { Self::stream_logs(log_client, log_id, log_task_id, log_broker).await; });
    }

    async fn probe_container(&self) -> Result<(), DockerError> {
        use std::time::Duration;
        let probe = match &self.probe { Some(p) => p, None => return Ok(()) };
        let port = probe.port;
        let path = probe.path.as_deref().map_or(DEFAULT_PROBE_PATH, |p| p);
        let timeout_str = probe.timeout.as_deref().map_or(DEFAULT_PROBE_TIMEOUT, |t| t);
        let timeout = parse_go_duration(timeout_str).map_err(|e| DockerError::ProbeTimeout(format!("invalid timeout: {}", e)))?;
        let inspect = self.client.inspect_container(&self.id, None).await.map_err(|e| DockerError::ContainerInspect(format!("{}: {}", self.id, e)))?;
        let port_key = format!("{}/tcp", port);
        let host_port = inspect.network_settings.as_ref().and_then(|ns| ns.ports.as_ref())
            .and_then(|ports| ports.get(&port_key)).and_then(|opt| opt.as_ref())
            .and_then(|bindings| bindings.first()).and_then(|b| b.host_port.as_ref())
            .ok_or_else(|| DockerError::ProbeError(format!("no port found for {}", self.id)))?;
        let probe_url = format!("http://localhost:{}{}", host_port, path);
        let http_client = reqwest::Client::builder().timeout(Duration::from_secs(3)).connect_timeout(Duration::from_secs(3))
            .build().map_err(|e| DockerError::ProbeError(format!("HTTP client: {}", e)))?;
        let deadline = tokio::time::Instant::now() + timeout;
        loop {
            if tokio::time::Instant::now() >= deadline { return Err(DockerError::ProbeTimeout(timeout_str.to_string())); }
            match http_client.get(&probe_url).send().await {
                Ok(resp) if resp.status().as_u16() == 200 => return Ok(()),
                Ok(resp) => { tracing::debug!(container_id = %self.id, status = resp.status().as_u16(), "probe non-200"); }
                Err(e) => { tracing::debug!(container_id = %self.id, error = %e, "probe failed"); }
            }
            sleep(Duration::from_secs(1)).await;
        }
    }

    async fn read_logs_tail(&self, lines: usize) -> Result<String, DockerError> {
        let options = LogsOptions { stdout: true, stderr: true, tail: lines.to_string(), ..Default::default() };
        let mut stream = self.client.logs(&self.id, Some(options));
        let mut output = String::new();
        while let Some(result) = stream.next().await { if let Ok(chunk) = result { output.push_str(&chunk.to_string()); } else { break; } }
        Ok(output)
    }

    async fn stream_logs(client: Docker, container_id: String, task_id: TaskId, broker: Option<Arc<dyn crate::broker::Broker>>) {
        let Some(broker) = broker else { return };
        let options = LogsOptions { stdout: true, stderr: true, follow: true, tail: "all".to_string(), ..Default::default() };
        let mut stream = client.logs(&container_id, Some(options));
        let mut part_num = 0i64;
        while let Some(result) = stream.next().await {
            match result {
                Ok(bollard::container::LogOutput::StdOut { message }) | Ok(bollard::container::LogOutput::StdErr { message }) => {
                    let msg = String::from_utf8_lossy(message.as_ref()).to_string();
                    if !msg.is_empty() { part_num += 1; let _ = broker.publish_task_log_part(&twerk_core::task::TaskLogPart { id: None, number: part_num, task_id: Some(task_id.clone()), contents: Some(msg), created_at: None }).await; }
                }
                _ => {}
            }
        }
    }

    async fn report_progress(client: Docker, container_id: String, task_id: TaskId, broker: Option<Arc<dyn crate::broker::Broker>>) {
        use std::time::Duration;
        let Some(broker) = broker else { return };
        let mut tick = tokio::time::interval(Duration::from_secs(10));
        let mut prev: Option<f64> = None;
        loop {
            tokio::select! { _ = tick.tick() => {
                match Self::read_progress_value(&client, &container_id).await {
                    Ok(p) if prev.map_or(true, |old| (old - p).abs() > 0.001) => {
                        prev = Some(p);
                        let twerk_task = twerk_core::task::Task { id: Some(task_id.clone()), progress: p, ..Default::default() };
                        if let Err(e) = broker.publish_task_progress(&twerk_task).await { tracing::warn!(task_id = %task_id, error = %e, "error publishing task progress"); }
                    }
                    Err(_) => break,
                    _ => {}
                }
            }}
        }
    }

    async fn read_progress_value(client: &Docker, cid: &str) -> Result<f64, DockerError> {
        let options = DownloadFromContainerOptions { path: "/twerk/progress".to_string() };
        let mut stream = client.download_from_container(cid, Some(options));
        let bytes = stream.next().await.ok_or_else(|| DockerError::CopyFromContainer("empty".to_string()))?
            .map_err(|e| DockerError::CopyFromContainer(e.to_string()))?;
        let contents = parse_tar_contents(&bytes.to_vec());
        let s = contents.trim();
        if s.is_empty() { return Ok(0.0); }
        s.parse::<f64>().map_err(|_| DockerError::CopyFromContainer("invalid progress".to_string()))
    }

    async fn read_output(&self) -> Result<String, DockerError> {
        let options = DownloadFromContainerOptions { path: "/twerk/stdout".to_string() };
        let mut stream = self.client.download_from_container(&self.id, Some(options));
        match stream.next().await { Some(Ok(bytes)) => Ok(parse_tar_contents(&bytes.to_vec())), Some(Err(e)) => Err(DockerError::CopyFromContainer(e.to_string())), None => Ok(String::new()) }
    }

    pub(crate) async fn init_twerkdir(&self, run_script: Option<&str>) -> Result<(), DockerError> {
        let mut archive = Archive::new().map_err(|e| DockerError::CopyToContainer(e.to_string()))?;
        archive.write_file("stdout", 0o222, &[]).map_err(|e| DockerError::CopyToContainer(e.to_string()))?;
        archive.write_file("progress", 0o222, &[]).map_err(|e| DockerError::CopyToContainer(e.to_string()))?;
        if let Some(script) = run_script { archive.write_file("entrypoint", 0o555, script.as_bytes()).map_err(|e| DockerError::CopyToContainer(e.to_string()))?; }
        archive.finish().map_err(|e| DockerError::CopyToContainer(e.to_string()))?;
        let mut reader = archive.reader().map_err(|e| DockerError::CopyToContainer(e.to_string()))?;
        let mut contents = Vec::new();
        reader.read_to_end(&mut contents).map_err(|e| DockerError::CopyToContainer(e.to_string()))?;
        let options = UploadToContainerOptions { path: "/twerk/".to_string(), ..Default::default() };
        self.client.upload_to_container(&self.id, Some(options), body_full(contents.into())).await.map_err(|e| DockerError::CopyToContainer(e.to_string()))?;
        archive.remove().map_err(|e| DockerError::CopyToContainer(e.to_string()))?;
        Ok(())
    }

    pub(crate) async fn init_workdir(&self, files: &HashMap<String, String>, workdir: &str) -> Result<(), DockerError> {
        if files.is_empty() { return Ok(()); }
        let mut archive = Archive::new().map_err(|e| DockerError::CopyToContainer(e.to_string()))?;
        for (name, data) in files { archive.write_file(name, 0o444, data.as_bytes()).map_err(|e| DockerError::CopyToContainer(e.to_string()))?; }
        archive.finish().map_err(|e| DockerError::CopyToContainer(e.to_string()))?;
        let mut reader = archive.reader().map_err(|e| DockerError::CopyToContainer(e.to_string()))?;
        let mut contents = Vec::new();
        reader.read_to_end(&mut contents).map_err(|e| DockerError::CopyToContainer(e.to_string()))?;
        let options = UploadToContainerOptions { path: workdir.to_string(), ..Default::default() };
        self.client.upload_to_container(&self.id, Some(options), body_full(contents.into())).await.map_err(|e| DockerError::CopyToContainer(e.to_string()))?;
        archive.remove().map_err(|e| DockerError::CopyToContainer(e.to_string()))?;
        Ok(())
    }

    #[allow(dead_code)]
    pub async fn remove(&self) {
        tracing::debug!(container_id = %self.id, "Removing container");
        let _ = self.client.remove_container(&self.id, Some(RemoveContainerOptions { force: true, ..Default::default() })).await;
        if let Some(ref source) = self.twerkdir_source { 
            use bollard::query_parameters::RemoveVolumeOptions;
            let _ = self.client.remove_volume(source, Some(RemoveVolumeOptions { force: true })).await; 
        }
    }
}
