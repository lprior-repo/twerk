use super::types::*;
use super::slot::SlotValue;
use super::expression::{CompiledExpr, CompiledAccessor};
use std::boxed::Box;

pub type WorkflowDigest = [u8; 32]; // sha256

#[derive(Debug, Clone)]
pub struct CompiledWorkflow {
    pub name: Box<str>,
    pub digest: WorkflowDigest,
    pub nodes: Box<[CompiledNode]>,
    pub expressions: Box<[CompiledExpr]>,
    pub accessors: Box<[CompiledAccessor]>,
    pub input_slots: Box<[InputSlot]>,
    pub output_slots: Box<[OutputSlot]>,
    pub constants: Box<[SlotValue]>,
    pub result_expr: ExprIdx,
}

#[derive(Debug, Clone)]
pub struct CompiledNode {
    pub index: StepIdx,
    pub kind: CompiledNodeKind,
    pub output_slot: Option<SlotIdx>,
    pub next: StepIdx,
}

#[derive(Debug, Clone)]
pub enum CompiledNodeKind {
    Set { expr: ExprIdx },
    Choose { branches: Box<[ChooseBranch]>, default: Option<StepIdx> },
    ForEach { item_slot: SlotIdx, body: StepIdx, post: StepIdx },
    Together { branches: Box<[StepIdx]>, post: StepIdx },
    Collect { items_expr: ExprIdx, result_slot: SlotIdx },
    Reduce { items_expr: ExprIdx, accumulator_slot: SlotIdx, body: StepIdx },
    Repeat { max_attempts: u16, body: StepIdx, post: StepIdx },
    Wait { duration_expr: ExprIdx },
    Ask { prompt_expr: ExprIdx, result_slot: SlotIdx },
    Do { action: ActionIdx, input_expr: ExprIdx, output_slot: SlotIdx },
    Finish { result_expr: ExprIdx },
}

#[derive(Debug, Clone)]
pub struct ChooseBranch {
    pub condition: ExprIdx,
    pub target: StepIdx,
}

#[derive(Debug, Clone)]
pub struct InputSlot {
    pub name: Box<str>,
    pub slot: SlotIdx,
    pub ty: ValueType,
}

#[derive(Debug, Clone)]
pub struct OutputSlot {
    pub name: Box<str>,
    pub slot: SlotIdx,
    pub ty: ValueType,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ValueType {
    Null,
    Bool,
    Number,
    Text,
    List,
    Object,
}