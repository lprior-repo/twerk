use super::types::*;
use std::boxed::Box;

#[derive(Debug, Clone)]
pub struct CompiledExpr {
    pub ops: Box<[Op]>,
    pub max_stack: u8,
}

#[derive(Debug, Clone)]
pub enum Op {
    LoadSlot(SlotIdx),
    LoadConst(ConstIdx),
    LoadAccessor(AccessorIdx),
    Eq,
    Gt,
    Lt,
    Gte,
    Lte,
    And,
    Or,
    Not,
    Add,
    Sub,
    Mul,
    Div,
    Mod,
    Contains,
    StartsWith,
    EndsWith,
    Length,
    Exists(AccessorIdx),
    JumpTrue(u16),
    JumpFalse(u16),
    Jump(u16),
}

#[derive(Debug, Clone)]
pub struct CompiledAccessor {
    pub id: AccessorIdx,
    pub path: Box<[AccessorSegment]>,
}

#[derive(Debug, Clone)]
pub enum AccessorSegment {
    Field(Box<str>),
    Index(usize),
}