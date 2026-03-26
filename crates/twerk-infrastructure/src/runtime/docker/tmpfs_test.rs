//! Tests for docker::tmpfs module.

use crate::runtime::docker::tmpfs::{TmpfsMounter, TmpfsMounterError};
use twerk_core::mount_type;
use twerk_core::mount::Mount;

#[test]
fn test_mount_tmpfs() {
    let mounter = TmpfsMounter::new();
    let mnt = Mount::new(mount_type::TMPFS, "/target");

    let result = mounter.mount(&mnt);
    assert!(result.is_ok());
}

#[test]
fn test_mount_tmpfs_with_source() {
    let mounter = TmpfsMounter::new();
    let mnt = Mount::new(mount_type::TMPFS, "/target").with_source("/source");

    let result = mounter.mount(&mnt);
    assert!(result.is_err());
}

#[test]
fn test_mount_tmpfs_no_target() {
    let mounter = TmpfsMounter::new();
    let mnt = Mount {
        mount_type: mount_type::TMPFS.to_string(),
        target: None,
        ..Default::default()
    };

    let result = mounter.mount(&mnt);
    assert!(result.is_err());
}

#[test]
fn test_mount_tmpfs_empty_target() {
    let mounter = TmpfsMounter::new();
    let mnt = Mount::new(mount_type::TMPFS, "").with_source("");

    let result = mounter.mount(&mnt);
    assert!(result.is_err());
}

#[test]
fn test_mount_tmpfs_empty_source_allowed() {
    let mounter = TmpfsMounter::new();
    let mnt = Mount::new(mount_type::TMPFS, "/target");

    // source is None, which is fine
    let result = mounter.mount(&mnt);
    assert!(result.is_ok());
}

#[test]
fn test_unmount_tmpfs() {
    let mounter = TmpfsMounter::new();
    let mnt = Mount::new(mount_type::TMPFS, "/target");

    let result = mounter.unmount(&mnt);
    assert!(result.is_ok());
}
