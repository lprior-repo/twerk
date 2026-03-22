//! Mount-related domain types

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Mount type constant for volume mounts
pub const MOUNT_TYPE_VOLUME: &str = "volume";
/// Mount type constant for bind mounts
pub const MOUNT_TYPE_BIND: &str = "bind";
/// Mount type constant for tmpfs mounts
pub const MOUNT_TYPE_TMPFS: &str = "tmpfs";

/// Mount represents a filesystem mount
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Mount {
    /// Mount ID (not serialized)
    #[serde(skip)]
    pub id: Option<String>,
    /// Mount type (volume, bind, tmpfs)
    #[serde(rename = "type", default)]
    pub mount_type: String,
    /// Source path or volume name
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source: Option<String>,
    /// Target path in container
    #[serde(skip_serializing_if = "Option::is_none")]
    pub target: Option<String>,
    /// Mount options
    #[serde(skip_serializing_if = "Option::is_none")]
    pub opts: Option<HashMap<String, String>>,
}

impl Default for Mount {
    fn default() -> Self {
        Self {
            id: None,
            mount_type: MOUNT_TYPE_VOLUME.to_string(),
            source: None,
            target: None,
            opts: None,
        }
    }
}

impl Mount {
    /// Creates a deep clone of this mount
    #[must_use]
    pub fn deep_clone(&self) -> Self {
        Self {
            id: self.id.clone(),
            mount_type: self.mount_type.clone(),
            source: self.source.clone(),
            target: self.target.clone(),
            opts: self
                .opts
                .as_ref()
                .map(|opts| opts.iter().map(|(k, v)| (k.clone(), v.clone())).collect()),
        }
    }
}

/// Creates a deep clone of a slice of mounts
#[must_use]
pub fn clone_mounts(mounts: &[Mount]) -> Vec<Mount> {
    mounts.iter().map(|m| m.deep_clone()).collect()
}
