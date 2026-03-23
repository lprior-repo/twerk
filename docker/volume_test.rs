//! Tests for docker::volume module.
//!
//! Note: Integration tests requiring a running Docker daemon are marked with `#[ignore]`.

use crate::docker::tork::mount_type;
use crate::docker::tork::Mount;
use crate::docker::volume::VolumeMounter;

#[tokio::test]
#[ignore = "requires Docker daemon"]
async fn test_create_volume() {
    let mounter = VolumeMounter::new().await.expect("should create mounter");

    let mnt = Mount::new(mount_type::VOLUME, "/somevol");

    let result = mounter.mount(&mnt).await;
    assert!(result.is_ok());
    let mounted = result.expect("should have mounted volume");
    assert!(mounted.source.is_some());
}
