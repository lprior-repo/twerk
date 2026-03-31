//! Archive operations for container file initialization.
//!
//! Provides `TempArchive` consuming builder pattern for creating tar archives
//! to upload into containers.

use crate::runtime::docker::archive::{Archive, ArchiveError};
use crate::runtime::docker::error::DockerError;
use bollard::query_parameters::UploadToContainerOptions;
use bollard::{body_full, Docker};
use std::collections::HashMap;
use std::fs::File;
use std::io::{BufReader, Read};

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

/// Uploads files to a container at the specified path.
pub async fn upload_files_to_container(
    client: &Docker,
    container_id: &str,
    files: &HashMap<String, String>,
    target_path: &str,
) -> Result<(), DockerError> {
    if files.is_empty() {
        return Ok(());
    }

    let mut archive =
        Archive::new().map_err(|e| DockerError::CopyToContainer(e.to_string()))?;

    for (name, data) in files {
        archive
            .write_file(name, 0o444, data.as_bytes())
            .map_err(|e| DockerError::CopyToContainer(e.to_string()))?;
    }

    archive
        .finish()
        .map_err(|e| DockerError::CopyToContainer(e.to_string()))?;

    let mut reader = archive
        .reader()
        .map_err(|e| DockerError::CopyToContainer(e.to_string()))?;

    let mut contents = Vec::new();
    reader
        .read_to_end(&mut contents)
        .map_err(|e| DockerError::CopyToContainer(e.to_string()))?;

    let options = UploadToContainerOptions {
        path: target_path.to_string(),
        ..Default::default()
    };

    client
        .upload_to_container(container_id, Some(options), body_full(contents.into()))
        .await
        .map_err(|e| DockerError::CopyToContainer(e.to_string()))?;

    archive
        .remove()
        .map_err(|e| DockerError::CopyToContainer(e.to_string()))?;

    Ok(())
}

/// Creates a twerk/tork directory structure in the container.
pub async fn init_runtime_dir(
    client: &Docker,
    container_id: &str,
    run_script: Option<&str>,
    target_path: &str,
) -> Result<(), DockerError> {
    let mut archive =
        Archive::new().map_err(|e| DockerError::CopyToContainer(e.to_string()))?;

    archive
        .write_file("stdout", 0o222, &[])
        .map_err(|e| DockerError::CopyToContainer(e.to_string()))?;

    archive
        .write_file("progress", 0o222, &[])
        .map_err(|e| DockerError::CopyToContainer(e.to_string()))?;

    if let Some(script) = run_script {
        if !script.is_empty() {
            archive
                .write_file("entrypoint", 0o555, script.as_bytes())
                .map_err(|e| DockerError::CopyToContainer(e.to_string()))?;
        }
    }

    archive
        .finish()
        .map_err(|e| DockerError::CopyToContainer(e.to_string()))?;

    let mut reader = archive
        .reader()
        .map_err(|e| DockerError::CopyToContainer(e.to_string()))?;

    let mut contents = Vec::new();
    reader
        .read_to_end(&mut contents)
        .map_err(|e| DockerError::CopyToContainer(e.to_string()))?;

    let options = UploadToContainerOptions {
        path: target_path.to_string(),
        ..Default::default()
    };

    client
        .upload_to_container(container_id, Some(options), body_full(contents.into()))
        .await
        .map_err(|e| DockerError::CopyToContainer(e.to_string()))?;

    archive
        .remove()
        .map_err(|e| DockerError::CopyToContainer(e.to_string()))?;

    Ok(())
}
