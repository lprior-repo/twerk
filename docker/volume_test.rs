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

    let mut mnt = Mount::new(mount_type::VOLUME, "/somevol");

    let result = mounter.mount(&mut mnt).await;
    assert!(result.is_ok());
    assert!(mnt.source.is_some());
}
