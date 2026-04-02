//! Docker archive (tar) creation following functional-rust conventions.
//!
//! # Architecture
//!
//! - **Data**: `Archive` struct holds archive state
//! - **Calc**: Pure tar header/entry construction
//! - **Actions**: File I/O at boundary

use std::fs::{self, File};
use std::io::{self, BufReader, Read};
use std::os::unix::fs::MetadataExt;
use std::path::Path;

use libc::S_IFMT;
use tar::{Builder as TarBuilder, Header};
use tempfile::NamedTempFile;

use thiserror::Error;

/// Errors from archive operations.
#[derive(Debug, Error)]
pub enum ArchiveError {
    #[error("IO error: {0}")]
    Io(#[from] io::Error),

    #[error("archive error: {0}")]
    Archive(String),

    #[error("reader not initialized")]
    ReaderNotInitialized,
}

/// A tar archive writer backed by a temporary file.
///
/// This struct is consuming the temp file on drop, use `remove()` to delete it.
pub struct Archive {
    /// The temp file holding the archive data.
    temp_file: NamedTempFile,
    /// Buffered reader for reading back the archive.
    reader: Option<BufReader<File>>,
    /// Tar builder for building the archive.
    #[allow(dead_code)]
    writer: Option<TarBuilder<File>>,
}

impl Archive {
    /// Creates a new temporary tar archive.
    ///
    /// # Errors
    ///
    /// Returns `ArchiveError` if the temp file cannot be created.
    pub fn new() -> Result<Self, ArchiveError> {
        let temp_file = NamedTempFile::with_prefix("archive-").map_err(ArchiveError::Io)?;

        let file = temp_file.reopen().map_err(ArchiveError::Io)?;
        let writer = TarBuilder::new(file);

        Ok(Self {
            temp_file,
            reader: None,
            writer: Some(writer),
        })
    }

    /// Writes a file entry to the archive.
    ///
    /// # Errors
    ///
    /// Returns `ArchiveError` if the entry cannot be written.
    pub fn write_file(
        &mut self,
        name: &str,
        mode: u32,
        contents: &[u8],
    ) -> Result<(), ArchiveError> {
        let writer = self
            .writer
            .as_mut()
            .ok_or_else(|| ArchiveError::Archive("archive is finalized".to_string()))?;

        let mut header = Header::new_gnu();
        header.set_path(name).map_err(ArchiveError::Io)?;
        header.set_size(contents.len() as u64);
        header.set_mode(mode);
        header.set_cksum();

        writer.append(&header, contents).map_err(ArchiveError::Io)?;
        Ok(())
    }

    /// Finalizes the archive and prepares it for reading.
    ///
    /// # Errors
    ///
    /// Returns `ArchiveError` if the archive cannot be finalized.
    pub fn finish(&mut self) -> Result<(), ArchiveError> {
        if let Some(writer) = self.writer.take() {
            writer.into_inner().map_err(ArchiveError::Io)?;
        }
        Ok(())
    }

    /// Returns the path to the temporary archive file.
    #[must_use]
    pub fn path(&self) -> &Path {
        self.temp_file.path()
    }

    /// Removes the temporary archive file.
    ///
    /// # Errors
    ///
    /// Returns `ArchiveError` if the file cannot be removed.
    pub fn remove(mut self) -> Result<(), ArchiveError> {
        // First ensure any pending writes are finalized
        if self.writer.is_some() {
            self.finish()?;
        }
        let path = self.temp_file.path().to_path_buf();
        // Forget self so NamedTempFile's destructor doesn't try to remove the file
        // We manually remove it here instead
        std::mem::forget(self.temp_file);
        std::fs::remove_file(&path).map_err(ArchiveError::Io)?;
        Ok(())
    }

    /// Returns a reader for reading the archive contents.
    ///
    /// # Errors
    ///
    /// Returns `ArchiveError` if the file cannot be opened.
    pub fn reader(&mut self) -> Result<BufReader<File>, ArchiveError> {
        // Ensure writer is finalized
        if self.writer.is_some() {
            self.finish()?;
        }

        let file = File::open(self.temp_file.path()).map_err(ArchiveError::Io)?;
        Ok(BufReader::new(file))
    }
}

// NOTE: Default impl removed - Archive::new() can fail and Default cannot return Result.
// Callers should use Archive::new() directly and handle the Result.

impl Read for Archive {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        if self.reader.is_none() {
            self.finish().map_err(io::Error::other)?;
            let file = File::open(self.temp_file.path())?;
            let reader = BufReader::new(file);
            self.reader = Some(reader);
        }

        self.reader
            .as_mut()
            .ok_or_else(|| io::Error::other("reader not initialized"))?
            .read(buf)
    }
}

/// Returns the file type bits for the given path.
///
/// This is equivalent to Go's `FileType` function which uses `os.Lstat`.
///
/// # Errors
///
/// Returns `ArchiveError` if the file metadata cannot be read.
pub fn file_type(path: &Path) -> Result<u32, ArchiveError> {
    let meta = fs::symlink_metadata(path).map_err(ArchiveError::Io)?;
    Ok(meta.mode() & S_IFMT)
}

/// Builder for creating archive entries.
#[derive(Debug, Clone)]
pub struct ArchiveEntry {
    name: String,
    mode: u32,
    contents: Vec<u8>,
}

impl ArchiveEntry {
    /// Creates a new archive entry.
    #[must_use]
    pub fn new(name: impl Into<String>, mode: u32, contents: Vec<u8>) -> Self {
        Self {
            name: name.into(),
            mode,
            contents,
        }
    }

    /// Writes this entry to an archive.
    ///
    /// # Errors
    ///
    /// Returns `ArchiveError` if writing fails.
    pub fn write_to(&self, archive: &mut Archive) -> Result<(), ArchiveError> {
        archive.write_file(&self.name, self.mode, &self.contents)
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]
    #![allow(clippy::expect_used)]
    use super::*;
    use std::io::Read;

    #[test]
    fn test_create_archive() {
        let mut archive = Archive::new().expect("should create archive");

        archive
            .write_file("some_file.txt", 0o444, b"hello world")
            .expect("should write file");

        archive.finish().expect("should finish archive");

        // Read back and verify
        let file = File::open(archive.path()).expect("should open file");
        let mut tar = tar::Archive::new(file);

        let mut entries = tar.entries().expect("should list entries");
        let mut entry = entries
            .next()
            .expect("should have entry")
            .expect("should be valid entry");

        assert_eq!(
            entry.header().path().expect("should have path").as_ref(),
            std::path::Path::new("some_file.txt")
        );

        let mut contents = Vec::new();
        entry
            .read_to_end(&mut contents)
            .expect("should read contents");
        assert_eq!(b"hello world", contents.as_slice());
    }

    #[test]
    fn test_archive_remove() {
        let archive = Archive::new().expect("should create archive");
        let path = archive.path().to_path_buf();

        archive.remove().expect("should remove");

        assert!(!path.exists(), "temp file should be removed");
    }

    #[test]
    fn test_archive_write_multiple_files() {
        let mut archive = Archive::new().expect("should create archive");

        archive
            .write_file("file_a.txt", 0o644, b"contents of a")
            .expect("should write file a");
        archive
            .write_file("dir/file_b.txt", 0o755, b"contents of b")
            .expect("should write file b");

        archive.finish().expect("should finish archive");

        let file = File::open(archive.path()).expect("should open file");
        let mut tar = tar::Archive::new(file);

        let names: Vec<String> = tar
            .entries()
            .expect("should list entries")
            .map(|e| {
                e.expect("should be valid entry")
                    .header()
                    .path()
                    .expect("should have path")
                    .to_string_lossy()
                    .to_string()
            })
            .collect();

        assert_eq!(names.len(), 2);
        assert!(names.contains(&"file_a.txt".to_string()));
        assert!(names.contains(&"dir/file_b.txt".to_string()));
    }

    #[test]
    fn test_archive_read_impl() {
        let mut archive = Archive::new().expect("should create archive");

        archive
            .write_file("data.bin", 0o444, b"\x00\x01\x02\x03")
            .expect("should write file");

        // Read via the Read impl (auto-finalizes)
        let mut buf = Vec::new();
        archive
            .read_to_end(&mut buf)
            .expect("should read via Read impl");

        assert!(!buf.is_empty());
    }

    #[test]
    fn test_archive_entry_builder() {
        let entry = ArchiveEntry::new("script.sh", 0o755, b"#!/bin/sh\necho hi".to_vec());
        let mut archive = Archive::new().expect("should create archive");

        entry.write_to(&mut archive).expect("should write entry");
        archive.finish().expect("should finish");

        let file = File::open(archive.path()).expect("should open file");
        let mut tar = tar::Archive::new(file);

        let mut entries = tar.entries().expect("should list entries");
        let mut entry = entries
            .next()
            .expect("should have entry")
            .expect("should be valid entry");

        assert_eq!(
            entry.header().path().expect("should have path").as_ref(),
            std::path::Path::new("script.sh")
        );

        let mut contents = Vec::new();
        entry
            .read_to_end(&mut contents)
            .expect("should read contents");
        assert_eq!(b"#!/bin/sh\necho hi", contents.as_slice());
    }
}
