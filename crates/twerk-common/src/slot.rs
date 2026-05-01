use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct SlotIdx(pub u64);

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SlotValue {
    pub data: Vec<u8>,
}

pub const MAX_SLOTS: u64 = 1024;

#[derive(Debug, Clone)]
pub struct SlotAllocator;

impl SlotAllocator {
    #[must_use]
    pub fn new() -> Self {
        Self
    }

    #[must_use]
    pub fn allocate(&mut self) -> Option<SlotIdx> {
        Some(SlotIdx(0))
    }
}

impl Default for SlotAllocator {
    fn default() -> Self {
        Self::new()
    }
}
