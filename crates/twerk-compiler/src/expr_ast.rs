use serde::{Deserialize, Serialize};

use crate::source_map::SourceSpan;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Expr {
    pub kind: ExprKind,
    pub span: SourceSpan,
}

impl Expr {
    #[must_use]
    pub const fn new(kind: ExprKind, span: SourceSpan) -> Self {
        Self { kind, span }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ExprKind {
    Literal(Literal),
    SlotAccess(SlotAccess),
    FunctionCall(FunctionCall),
    Unary {
        op: UnaryOp,
        expr: Box<Expr>,
    },
    Binary {
        op: BinaryOp,
        left: Box<Expr>,
        right: Box<Expr>,
    },
    Paren(Box<Expr>),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum Literal {
    Number(Box<str>),
    String(Box<str>),
    Bool(bool),
    Null,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SlotAccess {
    pub path: Vec<Box<str>>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FunctionCall {
    pub name: FunctionName,
    pub args: Vec<Expr>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum FunctionName {
    Length,
    Contains,
    Exists,
    Ident(Box<str>),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum UnaryOp {
    Not,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum BinaryOp {
    Or,
    And,
    Eq,
    Neq,
    Gt,
    Lt,
    Gte,
    Lte,
    Plus,
    Minus,
}
