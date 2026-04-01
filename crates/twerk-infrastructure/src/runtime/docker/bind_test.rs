//! Tests for docker::bind module.

use crate::runtime::docker::bind::{BindConfig, BindMounter, MountPolicy};
use twerk_core::mount::Mount;
use twerk_core::mount_type;

#[test]
fn test_bind_mount_not_allowed() {
    let mounter = BindMounter::new(BindConfig {
        policy: MountPolicy::Denied,
    });

    let mnt = Mount::new(mount_type::BIND, "/tmp").with_source("/tmp");

    let result = mounter.mount(&mnt);
    assert!(result.is_err());
}

#[test]
fn test_bind_mount_source_not_allowed() {
    let mounter = BindMounter::new(BindConfig {
        policy: MountPolicy::Allowed(vec!["/tmp".to_string()]),
    });

    let mnt = Mount::new(mount_type::BIND, "/other").with_source("/other");

    let result = mounter.mount(&mnt);
    assert!(result.is_err());
}

#[test]
fn test_bind_mount_allowed_source() {
    let mounter = BindMounter::new(BindConfig {
        policy: MountPolicy::Allowed(vec!["/tmp".to_string()]),
    });

    let mnt = Mount::new(mount_type::BIND, "/tmp").with_source("/tmp");

    let result = mounter.mount(&mnt);
    assert!(result.is_ok());
}

#[test]
fn test_bind_mount_empty_sources_allows_any() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let src = tmp.path().join("sub").to_string_lossy().to_string();

    let mounter = BindMounter::new(BindConfig {
        policy: MountPolicy::Allowed(vec![]),
    });

    let mnt = Mount::new(mount_type::BIND, &src).with_source(&src);

    let result = mounter.mount(&mnt);
    assert!(result.is_ok());
}

#[test]
fn test_bind_mount_case_insensitive() {
    let mounter = BindMounter::new(BindConfig {
        policy: MountPolicy::Allowed(vec!["/TMP".to_string()]),
    });

    let mnt = Mount::new(mount_type::BIND, "/tmp").with_source("/tmp");

    let result = mounter.mount(&mnt);
    assert!(result.is_ok());
}

#[test]
fn test_bind_mount_no_source() {
    let mounter = BindMounter::new(BindConfig {
        policy: MountPolicy::Allowed(vec![]),
    });

    let mnt = Mount::new(mount_type::BIND, "/target");
    // source is None

    let result = mounter.mount(&mnt);
    assert!(result.is_err());
}

#[test]
fn test_bind_mount_idempotent() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let src = tmp.path().to_string_lossy().to_string();

    let mounter = BindMounter::new(BindConfig {
        policy: MountPolicy::Allowed(vec![]),
    });

    let mnt = Mount::new(mount_type::BIND, &src).with_source(&src);

    assert!(mounter.mount(&mnt).is_ok());
    assert!(mounter.mount(&mnt).is_ok()); // second call should also succeed
}

#[test]
fn test_unmount_is_noop() {
    let mounter = BindMounter::new(BindConfig {
        policy: MountPolicy::Allowed(vec![]),
    });

    let mnt = Mount::new(mount_type::BIND, "/target").with_source("/target");

    assert!(mounter.unmount(&mnt).is_ok());
}
