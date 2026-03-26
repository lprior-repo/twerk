//! Tests for docker::archive module.

use std::fs::File;
use std::io::Read;

use crate::runtime::docker::archive::{Archive, ArchiveEntry, ArchiveError};

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
