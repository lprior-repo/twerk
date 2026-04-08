//! Tests for docker::volume module.
//!
//! Note: Integration tests requiring a running Docker daemon are marked with `#[ignore]`.

#![allow(clippy::unwrap_used)]
#![allow(clippy::redundant_pattern_matching)]

use twerk_core::mount_type;
use twerk_core::mount::Mount;
use crate::runtime::docker::volume::VolumeMounter;

#[tokio::test]
#[ignore = "requires Docker daemon"]
async fn test_create_volume() {
    let mounter = VolumeMounter::new().await.expect("should create mounter");

    let mnt = Mount::new(mount_type::VOLUME, "/somevol");

    let result = mounter.mount(&mnt).await;
    assert!(matches!(result, Ok(_)));
    let mounted = result.expect("should have mounted volume");
    assert!(mounted.source.is_some());
}
