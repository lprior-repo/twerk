//! Container operations for Docker runtime.
//!
//! This module provides both the `Container` struct (used by DockerRuntime) and
//! the `Tcontainer` struct (ported from Go tcontainer.go).

use std::collections::HashMap;
use std::fs::File;
use std::io::{BufReader, Read};
use std::sync::Arc;
use bollard::query_parameters::{DownloadFromContainerOptions, LogsOptions, RemoveContainerOptions, RemoveVolumeOptions, UploadToContainerOptions, WaitContainerOptions};
use bollard::{body_full, Docker};
use futures_util::StreamExt;
use tokio::time::sleep;
use crate::runtime::docker::archive::{Archive, ArchiveError};
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
                    .unwrap_or_else(|_| String::new()),
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
                    Ok(p) if prev.is_none_or(|old| (old - p).abs() > 0.001) => {
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
        let contents = parse_tar_contents(&bytes);
        let s = contents.trim();
        if s.is_empty() { return Ok(0.0); }
        s.parse::<f64>().map_err(|_| DockerError::CopyFromContainer("invalid progress".to_string()))
    }

    async fn read_output(&self) -> Result<String, DockerError> {
        let options = DownloadFromContainerOptions { path: "/twerk/stdout".to_string() };
        let mut stream = self.client.download_from_container(&self.id, Some(options));
        match stream.next().await { Some(Ok(bytes)) => Ok(parse_tar_contents(&bytes)), Some(Err(e)) => Err(DockerError::CopyFromContainer(e.to_string())), None => Ok(String::new()) }
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

// ----------------------------------------------------------------------------
// Tcontainer - Port from Go tcontainer.go
// ----------------------------------------------------------------------------

use bollard::config::{NetworkingConfig, HostConfig};
use bollard::models::{ContainerCreateBody, EndpointSettings, Mount as BollardMount, MountTypeEnum, PortBinding};
use bollard::query_parameters::CreateContainerOptions;
use twerk_core::mount::mount_type;
use twerk_core::task::Task;
use twerk_core::uuid::new_uuid;

use crate::runtime::docker::helpers::{parse_memory_bytes, slugify};
use crate::runtime::docker::mounters::Mounter;
use crate::broker::Broker;

const TWORK_OUTPUT: &str = "TWERK_OUTPUT=/twerk/stdout";
const TWORK_PROGRESS: &str = "TWERK_PROGRESS=/twerk/progress";

/// Tcontainer is the Docker container wrapper for task execution.
/// Ported from Go tcontainer struct.
pub struct Tcontainer {
    pub id: String,
    pub client: Docker,
    pub mounter: Arc<dyn Mounter>,
    pub broker: Arc<dyn Broker>,
    pub task: Task,
    pub logger: Box<dyn std::io::Write + Send + Sync>,
    pub torkdir: twerk_core::mount::Mount,
}

/// TempArchive is a consuming builder for temporary tar archives.
/// Follows functional-rust: Data-Calc-Actions separation.
///
/// # Architecture
/// - **Data**: `TempArchive` wraps `Archive` with consuming builder pattern
/// - **Calc**: Pure file entry construction
/// - **Actions**: File I/O at boundary (new, remove)
///
/// Go parity: NewTempArchive in tcontainer.go
#[must_use]
pub struct TempArchive {
    inner: Archive,
}

impl TempArchive {
    pub fn new() -> Result<Self, ArchiveError> {
        Archive::new().map(Self::from)
    }

    pub fn write_file(mut self, name: &str, mode: u32, data: &[u8]) -> Result<Self, ArchiveError> {
        self.inner.write_file(name, mode, data)?;
        Ok(self)
    }

    pub fn reader(&mut self) -> Result<BufReader<File>, ArchiveError> {
        self.inner.reader()
    }

    pub fn remove(self) -> Result<(), ArchiveError> {
        self.inner.remove()
    }
}

impl From<Archive> for TempArchive {
    fn from(inner: Archive) -> Self {
        Self { inner }
    }
}

impl Tcontainer {
    /// Starts the container and waits for the probe to be ready.
    pub async fn start(&self) -> Result<(), DockerError> {
        tracing::debug!(container_id = %self.id, "Starting container");
        self.client.start_container(&self.id, None).await
            .map_err(|e| DockerError::ContainerStart(format!("{}: {}", self.id, e)))?;
        self.probe_container().await?;
        Ok(())
    }

    /// Removes the container and cleans up resources.
    pub async fn remove(&self) -> Result<(), DockerError> {
        tracing::debug!(container_id = %self.id, "Removing container");
        self.client.remove_container(&self.id, Some(RemoveContainerOptions { force: true, ..Default::default() })).await
            .map_err(|e| DockerError::ContainerRemove(e.to_string()))?;
        if let Some(ref source) = self.torkdir.source {
            self.client.remove_volume(source, Some(RemoveVolumeOptions { force: true })).await
                .map_err(|e| DockerError::VolumeRemove(e.to_string()))?;
        }
        self.mounter.unmount(&self.torkdir).await
            .map_err(DockerError::Unmount)?;
        Ok(())
    }

    /// Waits for the container to complete and returns the stdout.
    pub async fn wait(&self) -> Result<String, DockerError> {
        let options = WaitContainerOptions { condition: "not-running".to_string() };
        let result = self.client.wait_container(&self.id, Some(options)).next().await
            .ok_or_else(|| DockerError::ContainerWait("no wait result".to_string()))?
            .map_err(|e| DockerError::ContainerWait(e.to_string()))?;
        let status_code: i64 = result.status_code;
        if status_code != 0 {
            return Err(DockerError::NonZeroExit(
                status_code,
                self.read_logs_tail(10).await.unwrap_or_else(|_| String::new()),
            ));
        }
        let stdout = self.read_output().await?;
        tracing::debug!(status_code, task_id = ?self.task.id, "task completed");
        Ok(stdout)
    }

    async fn probe_container(&self) -> Result<(), DockerError> {
        let probe = match &self.task.probe { Some(p) => p, None => return Ok(()) };
        let port = probe.port;
        let path = probe.path.as_deref().unwrap_or("/");
        let timeout_str = probe.timeout.as_deref().unwrap_or("1m");
        let timeout = parse_go_duration(timeout_str).map_err(|e| DockerError::ProbeTimeout(format!("invalid timeout: {}", e)))?;
        let inspect = self.client.inspect_container(&self.id, None).await
            .map_err(|e| DockerError::ContainerInspect(format!("{}: {}", self.id, e)))?;
        let port_key = format!("{}/tcp", port);
        let host_port = inspect.network_settings.as_ref().and_then(|ns| ns.ports.as_ref())
            .and_then(|ports| ports.get(&port_key)).and_then(|opt| opt.as_ref())
            .and_then(|bindings| bindings.first()).and_then(|b| b.host_port.as_ref())
            .ok_or_else(|| DockerError::ProbeError(format!("no port found for {}", self.id)))?;
        let probe_url = format!("http://localhost:{}{}", host_port, path);
        let http_client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(3))
            .connect_timeout(std::time::Duration::from_secs(3))
            .build().map_err(|e| DockerError::ProbeError(format!("HTTP client: {}", e)))?;
        let deadline = tokio::time::Instant::now() + timeout;
        loop {
            if tokio::time::Instant::now() >= deadline {
                return Err(DockerError::ProbeTimeout(timeout_str.to_string()));
            }
            match http_client.get(&probe_url).send().await {
                Ok(resp) if resp.status().as_u16() == 200 => return Ok(()),
                Ok(resp) => { tracing::debug!(container_id = %self.id, status = resp.status().as_u16(), "probe non-200"); }
                Err(e) => { tracing::debug!(container_id = %self.id, error = %e, "probe failed"); }
            }
            sleep(std::time::Duration::from_secs(1)).await;
        }
    }

    async fn report_progress(self: Arc<Self>) {
        let broker = self.broker.as_ref();
        let mut tick = tokio::time::interval(std::time::Duration::from_secs(10));
        let task_id = self.task.id.clone();
        loop {
            tokio::select! {
                _ = tick.tick() => {
                    match self.read_progress().await {
                        Ok(p) => {
                            if let Some(ref tid) = task_id {
                                let twerk_task = twerk_core::task::Task { id: Some(tid.clone()), progress: p, ..Default::default() };
                                if let Err(e) = broker.publish_task_progress(&twerk_task).await {
                                    tracing::warn!(task_id = %tid, error = %e, "error publishing task progress");
                                }
                            }
                        }
                        Err(_) => break,
                    }
                }
            }
        }
    }

    async fn read_output(&self) -> Result<String, DockerError> {
        let options = DownloadFromContainerOptions { path: "/tork/stdout".to_string() };
        let mut stream = self.client.download_from_container(&self.id, Some(options));
        match stream.next().await {
            Some(Ok(bytes)) => Ok(parse_tar_contents(&bytes)),
            Some(Err(e)) => Err(DockerError::CopyFromContainer(e.to_string())),
            None => Ok(String::new()),
        }
    }

    async fn read_progress(&self) -> Result<f64, DockerError> {
        let options = DownloadFromContainerOptions { path: "/tork/progress".to_string() };
        let mut stream = self.client.download_from_container(&self.id, Some(options));
        let bytes = stream.next().await.ok_or_else(|| DockerError::CopyFromContainer("empty".to_string()))?
            .map_err(|e| DockerError::CopyFromContainer(e.to_string()))?;
        let contents = parse_tar_contents(&bytes);
        let s = contents.trim();
        if s.is_empty() { return Ok(0.0); }
        s.parse::<f64>().map_err(|_| DockerError::CopyFromContainer("invalid progress".to_string()))
    }

    async fn read_logs_tail(&self, lines: usize) -> Result<String, DockerError> {
        let options = LogsOptions { stdout: true, stderr: true, tail: lines.to_string(), ..Default::default() };
        let mut stream = self.client.logs(&self.id, Some(options));
        let mut output = String::new();
        while let Some(result) = stream.next().await {
            if let Ok(chunk) = result { output.push_str(&chunk.to_string()); } else { break; }
        }
        Ok(output)
    }

    /// Initializes the tork directory in the container.
    pub async fn init_torkdir(&self) -> Result<(), DockerError> {
        let mut ar = TempArchive::new().map_err(|e| DockerError::CopyToContainer(e.to_string()))?;
        ar = ar.write_file("stdout", 0o222, &[]).map_err(|e| DockerError::CopyToContainer(e.to_string()))?;
        ar = ar.write_file("progress", 0o222, &[]).map_err(|e| DockerError::CopyToContainer(e.to_string()))?;

        if let Some(ref run_script) = self.task.run {
            if !run_script.is_empty() {
                ar = ar.write_file("entrypoint", 0o555, run_script.as_bytes())
                    .map_err(|e| DockerError::CopyToContainer(e.to_string()))?;
            }
        }

        let mut reader = ar.reader().map_err(|e| DockerError::CopyToContainer(e.to_string()))?;
        let mut contents = Vec::new();
        Read::read_to_end(&mut reader, &mut contents)
            .map_err(|e| DockerError::CopyToContainer(e.to_string()))?;

        let options = UploadToContainerOptions { path: "/twerk/".to_string(), ..Default::default() };
        self.client.upload_to_container(&self.id, Some(options), body_full(contents.into())).await
            .map_err(|e| DockerError::CopyToContainer(e.to_string()))?;

        ar.remove().map_err(|e| DockerError::CopyToContainer(e.to_string()))?;
        Ok(())
    }

    /// Initializes the work directory in the container.
    pub async fn init_workdir(&self) -> Result<(), DockerError> {
        let files = match &self.task.files {
            Some(f) => f,
            None => return Ok(()),
        };
        if files.is_empty() {
            return Ok(());
        }

        let mut ar = TempArchive::new().map_err(|e| DockerError::CopyToContainer(e.to_string()))?;
        for (name, data) in files {
            ar = ar.write_file(name, 0o444, data.as_bytes())
                .map_err(|e| DockerError::CopyToContainer(e.to_string()))?;
        }

        let mut reader = ar.reader().map_err(|e| DockerError::CopyToContainer(e.to_string()))?;
        let mut contents = Vec::new();
        Read::read_to_end(&mut reader, &mut contents)
            .map_err(|e| DockerError::CopyToContainer(e.to_string()))?;

        let workdir = self.task.workdir.as_deref().unwrap_or("/workspace");
        let options = UploadToContainerOptions { path: workdir.to_string(), ..Default::default() };
        self.client.upload_to_container(&self.id, Some(options), body_full(contents.into())).await
            .map_err(|e| DockerError::CopyToContainer(e.to_string()))?;

        ar.remove().map_err(|e| DockerError::CopyToContainer(e.to_string()))?;
        Ok(())
    }
}

fn parse_cpus(limits: Option<&twerk_core::task::TaskLimits>) -> Result<Option<i64>, DockerError> {
    let cpus = match limits.and_then(|l| l.cpus.as_ref()) {
        Some(cpus) if !cpus.is_empty() => {
            let value: f64 = cpus.parse()
                .map_err(|_| DockerError::InvalidCpus(cpus.clone()))?;
            Some((value * 1e9) as i64)
        }
        _ => None,
    };
    Ok(cpus)
}

fn parse_memory(limits: Option<&twerk_core::task::TaskLimits>) -> Result<Option<i64>, DockerError> {
    let memory = match limits.and_then(|l| l.memory.as_ref()) {
        Some(mem) if !mem.is_empty() => {
            Some(parse_memory_bytes(mem).map_err(DockerError::InvalidMemory)?)
        }
        _ => None,
    };
    Ok(memory)
}

fn parse_gpu_options(gpu_str: &str) -> Result<Vec<bollard::models::DeviceRequest>, DockerError> {
    use bollard::models::DeviceRequest;

    let mut count: Option<i64> = None;
    let mut driver: Option<String> = None;
    let mut capabilities: Vec<String> = Vec::new();
    let mut device_ids: Vec<String> = Vec::new();

    for part in gpu_str.split(',') {
        let part = part.trim();
        if let Some((key, value)) = part.split_once('=') {
            match key.trim() {
                "count" => {
                    count = if value.trim() == "all" {
                        Some(-1)
                    } else {
                        Some(value.trim().parse::<i64>()
                            .map_err(|_| DockerError::InvalidGpuOptions(format!("invalid count: {}", value)))?)
                    };
                }
                "driver" => {
                    driver = Some(value.trim().to_string());
                }
                "capabilities" => {
                    for cap in value.split(';') {
                        capabilities.push(cap.trim().to_string());
                    }
                }
                "device" => {
                    for dev in value.split(';') {
                        device_ids.push(dev.trim().to_string());
                    }
                }
                other => {
                    return Err(DockerError::InvalidGpuOptions(format!("unknown GPU option: {}", other)));
                }
            }
        }
    }

    if capabilities.is_empty() {
        capabilities.push("gpu".to_string());
    }

    Ok(vec![DeviceRequest {
        count,
        driver,
        capabilities: Some(vec![capabilities]),
        device_ids: if device_ids.is_empty() { None } else { Some(device_ids) },
        options: None,
    }])
}

/// Creates a task container for the given task.
/// Go parity: createTaskContainer in tcontainer.go
pub async fn create_task_container(
    client: &Docker,
    mounter: Arc<dyn Mounter>,
    broker: Arc<dyn Broker>,
    task: &Task,
    logger: Box<dyn std::io::Write + Send + Sync>,
) -> Result<Tcontainer, DockerError> {
    if task.id.as_ref().is_none_or(|id| id.is_empty()) {
        return Err(DockerError::TaskIdRequired);
    }

    let image = task.image.as_ref().ok_or_else(|| DockerError::ImageRequired)?;

    crate::runtime::docker::pull::pull_image(
        client,
        &crate::runtime::docker::config::DockerConfig::default(),
        &Default::default(),
        image,
        task.registry.as_ref(),
    ).await
        .map_err(|e| DockerError::ImagePull(format!("{}: {}", image, e)))?;

    let mut env: Vec<String> = if let Some(ref env_map) = task.env {
        env_map.iter()
            .map(|(k, v)| format!("{}={}", k, v))
            .collect()
    } else {
        Vec::new()
    };
    env.push(TWORK_OUTPUT.to_string());
    env.push(TWORK_PROGRESS.to_string());

    let mut mounts: Vec<BollardMount> = Vec::new();
    if let Some(ref task_mounts) = task.mounts {
        for mnt in task_mounts {
            let mount_type_str = mnt.mount_type.as_deref();
            let mt = match mount_type_str {
                Some(mount_type::VOLUME) => {
                    if mnt.target.as_ref().is_none_or(|t| t.is_empty()) {
                        return Err(DockerError::VolumeTargetRequired);
                    }
                    MountTypeEnum::VOLUME
                }
                Some(mount_type::BIND) => {
                    if mnt.target.as_ref().is_none_or(|t| t.is_empty()) {
                        return Err(DockerError::BindTargetRequired);
                    }
                    if mnt.source.as_ref().is_none_or(|s| s.is_empty()) {
                        return Err(DockerError::BindSourceRequired);
                    }
                    MountTypeEnum::BIND
                }
                Some(mount_type::TMPFS) => MountTypeEnum::TMPFS,
                Some(other) => return Err(DockerError::UnknownMountType(other.to_string())),
                None => return Err(DockerError::UnknownMountType("none".to_string())),
            };

            tracing::debug!(source = ?mnt.source, target = ?mnt.target, "Mounting");
            mounts.push(BollardMount {
                target: mnt.target.clone(),
                source: mnt.source.clone(),
                typ: Some(mt),
                ..Default::default()
            });
        }
    }

    let torkdir_id = new_uuid();
    let torkdir_volume_name = torkdir_id.clone();

    client.create_volume(bollard::models::VolumeCreateRequest {
        name: Some(torkdir_volume_name.clone()),
        driver: Some("local".to_string()),
        ..Default::default()
    }).await
        .map_err(|e| DockerError::VolumeCreate(e.to_string()))?;

    let torkdir = twerk_core::mount::Mount {
        id: Some(torkdir_id.clone()),
        mount_type: Some(mount_type::VOLUME.to_string()),
        target: Some("/twerk".to_string()),
        source: Some(torkdir_volume_name.clone()),
        opts: None,
    };

    mounts.push(BollardMount {
        typ: Some(MountTypeEnum::VOLUME),
        source: Some(torkdir_volume_name.clone()),
        target: Some("/twerk".to_string()),
        ..Default::default()
    });

    let nano_cpus = parse_cpus(task.limits.as_ref())?;
    let memory = parse_memory(task.limits.as_ref())?;

    let device_requests = task.gpus.as_ref()
        .map(|gpu_str| parse_gpu_options(gpu_str))
        .transpose()?;

    let cmd: Vec<String> = if task.cmd.as_ref().is_none_or(|c| c.is_empty()) {
        vec!["/twerk/entrypoint".to_string()]
    } else {
        task.cmd.clone().unwrap_or_default()
    };

    let entrypoint: Vec<String> = if task.entrypoint.as_ref().is_none_or(|e| e.is_empty()) && task.run.is_some() {
        vec!["sh".to_string(), "-c".to_string()]
    } else {
        task.entrypoint.clone().unwrap_or_default()
    };

    let mut exposed_ports: HashMap<String, Option<Vec<PortBinding>>> = HashMap::new();
    if let Some(ref probe) = task.probe {
        let port_key = format!("{}/tcp", probe.port);
        exposed_ports.insert(port_key.clone(), Some(vec![PortBinding {
            host_ip: Some("127.0.0.1".to_string()),
            host_port: Some("0".to_string()),
        }]));
    }

    let host_config = HostConfig {
        mounts: Some(mounts),
        nano_cpus,
        memory,
        privileged: Some(false),
        device_requests,
        port_bindings: if exposed_ports.is_empty() { None } else { Some(exposed_ports) },
        ..Default::default()
    };

    let networking_config = if task.networks.as_ref().is_none_or(|n| n.is_empty()) {
        None
    } else {
        let mut endpoints = HashMap::new();
        if let Some(ref networks) = task.networks {
            let alias = slugify(task.name.as_deref().unwrap_or("unknown"));
            for nw in networks {
                endpoints.insert(nw.clone(), EndpointSettings {
                    aliases: Some(vec![alias.clone()]),
                    ..Default::default()
                });
            }
        }
        Some(NetworkingConfig {
            endpoints_config: Some(endpoints),
        })
    };

    let container_config = ContainerCreateBody {
        image: task.image.clone(),
        env: Some(env),
        cmd: Some(cmd),
        entrypoint: if entrypoint.is_empty() { None } else { Some(entrypoint) },
        exposed_ports: if task.probe.is_some() {
            Some(vec![format!("{}/tcp", task.probe.as_ref().unwrap().port)].into_iter().collect())
        } else {
            None
        },
        host_config: Some(host_config),
        networking_config,
        ..Default::default()
    };

    let create_ctx = tokio::time::timeout(
        std::time::Duration::from_secs(30),
        client.create_container(
            Some(CreateContainerOptions { name: None, platform: String::new() }),
            container_config,
        ),
    ).await
        .map_err(|_| DockerError::ContainerCreate("creation timed out".to_string()))?
        .map_err(|e| {
            tracing::error!(image = %image, error = %e, "Error creating container");
            DockerError::ContainerCreate(e.to_string())
        })?;

    let container_id = create_ctx.id;

    let tc = Tcontainer {
        id: container_id.clone(),
        client: client.clone(),
        mounter: mounter.clone(),
        broker,
        task: task.clone(),
        logger,
        torkdir: torkdir.clone(),
    };

    if let Err(e) = tc.init_torkdir().await {
        let _ = client.remove_container(&container_id, Some(bollard::query_parameters::RemoveContainerOptions { force: true, ..Default::default() })).await;
        let _ = client.remove_volume(&torkdir_volume_name, Some(bollard::query_parameters::RemoveVolumeOptions { force: true })).await;
        return Err(DockerError::CopyToContainer(format!("error initializing torkdir: {}", e)));
    }

    let workdir_has_files = !task.files.as_ref().is_none_or(|f| f.is_empty());
    let effective_workdir: Option<String> = if task.workdir.is_some() {
        task.workdir.clone()
    } else if workdir_has_files {
        Some("/workspace".to_string())
    } else {
        None
    };

    if let Some(ref workdir) = effective_workdir {
        if let Err(e) = init_workdir_for_container(&tc, workdir).await {
            let _ = client.remove_container(&container_id, Some(bollard::query_parameters::RemoveContainerOptions { force: true, ..Default::default() })).await;
            let _ = client.remove_volume(&torkdir_volume_name, Some(bollard::query_parameters::RemoveVolumeOptions { force: true })).await;
            return Err(DockerError::CopyToContainer(format!("error initializing workdir: {}", e)));
        }
    }

    tracing::debug!(container_id = %container_id, "Created container");

    Ok(tc)
}

async fn init_workdir_for_container(tc: &Tcontainer, workdir: &str) -> Result<(), DockerError> {
    let files = match &tc.task.files {
        Some(f) => f,
        None => return Ok(()),
    };
    if files.is_empty() {
        return Ok(());
    }

    let mut archive = Archive::new().map_err(|e| DockerError::CopyToContainer(e.to_string()))?;
    for (name, data) in files {
        archive.write_file(name, 0o444, data.as_bytes())
            .map_err(|e| DockerError::CopyToContainer(e.to_string()))?;
    }

    archive.finish().map_err(|e| DockerError::CopyToContainer(e.to_string()))?;

    let mut reader = archive.reader().map_err(|e| DockerError::CopyToContainer(e.to_string()))?;
    let mut contents = Vec::new();
    std::io::Read::read_to_end(&mut reader, &mut contents)
        .map_err(|e| DockerError::CopyToContainer(e.to_string()))?;

    let options = UploadToContainerOptions { path: workdir.to_string(), ..Default::default() };
    tc.client.upload_to_container(&tc.id, Some(options), body_full(contents.into())).await
        .map_err(|e| DockerError::CopyToContainer(e.to_string()))?;

    archive.remove().map_err(|e| DockerError::CopyToContainer(e.to_string()))?;
    Ok(())
}
