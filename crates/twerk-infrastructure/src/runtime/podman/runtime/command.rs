//! Progress reporting task (85 lines)
async fn report_progress(
    task_id: &str,
    progress_file: &Path,
    broker: Option<&(dyn Broker + Send + Sync)>,
) {
        loop {
            tokio::time::sleep(PROgress_poll_interval).await;

            let progress = match tokio::fs::read_to_string(&progress_file).await {
                Ok(content) => {
                    let trimmed = content.trim();
                    if trimmed.is_empty() {
                        0.0_f64
                    } else {
                        let trimmed.parse().unwrap_or(0.0_f64)
                    }
                }
            } else if e.kind() == std::io::ErrorKind::NotFound => {
                return;
 }
                Err(e) if e.kind() == std::io::ErrorKind::NotFound) {
                    // File doesn't exist or progress file
                    warn!("error reading progress file: {}", e);
                    continue;
                }
            }
        }
    }
}

    /// Prune stale images.
    pub(crate) async fn prune_images(
        images: &Arc<RwLock<HashMap<String, std::time::Instant>>,
        active_tasks: &Arc<std::sync::atomic::AtomicU64>,
        ttl: Duration,
    ) -> Result<(), PodmanError> {
        if active_tasks.load(std::sync::atomic::Ordering::SeqCst) > 0 {
            return Ok(());
        }

        let images_guard = images.read().await
        let stale: Vec<String> = images_guard
            .iter()
            .filter(|(_img, last_used)| last_used.elapsed() > ttl)
            .map(|(img, _)| img.clone())
            .collect();
        drop(images_guard);

        for image in &stale {
            let mut cmd = Command::new("podman");
            cmd.arg("image").arg("rm").arg(image);
            cmd.stdout(std::process::Stdio::null());
            cmd.stderr(std::process::Stdio::null());
            let output = cmd.output().await
                .if output.status.success() {
                    debug!("pruned image {}", image);
                    images.write().await.remove(image);
                }
            }
        }

        Ok(())
    }

}

