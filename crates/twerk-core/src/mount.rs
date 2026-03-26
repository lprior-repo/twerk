use serde::{Deserialize, Serialize};
use std::collections::HashMap;

pub mod mount_type {
    pub const VOLUME: &str = "volume";
    pub const BIND: &str = "bind";
    pub const TMPFS: &str = "tmpfs";
}

pub const MOUNT_TYPE_VOLUME: &str = mount_type::VOLUME;
pub const MOUNT_TYPE_BIND: &str = mount_type::BIND;
pub const MOUNT_TYPE_TMPFS: &str = mount_type::TMPFS;

/// Mount represents a filesystem mount configuration.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct Mount {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(rename = "type", skip_serializing_if = "Option::is_none")]
    pub mount_type: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub target: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub opts: Option<HashMap<String, String>>,
}

impl Mount {
    #[must_use]
    pub fn new(mount_type: &str, target: &str) -> Self {
        Self {
            mount_type: Some(mount_type.to_string()),
            target: Some(target.to_string()),
            ..Default::default()
        }
    }

    #[must_use]
    pub fn with_source(mut self, source: &str) -> Self {
        self.source = Some(source.to_string());
        self
    }

    #[must_use]
    pub fn with_id(mut self, id: &str) -> Self {
        self.id = Some(id.to_string());
        self
    }
}
