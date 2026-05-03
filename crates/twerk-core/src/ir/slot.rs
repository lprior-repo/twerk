use super::types::*;
use std::boxed::Box;

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Taint {
    Clean = 0,
    Secret = 1,
    DerivedFromSecret = 2,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SlotValue {
    Null,
    Bool(bool),
    Number(i64), // finite only, no NaN
    Text(CompactText),
    List(ListRef),
    Object(ObjectRef),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompactText(pub Box<str>);

impl CompactText {
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl From<&str> for CompactText {
    fn from(s: &str) -> Self {
        Self(s.into())
    }
}

impl From<String> for CompactText {
    fn from(s: String) -> Self {
        Self(s.into_boxed_str())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ListRef(pub Box<[SlotValue]>);

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ObjectRef(pub Box<[(Box<str>, SlotValue)]>);

#[derive(Debug, Clone)]
pub enum StepState {
    Pending,
    Running,
    Completed,
    Failed { error_code: u16 },
    Waiting,
}

#[derive(Debug, Clone)]
pub struct RunFrame {
    pub id: RunId,
    pub current: StepIdx,
    pub step_states: Box<[StepState]>,
    pub slots: Box<[SlotValue]>,
    pub taint: Box<[Taint]>,
}

pub type RunId = [u8; 16]; // uuid bytes