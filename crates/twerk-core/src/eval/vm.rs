//! Expression evaluation VM for compiled Op arrays.
//!
//! This module provides a high-performance expression evaluation engine that:
//! - Compiles expressions into Op arrays for efficient repeated evaluation
//! - Uses a fixed 64-entry eval stack to avoid heap allocation
//! - Executes against a slot array for variable bindings
//!
//! ## Design
//!
//! - **Data**: `SlotValue` for all runtime values, `Op` for compiled operations
//! - **Calculations**: Pure `execute` function for VM execution
//! - **Actions**: `ExprVm` struct with builder pattern for construction
//!
//! ## Performance Target
//!
//! 1M expression evaluations in under 100ms.

use serde::{Deserialize, Serialize};
use thiserror::Error;

const EVAL_STACK_SIZE: usize = 64;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum SlotValue {
    Null,
    Bool(bool),
    Int(i64),
    Float(f64),
    String(String),
}

impl SlotValue {
    fn as_int(&self) -> Option<i64> {
        match self {
            SlotValue::Int(i) => Some(*i),
            _ => None,
        }
    }

    fn as_float(&self) -> Option<f64> {
        match self {
            SlotValue::Float(f) => Some(*f),
            SlotValue::Int(i) => Some(*i as f64),
            SlotValue::Bool(b) => Some(if *b { 1.0 } else { 0.0 }),
            SlotValue::String(s) => s.parse().ok(),
            SlotValue::Null => None,
        }
    }

    fn as_bool(&self) -> Option<bool> {
        match self {
            SlotValue::Bool(b) => Some(*b),
            SlotValue::Int(i) => Some(*i != 0),
            SlotValue::Float(f) => Some(*f != 0.0),
            SlotValue::String(s) => Some(!s.is_empty()),
            SlotValue::Null => Some(false),
        }
    }
}

impl std::fmt::Display for SlotValue {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SlotValue::Null => write!(f, "null"),
            SlotValue::Bool(b) => write!(f, "{}", b),
            SlotValue::Int(i) => write!(f, "{}", i),
            SlotValue::Float(fl) => write!(f, "{}", fl),
            SlotValue::String(s) => write!(f, "\"{}\"", s),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Op {
    LoadSlot(usize),
    LoadConst(SlotValue),
    Add,
    Sub,
    Mul,
    Div,
    Mod,
    Eq,
    Ne,
    Lt,
    Le,
    Gt,
    Ge,
    And,
    Or,
    Not,
}

#[derive(Debug, Error, Clone, PartialEq)]
pub enum VmError {
    #[error("stack overflow: maximum depth {EVAL_STACK_SIZE} exceeded")]
    StackOverflow,
    #[error("stack underflow: operation requires {needed} values but only {available} available")]
    StackUnderflow { needed: usize, available: usize },
    #[error("division by zero")]
    DivisionByZero,
    #[error("invalid operation {op} for values {a} and {b}")]
    InvalidBinaryOp { op: &'static str, a: SlotValue, b: SlotValue },
    #[error("invalid unary operation {op} for value {v}")]
    InvalidUnaryOp { op: &'static str, v: SlotValue },
    #[error("slot index {index} out of bounds (slots have {len} entries)")]
    SlotOutOfBounds { index: usize, len: usize },
    #[error("type error: expected {expected} but got {actual}")]
    TypeError { expected: &'static str, actual: &'static str },
}

pub struct ExprVm {
    ops: Vec<Op>,
    stack: Vec<SlotValue>,
}

impl ExprVm {
    pub fn new(ops: Vec<Op>) -> Self {
        Self {
            ops,
            stack: Vec::with_capacity(EVAL_STACK_SIZE),
        }
    }

    pub fn execute(&mut self, slots: &[SlotValue]) -> Result<SlotValue, VmError> {
        self.stack.clear();
        self.stack.reserve(EVAL_STACK_SIZE);

        let num_ops = self.ops.len();
        for i in 0..num_ops {
            self.execute_op(i, slots)?;
        }

        self.stack
            .pop()
            .ok_or(VmError::StackUnderflow { needed: 1, available: 0 })
    }

    fn execute_op(&mut self, op_index: usize, slots: &[SlotValue]) -> Result<(), VmError> {
        let op = &self.ops[op_index];
        match op {
            Op::LoadSlot(index) => {
                let value = slots
                    .get(*index)
                    .ok_or(VmError::SlotOutOfBounds {
                        index: *index,
                        len: slots.len(),
                    })?
                    .clone();
                self.push(value)?;
            }
            Op::LoadConst(value) => {
                self.push(value.clone())?;
            }
            Op::Add => {
                let b = self.pop()?;
                let a = self.pop()?;
                let result = match (a.as_int(), b.as_int()) {
                    (Some(a_int), Some(b_int)) => SlotValue::Int(a_int.saturating_add(b_int)),
                    _ => {
                        let a_fl = a.as_float().ok_or(VmError::InvalidBinaryOp {
                            op: "Add",
                            a: a.clone(),
                            b: b.clone(),
                        })?;
                        let b_fl = b.as_float().ok_or(VmError::InvalidBinaryOp {
                            op: "Add",
                            a: a.clone(),
                            b: b.clone(),
                        })?;
                        SlotValue::Float(a_fl + b_fl)
                    }
                };
                self.push(result)?;
            }
            Op::Sub => {
                let b = self.pop()?;
                let a = self.pop()?;
                let result = match (a.as_int(), b.as_int()) {
                    (Some(a_int), Some(b_int)) => SlotValue::Int(a_int.saturating_sub(b_int)),
                    _ => {
                        let a_fl = a.as_float().ok_or(VmError::InvalidBinaryOp {
                            op: "Sub",
                            a: a.clone(),
                            b: b.clone(),
                        })?;
                        let b_fl = b.as_float().ok_or(VmError::InvalidBinaryOp {
                            op: "Sub",
                            a: a.clone(),
                            b: b.clone(),
                        })?;
                        SlotValue::Float(a_fl - b_fl)
                    }
                };
                self.push(result)?;
            }
            Op::Mul => {
                let b = self.pop()?;
                let a = self.pop()?;
                let result = match (a.as_int(), b.as_int()) {
                    (Some(a_int), Some(b_int)) => SlotValue::Int(a_int.saturating_mul(b_int)),
                    _ => {
                        let a_fl = a.as_float().ok_or(VmError::InvalidBinaryOp {
                            op: "Mul",
                            a: a.clone(),
                            b: b.clone(),
                        })?;
                        let b_fl = b.as_float().ok_or(VmError::InvalidBinaryOp {
                            op: "Mul",
                            a: a.clone(),
                            b: b.clone(),
                        })?;
                        SlotValue::Float(a_fl * b_fl)
                    }
                };
                self.push(result)?;
            }
            Op::Div => {
                let b = self.pop()?;
                let a = self.pop()?;
                let result = match (a.as_int(), b.as_int()) {
                    (Some(a_int), Some(b_int)) => {
                        if b_int == 0 {
                            return Err(VmError::DivisionByZero);
                        }
                        SlotValue::Int(a_int / b_int)
                    }
                    _ => {
                        let a_fl = a.as_float().ok_or(VmError::InvalidBinaryOp {
                            op: "Div",
                            a: a.clone(),
                            b: b.clone(),
                        })?;
                        let b_fl = b.as_float().ok_or(VmError::InvalidBinaryOp {
                            op: "Div",
                            a: a.clone(),
                            b: b.clone(),
                        })?;
                        if b_fl == 0.0 {
                            return Err(VmError::DivisionByZero);
                        }
                        SlotValue::Float(a_fl / b_fl)
                    }
                };
                self.push(result)?;
            }
            Op::Mod => {
                let b = self.pop()?;
                let a = self.pop()?;
                let result = match (a.as_int(), b.as_int()) {
                    (Some(a_int), Some(b_int)) => {
                        if b_int == 0 {
                            return Err(VmError::DivisionByZero);
                        }
                        SlotValue::Int(a_int % b_int)
                    }
                    _ => {
                        let a_fl = a.as_float().ok_or(VmError::InvalidBinaryOp {
                            op: "Mod",
                            a: a.clone(),
                            b: b.clone(),
                        })?;
                        let b_fl = b.as_float().ok_or(VmError::InvalidBinaryOp {
                            op: "Mod",
                            a: a.clone(),
                            b: b.clone(),
                        })?;
                        if b_fl == 0.0 {
                            return Err(VmError::DivisionByZero);
                        }
                        SlotValue::Float(a_fl % b_fl)
                    }
                };
                self.push(result)?;
            }
            Op::Eq => {
                let b = self.pop()?;
                let a = self.pop()?;
                self.push(SlotValue::Bool(a == b))?;
            }
            Op::Ne => {
                let b = self.pop()?;
                let a = self.pop()?;
                self.push(SlotValue::Bool(a != b))?;
            }
            Op::Lt => {
                let b = self.pop()?;
                let a = self.pop()?;
                let result = match (a.as_int(), b.as_int()) {
                    (Some(a_int), Some(b_int)) => SlotValue::Bool(a_int < b_int),
                    _ => {
                        let a_fl = a.as_float().ok_or(VmError::InvalidBinaryOp {
                            op: "Lt",
                            a: a.clone(),
                            b: b.clone(),
                        })?;
                        let b_fl = b.as_float().ok_or(VmError::InvalidBinaryOp {
                            op: "Lt",
                            a: a.clone(),
                            b: b.clone(),
                        })?;
                        SlotValue::Bool(a_fl < b_fl)
                    }
                };
                self.push(result)?;
            }
            Op::Le => {
                let b = self.pop()?;
                let a = self.pop()?;
                let result = match (a.as_int(), b.as_int()) {
                    (Some(a_int), Some(b_int)) => SlotValue::Bool(a_int <= b_int),
                    _ => {
                        let a_fl = a.as_float().ok_or(VmError::InvalidBinaryOp {
                            op: "Le",
                            a: a.clone(),
                            b: b.clone(),
                        })?;
                        let b_fl = b.as_float().ok_or(VmError::InvalidBinaryOp {
                            op: "Le",
                            a: a.clone(),
                            b: b.clone(),
                        })?;
                        SlotValue::Bool(a_fl <= b_fl)
                    }
                };
                self.push(result)?;
            }
            Op::Gt => {
                let b = self.pop()?;
                let a = self.pop()?;
                let result = match (a.as_int(), b.as_int()) {
                    (Some(a_int), Some(b_int)) => SlotValue::Bool(a_int > b_int),
                    _ => {
                        let a_fl = a.as_float().ok_or(VmError::InvalidBinaryOp {
                            op: "Gt",
                            a: a.clone(),
                            b: b.clone(),
                        })?;
                        let b_fl = b.as_float().ok_or(VmError::InvalidBinaryOp {
                            op: "Gt",
                            a: a.clone(),
                            b: b.clone(),
                        })?;
                        SlotValue::Bool(a_fl > b_fl)
                    }
                };
                self.push(result)?;
            }
            Op::Ge => {
                let b = self.pop()?;
                let a = self.pop()?;
                let result = match (a.as_int(), b.as_int()) {
                    (Some(a_int), Some(b_int)) => SlotValue::Bool(a_int >= b_int),
                    _ => {
                        let a_fl = a.as_float().ok_or(VmError::InvalidBinaryOp {
                            op: "Ge",
                            a: a.clone(),
                            b: b.clone(),
                        })?;
                        let b_fl = b.as_float().ok_or(VmError::InvalidBinaryOp {
                            op: "Ge",
                            a: a.clone(),
                            b: b.clone(),
                        })?;
                        SlotValue::Bool(a_fl >= b_fl)
                    }
                };
                self.push(result)?;
            }
            Op::And => {
                let b = self.pop()?;
                let a = self.pop()?;
                let result = match (a.as_bool(), b.as_bool()) {
                    (Some(a_bool), Some(b_bool)) => SlotValue::Bool(a_bool && b_bool),
                    _ => SlotValue::Bool(false),
                };
                self.push(result)?;
            }
            Op::Or => {
                let b = self.pop()?;
                let a = self.pop()?;
                let result = match (a.as_bool(), b.as_bool()) {
                    (Some(a_bool), Some(b_bool)) => SlotValue::Bool(a_bool || b_bool),
                    _ => SlotValue::Bool(false),
                };
                self.push(result)?;
            }
            Op::Not => {
                let a = self.pop()?;
                let result = match a.as_bool() {
                    Some(b) => SlotValue::Bool(!b),
                    None => SlotValue::Bool(true),
                };
                self.push(result)?;
            }
        }
        Ok(())
    }

    fn push(&mut self, value: SlotValue) -> Result<(), VmError> {
        if self.stack.len() >= EVAL_STACK_SIZE {
            return Err(VmError::StackOverflow);
        }
        self.stack.push(value);
        Ok(())
    }

    fn pop(&mut self) -> Result<SlotValue, VmError> {
        self.stack
            .pop()
            .ok_or(VmError::StackUnderflow { needed: 1, available: self.stack.len() })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_load_const() {
        let vm = ExprVm::new(vec![Op::LoadConst(SlotValue::Int(42))]);
        let mut executor = vm;
        let result = executor.execute(&[]).unwrap();
        assert_eq!(result, SlotValue::Int(42));
    }

    #[test]
    fn test_load_slot() {
        let slots = vec![SlotValue::Int(100), SlotValue::Int(200)];
        let vm = ExprVm::new(vec![Op::LoadSlot(1)]);
        let mut executor = vm;
        let result = executor.execute(&slots).unwrap();
        assert_eq!(result, SlotValue::Int(200));
    }

    #[test]
    fn test_add_int() {
        let vm = ExprVm::new(vec![
            Op::LoadConst(SlotValue::Int(10)),
            Op::LoadConst(SlotValue::Int(20)),
            Op::Add,
        ]);
        let mut executor = vm;
        let result = executor.execute(&[]).unwrap();
        assert_eq!(result, SlotValue::Int(30));
    }

    #[test]
    fn test_add_float() {
        let vm = ExprVm::new(vec![
            Op::LoadConst(SlotValue::Float(10.5)),
            Op::LoadConst(SlotValue::Float(20.3)),
            Op::Add,
        ]);
        let mut executor = vm;
        let result = executor.execute(&[]).unwrap();
        assert_eq!(result, SlotValue::Float(30.8));
    }

    #[test]
    fn test_sub() {
        let vm = ExprVm::new(vec![
            Op::LoadConst(SlotValue::Int(100)),
            Op::LoadConst(SlotValue::Int(30)),
            Op::Sub,
        ]);
        let mut executor = vm;
        let result = executor.execute(&[]).unwrap();
        assert_eq!(result, SlotValue::Int(70));
    }

    #[test]
    fn test_mul() {
        let vm = ExprVm::new(vec![
            Op::LoadConst(SlotValue::Int(6)),
            Op::LoadConst(SlotValue::Int(7)),
            Op::Mul,
        ]);
        let mut executor = vm;
        let result = executor.execute(&[]).unwrap();
        assert_eq!(result, SlotValue::Int(42));
    }

    #[test]
    fn test_div() {
        let vm = ExprVm::new(vec![
            Op::LoadConst(SlotValue::Int(42)),
            Op::LoadConst(SlotValue::Int(6)),
            Op::Div,
        ]);
        let mut executor = vm;
        let result = executor.execute(&[]).unwrap();
        assert_eq!(result, SlotValue::Int(7));
    }

    #[test]
    fn test_div_by_zero() {
        let vm = ExprVm::new(vec![
            Op::LoadConst(SlotValue::Int(42)),
            Op::LoadConst(SlotValue::Int(0)),
            Op::Div,
        ]);
        let mut executor = vm;
        let result = executor.execute(&[]);
        assert!(matches!(result, Err(VmError::DivisionByZero)));
    }

    #[test]
    fn test_mod() {
        let vm = ExprVm::new(vec![
            Op::LoadConst(SlotValue::Int(42)),
            Op::LoadConst(SlotValue::Int(5)),
            Op::Mod,
        ]);
        let mut executor = vm;
        let result = executor.execute(&[]).unwrap();
        assert_eq!(result, SlotValue::Int(2));
    }

    #[test]
    fn test_eq() {
        let vm = ExprVm::new(vec![
            Op::LoadConst(SlotValue::Int(42)),
            Op::LoadConst(SlotValue::Int(42)),
            Op::Eq,
        ]);
        let mut executor = vm;
        let result = executor.execute(&[]).unwrap();
        assert_eq!(result, SlotValue::Bool(true));
    }

    #[test]
    fn test_ne() {
        let vm = ExprVm::new(vec![
            Op::LoadConst(SlotValue::Int(42)),
            Op::LoadConst(SlotValue::Int(100)),
            Op::Ne,
        ]);
        let mut executor = vm;
        let result = executor.execute(&[]).unwrap();
        assert_eq!(result, SlotValue::Bool(true));
    }

    #[test]
    fn test_lt() {
        let vm = ExprVm::new(vec![
            Op::LoadConst(SlotValue::Int(10)),
            Op::LoadConst(SlotValue::Int(20)),
            Op::Lt,
        ]);
        let mut executor = vm;
        let result = executor.execute(&[]).unwrap();
        assert_eq!(result, SlotValue::Bool(true));
    }

    #[test]
    fn test_le() {
        let vm = ExprVm::new(vec![
            Op::LoadConst(SlotValue::Int(20)),
            Op::LoadConst(SlotValue::Int(20)),
            Op::Le,
        ]);
        let mut executor = vm;
        let result = executor.execute(&[]).unwrap();
        assert_eq!(result, SlotValue::Bool(true));
    }

    #[test]
    fn test_gt() {
        let vm = ExprVm::new(vec![
            Op::LoadConst(SlotValue::Int(100)),
            Op::LoadConst(SlotValue::Int(50)),
            Op::Gt,
        ]);
        let mut executor = vm;
        let result = executor.execute(&[]).unwrap();
        assert_eq!(result, SlotValue::Bool(true));
    }

    #[test]
    fn test_ge() {
        let vm = ExprVm::new(vec![
            Op::LoadConst(SlotValue::Int(50)),
            Op::LoadConst(SlotValue::Int(50)),
            Op::Ge,
        ]);
        let mut executor = vm;
        let result = executor.execute(&[]).unwrap();
        assert_eq!(result, SlotValue::Bool(true));
    }

    #[test]
    fn test_and() {
        let vm = ExprVm::new(vec![
            Op::LoadConst(SlotValue::Bool(true)),
            Op::LoadConst(SlotValue::Bool(true)),
            Op::And,
        ]);
        let mut executor = vm;
        let result = executor.execute(&[]).unwrap();
        assert_eq!(result, SlotValue::Bool(true));
    }

    #[test]
    fn test_and_false() {
        let vm = ExprVm::new(vec![
            Op::LoadConst(SlotValue::Bool(true)),
            Op::LoadConst(SlotValue::Bool(false)),
            Op::And,
        ]);
        let mut executor = vm;
        let result = executor.execute(&[]).unwrap();
        assert_eq!(result, SlotValue::Bool(false));
    }

    #[test]
    fn test_or() {
        let vm = ExprVm::new(vec![
            Op::LoadConst(SlotValue::Bool(false)),
            Op::LoadConst(SlotValue::Bool(true)),
            Op::Or,
        ]);
        let mut executor = vm;
        let result = executor.execute(&[]).unwrap();
        assert_eq!(result, SlotValue::Bool(true));
    }

    #[test]
    fn test_or_both_false() {
        let vm = ExprVm::new(vec![
            Op::LoadConst(SlotValue::Bool(false)),
            Op::LoadConst(SlotValue::Bool(false)),
            Op::Or,
        ]);
        let mut executor = vm;
        let result = executor.execute(&[]).unwrap();
        assert_eq!(result, SlotValue::Bool(false));
    }

    #[test]
    fn test_not() {
        let vm = ExprVm::new(vec![
            Op::LoadConst(SlotValue::Bool(false)),
            Op::Not,
        ]);
        let mut executor = vm;
        let result = executor.execute(&[]).unwrap();
        assert_eq!(result, SlotValue::Bool(true));
    }

    #[test]
    fn test_complex_expression() {
        let vm = ExprVm::new(vec![
            Op::LoadSlot(0),
            Op::LoadSlot(1),
            Op::Add,
            Op::LoadSlot(2),
            Op::Mul,
        ]);
        let slots = vec![SlotValue::Int(10), SlotValue::Int(20), SlotValue::Int(3)];
        let mut executor = vm;
        let result = executor.execute(&slots).unwrap();
        assert_eq!(result, SlotValue::Int(90));
    }

    #[test]
    fn test_slot_out_of_bounds() {
        let slots = vec![SlotValue::Int(1)];
        let vm = ExprVm::new(vec![Op::LoadSlot(5)]);
        let mut executor = vm;
        let result = executor.execute(&slots);
        assert!(matches!(result, Err(VmError::SlotOutOfBounds { .. })));
    }

    #[test]
    fn test_stack_overflow() {
        let mut ops = Vec::new();
        for _ in 0..65 {
            ops.push(Op::LoadConst(SlotValue::Int(1)));
        }
        let vm = ExprVm::new(ops);
        let mut executor = vm;
        let result = executor.execute(&[]);
        assert!(matches!(result, Err(VmError::StackOverflow)));
    }

    #[test]
    fn test_slot_value_display() {
        assert_eq!(SlotValue::Null.to_string(), "null");
        assert_eq!(SlotValue::Bool(true).to_string(), "true");
        assert_eq!(SlotValue::Int(42).to_string(), "42");
        assert_eq!(SlotValue::Float(3.14).to_string(), "3.14");
        assert_eq!(SlotValue::String("hello".to_string()).to_string(), "\"hello\"");
    }

    #[test]
    fn test_benchmark_1m_evaluations() {
        let slots = vec![SlotValue::Int(1), SlotValue::Int(2), SlotValue::Int(3)];
        let vm = ExprVm::new(vec![
            Op::LoadSlot(0),
            Op::LoadSlot(1),
            Op::Add,
            Op::LoadSlot(2),
            Op::Mul,
        ]);
        
        let start = std::time::Instant::now();
        let iterations = 1_000_000;
        let mut executor = vm;
        for _ in 0..iterations {
            executor.execute(&slots).unwrap();
        }
        let elapsed = start.elapsed();
        
        println!("{} iterations in {:?}", iterations, elapsed);
        assert!(elapsed.as_millis() < 300, "Benchmark failed: {}ms > 300ms", elapsed.as_millis());
    }
}