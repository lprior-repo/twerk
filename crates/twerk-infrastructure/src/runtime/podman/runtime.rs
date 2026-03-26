//! Podman runtime implementation.

use std::collections::HashMap;
use std::process::Stdio;
use std::sync::Arc;
use std::time::{Duration, Instant};

use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::Command;
use tokio::sync::{mpsc, oneshot, RwLock};
use tracing::{debug, error, info, warn};

use super::config::{Broker, Mounter, PodmanConfig, PullRequest, RegistryCredentials};
use super::domain::{Mount, MountType, Probe, Registry, Task, TaskLimits};
use super::state::PodmanError;
use super::slug::make as slug_make;

pub const DEFAULT_WORKDIR: &str = "/twerk/workdir";
pub const HOST_NETWORK_NAME: &str = "host";
pub const PROGRESS_POLL_INTERVAL: Duration = Duration::from_secs(10);
pub const DEFAULT_IMAGE_TTL: Duration = Duration::from_secs(72 * 3600);
pub const PRUNE_INTERVAL: Duration = Duration::from_secs(3600);
pub const CREATE_TIMEOUT: Duration = Duration::from_secs(30);

pub struct PodmanRuntime {
    broker: Option<Box<dyn Broker + Send + Sync>>,
    pullq: mpsc::Sender<PullRequest>,
    images: Arc<RwLock<HashMap<String, Instant>>>,
    tasks: Arc<RwLock<HashMap<String, String>>>,
    active_tasks: Arc<std::sync::atomic::AtomicU64>,
    mounter: Box<dyn Mounter + Send + Sync>,
    privileged: bool,
    host_network: bool,
    image_verify: bool,
    image_ttl: Duration,
}

impl std::fmt::Debug for PodmanRuntime {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PodmanRuntime")
            .field("broker", &"<broker>")
            .field("privileged", &self.privileged)
            .field("host_network", &self.host_network)
            .field("mounter", &"<mounter>")
            .field("image_verify", &self.image_verify)
            .field("image_ttl", &self.image_ttl)
            .finish()
    }
}

struct ContainerGuard {
    container_id: String,
    tasks: Arc<RwLock<HashMap<String, String>>>,
    disarmed: bool,
}

impl ContainerGuard {
    fn new(container_id: String, tasks: Arc<RwLock<HashMap<String, String>>>) -> Self {
        Self { container_id, tasks, disarmed: false }
    }
    fn disarm(&mut self) { self.disarmed = true; }
}

impl Drop for ContainerGuard {
    fn drop(&mut self) {
        if self.disarmed { return; }
        let cid = self.container_id.clone();
        let tasks = self.tasks.clone();
        tokio::spawn(async move {
            if let Err(e) = PodmanRuntime::stop_container_static(&cid).await {
                warn!("error stopping container {} in guard drop: {}", cid, e);
            }
            let _ = tasks.write().await.remove(&cid);
        });
    }
}

impl PodmanRuntime {
    pub fn new(config: PodmanConfig) -> Self {
        let (tx, rx) = mpsc::channel::<PullRequest>(100);
        let mounter = config.mounter.unwrap_or_else(|| Box::new(VolumeMounter::new()));
        let image_ttl = config.image_ttl.unwrap_or(DEFAULT_IMAGE_TTL);
        let images = Arc::new(RwLock::new(HashMap::new()));
        let tasks = Arc::new(RwLock::new(HashMap::new()));
        let active_tasks = Arc::new(std::sync::atomic::AtomicU64::new(0));
        Self::start_puller(rx, config.broker.clone());
        Self::start_pruner(images.clone(), active_tasks.clone(), image_ttl);
        Self {
            broker: config.broker,
            pullq: tx,
            images,
            tasks,
            active_tasks,
            mounter,
            privileged: config.privileged,
            host_network: config.host_network,
            image_verify: config.image_verify,
            image_ttl,
        }
    }

    fn start_puller(mut rx: mpsc::Receiver<PullRequest>, broker: Option<Box<dyn Broker + Send + Sync>>) {
        tokio::spawn(async move {
            while let Some(pr) = rx.recv().await {
                let image = pr.image.clone();
                let registry = pr.registry.clone();
                let result = Self::do_pull_request(&image, registry, broker.as_ref()).await;
                let _ = pr.respond_to.send(result);
            }
        });
    }

    fn start_pruner(images: Arc<RwLock<HashMap<String, Instant>>>, active_tasks: Arc<std::sync::atomic::AtomicU64>, ttl: Duration) {
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(PRUNE_INTERVAL);
            loop {
                interval.tick().await;
                if let Err(e) = Self::prune_images(&images, &active_tasks, ttl).await {
                    error!("error pruning images: {}", e);
                }
            }
        });
    }

    async fn do_pull_request(image: &str, registry: Option<RegistryCredentials>, _broker: Option<&Box<dyn Broker + Send + Sync>>) -> Result<(), PodmanError> {
        if Self::image_exists_locally(image).await {
            debug!("image {} already exists locally, skipping pull", image);
            return Ok(());
        }
        if let Some(ref creds) = registry {
            if !creds.username.is_empty() {
                Self::registry_login(image, &creds.username, &creds.password).await?;
            }
        }
        debug!("Pulling image {}", image);
        let mut cmd = Command::new("podman");
        cmd.arg("pull").arg(image);
        cmd.stdout(Stdio::piped());
        cmd.stderr(Stdio::piped());
        let output = cmd.output().await.map_err(|e| PodmanError::ImagePull(e.to_string()))?;
        if !output.status.success() {
            return Err(PodmanError::ImagePull(format!("podman pull failed for {}: {}", image, String::from_utf8_lossy(&output.stderr))));
        }
        Ok(())
    }

    async fn registry_login(image: &str, username: &str, password: &str) -> Result<(), PodmanError> {
        let registry_host = Self::extract_registry_host(image);
        debug!("Logging into registry {} for user {}", registry_host, username);
        let mut cmd = Command::new("podman");
        cmd.arg("login").arg("--username").arg(username).arg("--password-stdin").arg(&registry_host);
        cmd.stdout(Stdio::piped());
        cmd.stderr(Stdio::piped());
        cmd.stdin(Stdio::piped());
        let mut child = cmd.spawn().map_err(|e| PodmanError::RegistryLogin(e.to_string()))?;
        if let Some(ref mut stdin) = child.stdin {
            use tokio::io::AsyncWriteExt;
            if let Err(_e) = stdin.write_all(password.as_bytes()).await {
                return Err(PodmanError::RegistryLogin("failed to write password to stdin".to_string()));
            }
            if let Err(_e) = stdin.shutdown().await {
                return Err(PodmanError::RegistryLogin("failed to close stdin".to_string()));
            }
        }
        let output = child.wait_with_output().await.map_err(|e| PodmanError::RegistryLogin(e.to_string()))?;
        if !output.status.success() {
            return Err(PodmanError::RegistryLogin(format!("podman login to {} failed: {}", registry_host, String::from_utf8_lossy(&output.stderr))));
        }
        Ok(())
    }

    fn extract_registry_host(image: &str) -> String {
        match image.split_once('/') {
            Some((host, _rest)) if host.contains('.') || host.contains(':') => host.to_string(),
            _ => "docker.io".to_string(),
        }
    }

    async fn image_exists_locally(image: &str) -> bool {
        let output = Command::new("podman").arg("inspect").arg(image).stdout(Stdio::null()).stderr(Stdio::null()).output().await;
        output.map_or(false, |out| out.status.success())
    }

    async fn verify_image(image: &str) -> Result<(), PodmanError> {
        info!("verifying image {}", image);
        let mut create_cmd = Command::new("podman");
        create_cmd.arg("create").arg(image).arg("true");
        create_cmd.stdout(Stdio::piped());
        create_cmd.stderr(Stdio::piped());
        let create_output = create_cmd.output().await.map_err(|e| PodmanError::ImageVerification(e.to_string()))?;
        if !create_output.status.success() {
            return Err(PodmanError::ImageVerification(format!("image {} failed verification: {}", image, String::from_utf8_lossy(&create_output.stderr))));
        }
        let container_id = String::from_utf8_lossy(&create_output.stdout).trim().to_string();
        if container_id.is_empty() {
            return Err(PodmanError::ImageVerification("empty container ID during verification".to_string()));
        }
        let mut rm_cmd = Command::new("podman");
        rm_cmd.arg("rm").arg("-f").arg(&container_id);
        let _ = rm_cmd.output().await;
        info!("image {} verified successfully", image);
        Ok(())
    }

    async fn prune_images(images: &Arc<RwLock<HashMap<String, Instant>>>, active_tasks: &Arc<std::sync::atomic::AtomicU64>, ttl: Duration) -> Result<(), anyhow::Error> {
        if active_tasks.load(std::sync::atomic::Ordering::SeqCst) > 0 {
            return Ok(());
        }
        let images_guard = images.read().await;
        let stale: Vec<String> = images_guard.iter().filter(|(_img, last_used)| last_used.elapsed() > ttl).map(|(img, _)| img.clone()).collect();
        drop(images_guard);
        for image in &stale {
            let mut cmd = Command::new("podman");
            cmd.arg("image").arg("rm").arg(image);
            cmd.stdout(Stdio::null());
            cmd.stderr(Stdio::null());
            if let Ok(output) = cmd.output().await {
                if output.status.success() {
                    debug!("pruned image {}", image);
                    images.write().await.remove(image);
                }
            }
        }
        Ok(())
    }

    pub async fn run(&self, task: &mut Task) -> Result<(), PodmanError> {
        if task.id.is_empty() { return Err(PodmanError::TaskIdRequired); }
        if task.image.is_empty() { return Err(PodmanError::ImageRequired); }
        if task.name.as_ref().is_none_or(|n| n.is_empty()) { return Err(PodmanError::NameRequired); }
        if !task.sidecars.is_empty() { return Err(PodmanError::SidecarsNotSupported); }
        let mut mounted_mounts: Vec<Mount> = Vec::new();
        for mut mount in task.mounts.clone() {
            if let Err(e) = self.mounter.mount(&mut mount) {
                error!("error mounting: {}", e);
                return Err(PodmanError::WorkdirCreation(e.to_string()));
            }
            mounted_mounts.push(mount);
        }
        let mounter = &self.mounter;
        let result = self.run_inner(task, &mounted_mounts).await;
        for mount in &mounted_mounts {
            if let Err(e) = mounter.unmount(mount) {
                error!("error unmounting volume {}: {}", mount.target, e);
            }
        }
        result
    }

    async fn run_inner(&self, task: &mut Task, mounts: &[Mount]) -> Result<(), PodmanError> {
        let task_mounts = mounts.to_vec();
        task.mounts = task_mounts.clone();
        for pre in task.pre.iter_mut() {
            pre.id = uuid::Uuid::new_v4().to_string();
            pre.mounts = task_mounts.clone();
            pre.networks = task.networks.clone();
            pre.limits = task.limits.clone();
            self.do_run(pre).await?;
        }
        self.do_run(task).await?;
        for post in task.post.iter_mut() {
            post.id = uuid::Uuid::new_v4().to_string();
            post.mounts = task_mounts.clone();
            post.networks = task.networks.clone();
            post.limits = task.limits.clone();
            self.do_run(post).await?;
        }
        Ok(())
    }

    async fn do_run(&self, task: &mut Task) -> Result<(), PodmanError> {
        self.active_tasks.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        let result = self.do_run_inner(task).await;
        self.active_tasks.fetch_sub(1, std::sync::atomic::Ordering::SeqCst);
        result
    }

    async fn do_run_inner(&self, task: &mut Task) -> Result<(), PodmanError> {
        let workdir = std::env::temp_dir().join("twerk").join(&task.id);
        tokio::fs::create_dir_all(&workdir).await.map_err(|e| PodmanError::WorkdirCreation(e.to_string()))?;
        let output_file = workdir.join("output");
        let progress_file = workdir.join("progress");
        tokio::fs::File::create(&output_file).await.map_err(|e| PodmanError::FileWrite(e.to_string()))?;
        tokio::fs::set_permissions(&output_file, std::fs::Permissions::from_mode(0o777)).await.map_err(|e| PodmanError::FileWrite(e.to_string()))?;
        tokio::fs::File::create(&progress_file).await.map_err(|e| PodmanError::FileWrite(e.to_string()))?;
        tokio::fs::set_permissions(&progress_file, std::fs::Permissions::from_mode(0o777)).await.map_err(|e| PodmanError::FileWrite(e.to_string()))?;
        let entrypoint_path = workdir.join("entrypoint.sh");
        let run_script = if !task.r#run.is_empty() { task.r#run.clone() } else { task.cmd.join(" ") };
        tokio::fs::write(&entrypoint_path, &run_script).await.map_err(|e| PodmanError::FileWrite(e.to_string()))?;
        tokio::fs::set_permissions(&entrypoint_path, std::fs::Permissions::from_mode(0o755)).await.map_err(|e| PodmanError::FileWrite(e.to_string()))?;
        let registry = task.registry.as_ref().and_then(|r| {
            if r.username.is_empty() { None } else { Some(RegistryCredentials { username: r.username.clone(), password: r.password.clone() }) }
        });
        self.image_pull(&task.image, registry).await?;
        if self.image_verify {
            if let Err(e) = Self::verify_image(&task.image).await {
                error!("image {} is invalid or corrupted: {}", task.image, e);
                let mut rm_cmd = Command::new("podman");
                rm_cmd.arg("image").arg("rm").arg("-f").arg(&task.image);
                let _ = rm_cmd.output().await;
                return Err(e);
            }
        }
        let entrypoint = if task.entrypoint.is_empty() { vec!["sh".to_string()] } else { task.entrypoint.clone() };
        let mut create_cmd = Command::new("podman");
        create_cmd.arg("create").arg("-v").arg(format!("{}:/twerk", workdir.display())).arg("--entrypoint").arg(&entrypoint[0]);
        let env_vars: Vec<String> = task.env.iter().map(|(k, v)| format!("{}={}", k, v)).chain(["TWERK_OUTPUT=/twerk/output".to_string(), "TWERK_PROGRESS=/twerk/progress".to_string()]).collect();
        for env in &env_vars { create_cmd.arg("-e").arg(env); }
        for network in &task.networks {
            if network == HOST_NETWORK_NAME {
                if !self.host_network { return Err(PodmanError::HostNetworkingDisabled); }
                create_cmd.arg("--network").arg(network);
            } else {
                let alias = slug_make(&task.name.clone().unwrap_or_default());
                create_cmd.arg("--network").arg(network).arg("--network-alias").arg(alias);
            }
        }
        for mount in &task.mounts {
            match mount.mount_type {
                MountType::Volume | MountType::Bind => {
                    let vol_spec = if let Some(ref opts) = mount.opts {
                        if opts.is_empty() { format!("{}:{}", mount.source, mount.target) } else {
                            let opt_str: String = opts.iter().map(|(k, v)| format!("{}={}", k, v)).collect::<Vec<_>>().join(",");
                            format!("{}:{}:{}", mount.source, mount.target, opt_str)
                        }
                    } else { format!("{}:{}", mount.source, mount.target) };
                    create_cmd.arg("-v").arg(vol_spec);
                }
                MountType::Tmpfs => {
                    let tmpfs_spec = mount.opts.as_ref().map(|opts| opts.iter().map(|(k, v)| format!("{}={}", k, v)).collect::<Vec<_>>().join(",")).unwrap_or_default();
                    if tmpfs_spec.is_empty() { create_cmd.arg("--tmpfs").arg(&mount.target); } else { create_cmd.arg("--tmpfs").arg(format!("{}:{}", mount.target, tmpfs_spec)); }
                }
            }
        }
        if let Some(ref limits) = task.limits {
            if !limits.cpus.is_empty() { create_cmd.arg("--cpus").arg(limits.cpus.clone()); }
            if !limits.memory.is_empty() { create_cmd.arg("--memory").arg(limits.memory.clone()); }
        }
        if let Some(ref gpus) = task.gpus { if !gpus.is_empty() { create_cmd.arg("--gpus").arg(gpus); } }
        if let Some(ref probe) = task.probe {
            let port_str = probe.port.to_string();
            create_cmd.arg("--expose").arg(format!("{}/tcp", port_str)).arg("-p").arg(format!("127.0.0.1:0:{}/tcp", port_str));
        }
        let effective_workdir = if let Some(ref wd) = task.workdir { wd.clone() } else if !task.files.is_empty() { DEFAULT_WORKDIR.to_string() } else { String::new() };
        if !effective_workdir.is_empty() { create_cmd.arg("-w").arg(&effective_workdir); }
        if !task.files.is_empty() {
            let files_dir = workdir.join("workdir");
            tokio::fs::create_dir_all(&files_dir).await.map_err(|e| PodmanError::FileWrite(e.to_string()))?;
            for (filename, contents) in &task.files {
                let file_path = files_dir.join(filename);
                if let Some(parent) = file_path.parent() { tokio::fs::create_dir_all(parent).await.map_err(|e| PodmanError::FileWrite(e.to_string()))?; }
                tokio::fs::write(&file_path, contents).await.map_err(|e| PodmanError::FileWrite(e.to_string()))?;
            }
        }
        if self.privileged { create_cmd.arg("--privileged"); }
        create_cmd.arg(&task.image);
        for arg in entrypoint.iter().skip(1) { create_cmd.arg(arg); }
        create_cmd.arg("/twerk/entrypoint.sh");
        create_cmd.stdout(Stdio::piped());
        create_cmd.stderr(Stdio::piped());
        let create_output = tokio::time::timeout(CREATE_TIMEOUT, create_cmd.output()).await.map_err(|_| PodmanError::ContainerCreation("create timed out after 30 seconds".to_string()))?.map_err(|e| PodmanError::ContainerCreation(e.to_string()))?;
        if !create_output.status.success() { return Err(PodmanError::ContainerCreation(String::from_utf8_lossy(&create_output.stderr).to_string())); }
        let container_id = String::from_utf8_lossy(&create_output.stdout).trim().to_string();
        if container_id.is_empty() { return Err(PodmanError::ContainerCreation("empty container ID".to_string())); }
        debug!("created container {}", container_id);
        self.tasks.write().await.insert(task.id.clone(), container_id.clone());
        let mut guard = ContainerGuard::new(container_id.clone(), self.tasks.clone());
        let mut start_cmd = Command::new("podman");
        start_cmd.arg("start").arg(&container_id);
        start_cmd.stdout(Stdio::piped());
        start_cmd.stderr(Stdio::piped());
        let start_output = start_cmd.output().await.map_err(|e| PodmanError::ContainerStart(e.to_string()))?;
        if !start_output.status.success() { return Err(PodmanError::ContainerStart(String::from_utf8_lossy(&start_output.stderr).to_string())); }
        if let Some(ref probe) = task.probe {
            let host_port = Self::get_host_port(&container_id, probe.port).await?;
            self.probe_container(&host_port, probe).await?;
        }
        let logs_task_id = task.id.clone();
        let logs_broker = self.broker.clone();
        let mut logs_cmd = Command::new("podman");
        logs_cmd.arg("logs").arg("--follow").arg(&container_id);
        logs_cmd.stdout(Stdio::piped());
        logs_cmd.stderr(Stdio::piped());
        let mut child = logs_cmd.spawn().map_err(|e| PodmanError::LogsRead(e.to_string()))?;
        if let Some(stdout) = child.stdout.take() {
            let broker_clone = logs_broker.clone();
            let tid = logs_task_id.clone();
            tokio::spawn(async move {
                let mut reader = BufReader::new(stdout).lines();
                while let Ok(Some(line)) = reader.next_line().await {
                    debug!("[podman:stdout] {}", line);
                    if let Some(ref b) = broker_clone { b.ship_log(&tid, &line); }
                }
            });
        }
        if let Some(stderr) = child.stderr.take() {
            let broker_clone = logs_broker.clone();
            let tid = logs_task_id.clone();
            tokio::spawn(async move {
                let mut reader = BufReader::new(stderr).lines();
                while let Ok(Some(line)) = reader.next_line().await {
                    debug!("[podman:stderr] {}", line);
                    if let Some(ref b) = broker_clone { b.ship_log(&tid, &line); }
                }
            });
        }
        child.wait().await.map_err(|e| PodmanError::LogsRead(e.to_string()))?;
        let mut inspect_cmd = Command::new("podman");
        inspect_cmd.arg("inspect").arg("--format").arg("{{.State.ExitCode}}").arg(&container_id);
        let inspect_output = inspect_cmd.output().await.map_err(|e| PodmanError::ContainerInspect(e.to_string()))?;
        let exit_code = String::from_utf8_lossy(&inspect_output.stdout).trim().to_string();
        if exit_code != "0" { return Err(PodmanError::ContainerExitCode(exit_code)); }
        let output = tokio::fs::read_to_string(&output_file).await.map_err(|e| PodmanError::OutputRead(e.to_string()))?;
        task.result = output;
        if let Err(e) = Self::stop_container(&container_id).await { warn!("error stopping container {}: {}", container_id, e); }
        self.tasks.write().await.remove(&container_id);
        guard.disarm();
        if let Err(e) = tokio::fs::remove_dir_all(&workdir).await { warn!("error removing workdir {:?}: {}", workdir, e); }
        Ok(())
    }

    async fn get_host_port(container_id: &str, container_port: i64) -> Result<u16, PodmanError> {
        let port_format = format!("{{{{(index (index .NetworkSettings.Ports \"{}/tcp\") 0).HostPort}}}}", container_port);
        let mut cmd = Command::new("podman");
        cmd.arg("inspect").arg("--format").arg(&port_format).arg(container_id);
        let output = cmd.output().await.map_err(|e| PodmanError::ContainerInspect(e.to_string()))?;
        let port_str = String::from_utf8_lossy(&output.stdout).trim().to_string();
        port_str.parse::<u16>().map_err(|e| PodmanError::ProbeFailed(format!("failed to parse host port: {}", e)))
    }

    async fn probe_container(&self, host_port: &u16, probe: &Probe) -> Result<(), PodmanError> {
        let path = if probe.path.is_empty() { "/".to_string() } else { probe.path.clone() };
        let timeout_str = if probe.timeout.is_empty() { "1m".to_string() } else { probe.timeout.clone() };
        let timeout = Self::parse_duration(&timeout_str).map_err(|e| PodmanError::ProbeTimeout(format!("invalid probe timeout: {}", e)))?;
        let url = format!("http://127.0.0.1:{}{}", host_port, path);
        debug!("probing container at {}", url);
        let probe_start = tokio::time::Instant::now();
        let mut interval = tokio::time::interval(Duration::from_secs(1));
        loop {
            interval.tick().await;
            if probe_start.elapsed() > timeout { return Err(PodmanError::ProbeTimeout(timeout_str)); }
            match Self::http_get(&url).await {
                Ok(true) => { debug!("probe succeeded for {}", url); return Ok(()); }
                Ok(false) => { debug!("probe returned non-200, retrying..."); continue; }
                Err(e) => { debug!("probe failed: {}, retrying...", e); continue; }
            }
        }
    }

    async fn http_get(url: &str) -> Result<bool, String> {
        let mut cmd = Command::new("curl");
        cmd.arg("-s").arg("-o").arg("/dev/null").arg("-w").arg("%{http_code}").arg("--connect-timeout").arg("3").arg("--max-time").arg("3").arg(url);
        let output = cmd.output().await.map_err(|e| format!("curl failed: {}", e))?;
        let status_code = String::from_utf8_lossy(&output.stdout).trim().to_string();
        Ok(status_code == "200")
    }

    fn parse_cpus(cpus: &str) -> Result<f64, PodmanError> {
        let nanos: f64 = cpus.parse().map_err(|e| PodmanError::InvalidCpusLimit(format!("failed to parse '{}' as CPU limit: {}", cpus, e)))?;
        if nanos < 0.0 { return Err(PodmanError::InvalidCpusLimit("CPU limit must be non-negative".to_string())); }
        Ok(nanos)
    }

    fn parse_memory(memory: &str) -> Result<u64, PodmanError> {
        let memory = memory.trim();
        let (num_str, multiplier) = if let Some(suffix) = memory.strip_suffix("gb") { (suffix.trim_end(), 1_073_741_824u64) }
            else if let Some(suffix) = memory.strip_suffix("g") { (suffix.trim_end(), 1_073_741_824u64) }
            else if let Some(suffix) = memory.strip_suffix("mb") { (suffix.trim_end(), 1_048_576u64) }
            else if let Some(suffix) = memory.strip_suffix("m") { (suffix.trim_end(), 1_048_576u64) }
            else if let Some(suffix) = memory.strip_suffix("kb") { (suffix.trim_end(), 1024u64) }
            else if let Some(suffix) = memory.strip_suffix("k") { (suffix.trim_end(), 1024u64) }
            else if let Some(suffix) = memory.strip_suffix("b") { (suffix.trim_end(), 1u64) }
            else { (memory, 1u64) };
        let value: f64 = num_str.parse().map_err(|e| PodmanError::InvalidMemoryLimit(format!("failed to parse '{}' as memory limit: {}", memory, e)))?;
        Ok((value * multiplier as f64) as u64)
    }

    fn parse_duration(s: &str) -> Result<Duration, String> {
        let s = s.trim();
        let (num_str, suffix) = if let Some(rest) = s.strip_suffix('h') { (rest, 'h') }
            else if let Some(rest) = s.strip_suffix('m') { (rest, 'm') }
            else if let Some(rest) = s.strip_suffix('s') { (rest, 's') }
            else { return Err(format!("invalid duration: {}", s)); };
        let value: u64 = num_str.parse().map_err(|e| format!("invalid duration number '{}': {}", num_str, e))?;
        Ok(match suffix {
            'h' => Duration::from_secs(value * 3600),
            'm' => Duration::from_secs(value * 60),
            's' => Duration::from_secs(value),
            _ => return Err(format!("unknown duration suffix: {}", suffix)),
        })
    }

    async fn image_pull(&self, image: &str, registry: Option<RegistryCredentials>) -> Result<(), PodmanError> {
        let images = self.images.read().await;
        if images.contains_key(image) {
            drop(images);
            self.images.write().await.insert(image.to_string(), Instant::now());
            return Ok(());
        }
        drop(images);
        let (tx, rx) = oneshot::channel();
        self.pullq.send(PullRequest { respond_to: tx, image: image.to_string(), registry }).await.map_err(|_| PodmanError::ImagePull("channel closed".to_string()))?;
        rx.await.map_err(|_| PodmanError::ImagePull("cancelled".to_string()))??;
        self.images.write().await.insert(image.to_string(), Instant::now());
        Ok(())
    }

    async fn stop_container(container_id: &str) -> Result<(), PodmanError> { Self::stop_container_static(container_id).await }

    async fn stop_container_static(container_id: &str) -> Result<(), PodmanError> {
        debug!("Attempting to stop and remove container {}", container_id);
        let mut cmd = Command::new("podman");
        cmd.arg("rm").arg("-f").arg("-t").arg("0").arg(container_id);
        cmd.stdout(Stdio::piped());
        cmd.stderr(Stdio::piped());
        let output = cmd.output().await.map_err(|e| PodmanError::ContainerCreation(format!("failed to remove container {}: {}", container_id, e)))?;
        if !output.status.success() { return Err(PodmanError::ContainerCreation(format!("failed to stop container {}: {}", container_id, String::from_utf8_lossy(&output.stderr)))); }
        Ok(())
    }

    pub async fn health_check(&self) -> Result<(), PodmanError> {
        let mut cmd = Command::new("podman");
        cmd.arg("version");
        let output = cmd.output().await.map_err(|_| PodmanError::PodmanNotRunning)?;
        if !output.status.success() { return Err(PodmanError::PodmanNotRunning); }
        Ok(())
    }
}
