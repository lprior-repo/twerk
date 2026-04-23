//! Mount-domain validators.
//!
//! Validates mount type, target path, and bind source paths.

use super::{fault_messages, ValidationFault, ValidationKind};
use crate::mount::Mount;

/// Validate a single mount.
pub(super) fn check_mount(mount: &Mount) -> Vec<ValidationFault> {
    let mut faults = Vec::new();
    if mount.mount_type.as_ref().is_some_and(String::is_empty) {
        faults.push(ValidationFault {
            kind: ValidationKind::MountType,
            message: "mount type is required".into(),
        });
    }
    faults.extend(check_mount_target(mount));
    faults.extend(check_bind_source(mount));
    faults
}

/// Validate a mount's target path.
fn check_mount_target(mount: &Mount) -> Vec<ValidationFault> {
    mount.target.as_ref().map_or_else(Vec::new, |target| {
        if target.is_empty() {
            vec![ValidationFault {
                kind: ValidationKind::MountTarget,
                message: "target is required".into(),
            }]
        } else if target.contains(':') {
            vec![ValidationFault {
                kind: ValidationKind::MountTarget,
                message: "invalid target path: cannot contain colon".into(),
            }]
        } else if target == "/tork" {
            vec![ValidationFault {
                kind: ValidationKind::MountTarget,
                message: "target path cannot be /tork".into(),
            }]
        } else {
            Vec::new()
        }
    })
}

/// Validate source path for bind mounts.
fn check_bind_source(mount: &Mount) -> Vec<ValidationFault> {
    if mount.mount_type.as_deref() != Some("bind") {
        return Vec::new();
    }
    match &mount.source {
        None => vec![ValidationFault {
            kind: ValidationKind::MountSource,
            message: "source is required for bind mount".into(),
        }],
        Some(src) if src.is_empty() => vec![ValidationFault {
            kind: ValidationKind::MountSource,
            message: "source is required for bind mount".into(),
        }],
        Some(src) if src.contains('#') => vec![ValidationFault {
            kind: ValidationKind::MountSource,
            message: "invalid source path: cannot contain hash".into(),
        }],
        _ => Vec::new(),
    }
}

/// Validates mount configurations.
///
/// # Errors
/// Returns a list of validation errors if any mounts are invalid.
pub fn validate_mounts(mounts: &Option<Vec<Mount>>) -> Result<(), Vec<String>> {
    let faults = mounts
        .as_ref()
        .map(|ms| ms.iter().flat_map(check_mount).collect::<Vec<_>>())
        .map_or_else(Vec::new, std::convert::identity);
    if faults.is_empty() {
        Ok(())
    } else {
        Err(fault_messages(&faults))
    }
}
