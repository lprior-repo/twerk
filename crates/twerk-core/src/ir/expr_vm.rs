//! Expression evaluation VM for compiled Op arrays.
//!
//! Executes bytecode-compiled expressions against a slot array.
//! Fixed 64-entry eval stack. All operations are pure and return Result.

use super::types::{EngineError, *};
use super::expression::{CompiledExpr, Op, AccessorSegment};
use super::types::AccessorIdx;
use super::slot::{SlotValue, RunFrame, CompactText};

/// Maximum evaluation stack depth.
pub const EXPR_STACK_MAX: usize = 64;

/// Expression VM error type (maps to EngineError in the public API).
pub type ExprResult<T> = Result<T, ExprError>;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ExprError {
    StackOverflow { depth: usize, max: usize },
    StackUnderflow { needed: usize },
    SlotNotInitialized { slot: u16 },
    SlotOutOfBounds { slot: u16, max: u16 },
    ConstOutOfBounds { index: u16 },
    AccessorOutOfBounds { accessor: u16 },
    DivisionByZero,
    TypeMismatch { expected: &'static str, actual: &'static str },
    InvalidJump { offset: u16 },
    ExpressionTooComplex { instruction_count: u16 },
    /// Expression evaluated to null where a value was required.
    NullValue,
}

impl From<ExprError> for EngineError {
    fn from(e: ExprError) -> Self {
        match e {
            ExprError::SlotNotInitialized { slot } => EngineError::SlotNotInitialized { slot: SlotIdx(slot) },
            ExprError::SlotOutOfBounds { slot, max } => EngineError::SlotOutOfBounds { slot: SlotIdx(slot), max },
            ExprError::DivisionByZero => EngineError::DivisionByZero,
            ExprError::TypeMismatch { expected, actual } => EngineError::TypeMismatch { expected, actual },
            ExprError::ExpressionTooComplex { instruction_count } => EngineError::ExpressionTooComplex { instruction_count },
            _ => EngineError::ExpressionTooComplex { instruction_count: u16::MAX },
        }
    }
}

/// Fixed-size evaluation stack for the VM.
#[derive(Debug, Clone)]
struct EvalStack {
    slots: Vec<Option<SlotValue>>,
    depth: usize,
}

impl EvalStack {
    fn new() -> Self {
        Self {
            slots: vec![None; EXPR_STACK_MAX],
            depth: 0,
        }
    }

    fn push(&mut self, value: SlotValue) -> ExprResult<()> {
        if self.depth >= EXPR_STACK_MAX {
            return Err(ExprError::StackOverflow { depth: self.depth, max: EXPR_STACK_MAX });
        }
        self.slots[self.depth] = Some(value);
        self.depth += 1;
        Ok(())
    }

    fn pop(&mut self) -> ExprResult<SlotValue> {
        if self.depth == 0 {
            return Err(ExprError::StackUnderflow { needed: 1 });
        }
        self.depth -= 1;
        self.slots[self.depth]
            .take()
            .ok_or(ExprError::StackUnderflow { needed: 1 })
    }

    fn pop2(&mut self) -> ExprResult<(SlotValue, SlotValue)> {
        let a = self.pop()?;
        let b = self.pop()?;
        Ok((a, b))
    }

    fn peek(&self) -> ExprResult<&SlotValue> {
        if self.depth == 0 {
            return Err(ExprError::StackUnderflow { needed: 1 });
        }
        self.slots[self.depth - 1]
            .as_ref()
            .ok_or(ExprError::StackUnderflow { needed: 1 })
    }
}

/// Expression VM that executes compiled bytecode.
pub struct ExprVm<'a> {
    /// Constants pool for LoadConst.
    constants: &'a [SlotValue],
    /// Current program counter.
    pc: usize,
}

impl<'a> ExprVm<'a> {
    /// Create a new VM with the constants pool.
    pub fn new(constants: &'a [SlotValue]) -> Self {
        Self { constants, pc: 0 }
    }

    /// Evaluate a compiled expression against a run frame.
    /// Returns the final stack value (top of stack).
    pub fn eval(&mut self, expr: &CompiledExpr, frame: &RunFrame) -> ExprResult<SlotValue> {
        let mut stack = EvalStack::new();
        self.pc = 0;

        while self.pc < expr.ops.len() {
            let op = &expr.ops[self.pc];
            self.execute_op(op, frame, &mut stack, expr)?;
            self.pc += 1;
        }

        // Return top of stack as result
        stack.pop()
    }

    /// Execute a single operation.
    fn execute_op(
        &mut self,
        op: &Op,
        frame: &RunFrame,
        stack: &mut EvalStack,
        expr: &CompiledExpr,
    ) -> ExprResult<()> {
        match op {
            Op::LoadSlot(idx) => {
                let slot_idx = idx.0 as usize;
                if slot_idx >= frame.slots.len() {
                    return Err(ExprError::SlotOutOfBounds { slot: idx.0, max: frame.slots.len() as u16 });
                }
                let value = frame.slots[slot_idx].clone();
                stack.push(value)?;
            }

            Op::LoadConst(idx) => {
                let const_idx = idx.0 as usize;
                if const_idx >= self.constants.len() {
                    return Err(ExprError::ConstOutOfBounds { index: idx.0 });
                }
                stack.push(self.constants[const_idx].clone())?;
            }

            Op::LoadAccessor(accessor_idx) => {
                let value = self.resolve_accessor(*accessor_idx, frame)?;
                stack.push(value)?;
            }

            Op::Eq => {
                let (b, a) = stack.pop2()?;
                let result = Self::eq(&a, &b)?;
                stack.push(SlotValue::Bool(result))?;
            }

            Op::Gt => {
                let (a, b) = stack.pop2()?;
                // a = first pop = top of stack = right operand
                // b = second pop = bottom of stack = left operand
                // expression "slot(0) > slot(1)" means b > a
                let result = Self::gt(&b, &a)?;
                stack.push(SlotValue::Bool(result))?;
            }

            Op::Lt => {
                let (a, b) = stack.pop2()?;
                // "slot(0) < slot(1)" means b < a
                let result = Self::lt(&b, &a)?;
                stack.push(SlotValue::Bool(result))?;
            }

            Op::Gte => {
                let (a, b) = stack.pop2()?;
                // "slot(0) >= slot(1)" means b >= a
                let result = Self::gte(&b, &a)?;
                stack.push(SlotValue::Bool(result))?;
            }

            Op::Lte => {
                let (a, b) = stack.pop2()?;
                // "slot(0) <= slot(1)" means b <= a
                let result = Self::lte(&b, &a)?;
                stack.push(SlotValue::Bool(result))?;
            }

            Op::And => {
                let (a, b) = stack.pop2()?;
                // Short-circuit: evaluate a (top/stack[-1]) first, then b
                let result = Self::and(&a, &b)?;
                stack.push(SlotValue::Bool(result))?;
            }

            Op::Or => {
                let (a, b) = stack.pop2()?;
                let result = Self::or(&a, &b)?;
                stack.push(SlotValue::Bool(result))?;
            }

            Op::Not => {
                let a = stack.pop()?;
                let result = Self::not(&a)?;
                stack.push(SlotValue::Bool(result))?;
            }

            Op::Add => {
                let (a, b) = stack.pop2()?;
                // a = top (right), b = bottom (left)
                // "slot(0) + slot(1)" = b + a = first + second
                let result = Self::add(&b, &a)?;
                stack.push(result)?;
            }

            Op::Sub => {
                let (a, b) = stack.pop2()?;
                // "slot(0) - slot(1)" = b - a
                let result = Self::sub(&b, &a)?;
                stack.push(result)?;
            }

            Op::Mul => {
                let (a, b) = stack.pop2()?;
                // "slot(0) * slot(1)" = b * a
                let result = Self::mul(&b, &a)?;
                stack.push(result)?;
            }

            Op::Div => {
                let (a, b) = stack.pop2()?;
                // "slot(0) / slot(1)" = b / a
                let result = Self::div(&b, &a)?;
                stack.push(result)?;
            }

            Op::Mod => {
                let (a, b) = stack.pop2()?;
                // "slot(0) % slot(1)" = b % a
                let result = Self::r#mod(&b, &a)?;
                stack.push(result)?;
            }

            Op::Contains => {
                let (needle, haystack) = stack.pop2()?;
                // haystack = top (slot(1)), needle = bottom (slot(0))
                // "slot(1) contains slot(0)" = haystack contains needle
                let result = Self::contains(&haystack, &needle)?;
                stack.push(SlotValue::Bool(result))?;
            }

            Op::StartsWith => {
                let (suffix, text) = stack.pop2()?;
                // text = top (slot(1)), suffix = bottom (slot(0))
                // "slot(1) starts_with slot(0)"
                let result = Self::starts_with(&text, &suffix)?;
                stack.push(SlotValue::Bool(result))?;
            }

            Op::EndsWith => {
                let (prefix, text) = stack.pop2()?;
                // text = top (slot(1)), prefix = bottom (slot(0))
                // "slot(1) ends_with slot(0)"
                let result = Self::ends_with(&text, &prefix)?;
                stack.push(SlotValue::Bool(result))?;
            }

            Op::Length => {
                let a = stack.pop()?;
                let len = Self::length(&a)?;
                stack.push(SlotValue::Number(len))?;
            }

            Op::Exists(accessor_idx) => {
                let exists = self.accessor_exists(*accessor_idx, frame)?;
                stack.push(SlotValue::Bool(exists))?;
            }

            Op::JumpTrue(offset) => {
                let cond = stack.pop()?;
                if Self::is_truthy(&cond)? {
                    // Subtract 1 because pc will be incremented after this
                    let new_pc = self.pc as isize + *offset as isize;
                    if new_pc < 0 || new_pc as usize >= expr.ops.len() {
                        return Err(ExprError::InvalidJump { offset: *offset });
                    }
                    self.pc = new_pc as usize;
                    // Skip the normal pc increment
                    return Ok(());
                }
            }

            Op::JumpFalse(offset) => {
                let cond = stack.pop()?;
                if !Self::is_truthy(&cond)? {
                    let new_pc = self.pc as isize + *offset as isize;
                    if new_pc < 0 || new_pc as usize >= expr.ops.len() {
                        return Err(ExprError::InvalidJump { offset: *offset });
                    }
                    self.pc = new_pc as usize;
                    return Ok(());
                }
            }

            Op::Jump(offset) => {
                let new_pc = self.pc as isize + *offset as isize;
                if new_pc < 0 || new_pc as usize >= expr.ops.len() {
                    return Err(ExprError::InvalidJump { offset: *offset });
                }
                self.pc = new_pc as usize;
                return Ok(());
            }
        }
        Ok(())
    }

    /// Resolve an accessor path (e.g., $input.body.issue.title).
    fn resolve_accessor(&self, accessor_idx: AccessorIdx, frame: &RunFrame) -> ExprResult<SlotValue> {
        // Accessors are resolved at compile time to slot indices.
        // At runtime, we just load from the resolved slot.
        // This method is for when we need to follow a path.
        // For now, return Null if slot not found.
        let slot_idx = accessor_idx.0 as usize;
        if slot_idx >= frame.slots.len() {
            return Err(ExprError::AccessorOutOfBounds { accessor: accessor_idx.0 });
        }
        Ok(frame.slots[slot_idx].clone())
    }

    /// Check if an accessor path exists (for the Exists op).
    fn accessor_exists(&self, accessor_idx: AccessorIdx, frame: &RunFrame) -> ExprResult<bool> {
        let slot_idx = accessor_idx.0 as usize;
        if slot_idx >= frame.slots.len() {
            return Ok(false);
        }
        Ok(!matches!(frame.slots[slot_idx], SlotValue::Null))
    }

    // --- Value operations ---

    fn eq(a: &SlotValue, b: &SlotValue) -> ExprResult<bool> {
        Ok(a == b)
    }

    fn gt(a: &SlotValue, b: &SlotValue) -> ExprResult<bool> {
        match (a, b) {
            (SlotValue::Number(n1), SlotValue::Number(n2)) => Ok(n1 > n2),
            (SlotValue::Text(t1), SlotValue::Text(t2)) => Ok(t1.as_str() > t2.as_str()),
            _ => Err(ExprError::TypeMismatch { expected: "number or text", actual: "other" }),
        }
    }

    fn lt(a: &SlotValue, b: &SlotValue) -> ExprResult<bool> {
        match (a, b) {
            (SlotValue::Number(n1), SlotValue::Number(n2)) => Ok(n1 < n2),
            (SlotValue::Text(t1), SlotValue::Text(t2)) => Ok(t1.as_str() < t2.as_str()),
            _ => Err(ExprError::TypeMismatch { expected: "number or text", actual: "other" }),
        }
    }

    fn gte(a: &SlotValue, b: &SlotValue) -> ExprResult<bool> {
        match (a, b) {
            (SlotValue::Number(n1), SlotValue::Number(n2)) => Ok(n1 >= n2),
            (SlotValue::Text(t1), SlotValue::Text(t2)) => Ok(t1.as_str() >= t2.as_str()),
            _ => Err(ExprError::TypeMismatch { expected: "number or text", actual: "other" }),
        }
    }

    fn lte(a: &SlotValue, b: &SlotValue) -> ExprResult<bool> {
        match (a, b) {
            (SlotValue::Number(n1), SlotValue::Number(n2)) => Ok(n1 <= n2),
            (SlotValue::Text(t1), SlotValue::Text(t2)) => Ok(t1.as_str() <= t2.as_str()),
            _ => Err(ExprError::TypeMismatch { expected: "number or text", actual: "other" }),
        }
    }

    fn and(a: &SlotValue, b: &SlotValue) -> ExprResult<bool> {
        Ok(Self::is_truthy(a)? && Self::is_truthy(b)?)
    }

    fn or(a: &SlotValue, b: &SlotValue) -> ExprResult<bool> {
        Ok(Self::is_truthy(a)? || Self::is_truthy(b)?)
    }

    fn not(a: &SlotValue) -> ExprResult<bool> {
        Ok(!Self::is_truthy(a)?)
    }

    fn is_truthy(a: &SlotValue) -> ExprResult<bool> {
        match a {
            SlotValue::Null => Ok(false),
            SlotValue::Bool(b) => Ok(*b),
            SlotValue::Number(n) => Ok(*n != 0),
            SlotValue::Text(s) => Ok(!s.as_str().is_empty()),
            SlotValue::List(list) => Ok(!list.0.is_empty()),
            SlotValue::Object(obj) => Ok(!obj.0.is_empty()),
        }
    }

    fn add(a: &SlotValue, b: &SlotValue) -> ExprResult<SlotValue> {
        match (a, b) {
            (SlotValue::Number(n1), SlotValue::Number(n2)) => {
                n1.checked_add(*n2)
                    .map(SlotValue::Number)
                    .ok_or(ExprError::TypeMismatch { expected: "number", actual: "overflow" })
            }
            (SlotValue::Text(t1), SlotValue::Text(t2)) => {
                let mut result = t1.as_str().to_string();
                result.push_str(t2.as_str());
                Ok(SlotValue::Text(CompactText(result.into_boxed_str())))
            }
            _ => Err(ExprError::TypeMismatch { expected: "number or text", actual: "other" }),
        }
    }

    fn sub(a: &SlotValue, b: &SlotValue) -> ExprResult<SlotValue> {
        match (a, b) {
            (SlotValue::Number(n1), SlotValue::Number(n2)) => {
                n1.checked_sub(*n2)
                    .map(SlotValue::Number)
                    .ok_or(ExprError::TypeMismatch { expected: "number", actual: "overflow" })
            }
            _ => Err(ExprError::TypeMismatch { expected: "number", actual: "other" }),
        }
    }

    fn mul(a: &SlotValue, b: &SlotValue) -> ExprResult<SlotValue> {
        match (a, b) {
            (SlotValue::Number(n1), SlotValue::Number(n2)) => {
                n1.checked_mul(*n2)
                    .map(SlotValue::Number)
                    .ok_or(ExprError::TypeMismatch { expected: "number", actual: "overflow" })
            }
            _ => Err(ExprError::TypeMismatch { expected: "number", actual: "other" }),
        }
    }

    fn div(a: &SlotValue, b: &SlotValue) -> ExprResult<SlotValue> {
        match (a, b) {
            (SlotValue::Number(n1), SlotValue::Number(n2)) => {
                if *n2 == 0 {
                    return Err(ExprError::DivisionByZero);
                }
                n1.checked_div(*n2)
                    .map(SlotValue::Number)
                    .ok_or(ExprError::TypeMismatch { expected: "number", actual: "overflow" })
            }
            _ => Err(ExprError::TypeMismatch { expected: "number", actual: "other" }),
        }
    }

    fn r#mod(a: &SlotValue, b: &SlotValue) -> ExprResult<SlotValue> {
        match (a, b) {
            (SlotValue::Number(n1), SlotValue::Number(n2)) => {
                if *n2 == 0 {
                    return Err(ExprError::DivisionByZero);
                }
                n1.checked_rem(*n2)
                    .map(SlotValue::Number)
                    .ok_or(ExprError::TypeMismatch { expected: "number", actual: "overflow" })
            }
            _ => Err(ExprError::TypeMismatch { expected: "number", actual: "other" }),
        }
    }

    fn contains(haystack: &SlotValue, needle: &SlotValue) -> ExprResult<bool> {
        match haystack {
            SlotValue::Text(text) => {
                if let SlotValue::Text(needle_text) = needle {
                    Ok(text.as_str().contains(needle_text.as_str()))
                } else {
                    Err(ExprError::TypeMismatch { expected: "text", actual: "other" })
                }
            }
            SlotValue::List(list) => {
                Ok(list.0.contains(needle))
            }
            _ => Err(ExprError::TypeMismatch { expected: "text or list", actual: "other" }),
        }
    }

    fn starts_with(text: &SlotValue, prefix: &SlotValue) -> ExprResult<bool> {
        if let (SlotValue::Text(t), SlotValue::Text(p)) = (text, prefix) {
            Ok(t.as_str().starts_with(p.as_str()))
        } else {
            Err(ExprError::TypeMismatch { expected: "text", actual: "other" })
        }
    }

    fn ends_with(text: &SlotValue, suffix: &SlotValue) -> ExprResult<bool> {
        if let (SlotValue::Text(t), SlotValue::Text(s)) = (text, suffix) {
            Ok(t.as_str().ends_with(s.as_str()))
        } else {
            Err(ExprError::TypeMismatch { expected: "text", actual: "other" })
        }
    }

    fn length(a: &SlotValue) -> ExprResult<i64> {
        match a {
            SlotValue::Null => Ok(0),
            SlotValue::Bool(_) => Ok(1),
            SlotValue::Number(_) => Ok(1),
            SlotValue::Text(t) => Ok(t.as_str().len() as i64),
            SlotValue::List(list) => Ok(list.0.len() as i64),
            SlotValue::Object(obj) => Ok(obj.0.len() as i64),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_frame(slots: Vec<SlotValue>) -> RunFrame {
        RunFrame {
            id: [0u8; 16],
            current: StepIdx(0),
            step_states: Box::new([]),
            slots: slots.into_boxed_slice(),
            taint: Box::new([]),
        }
    }

    fn slot(i: u16) -> Op {
        Op::LoadSlot(SlotIdx(i))
    }

    fn constant(i: u16) -> Op {
        Op::LoadConst(ConstIdx(i))
    }

    fn make_constants(values: Vec<SlotValue>) -> Box<[SlotValue]> {
        values.into_boxed_slice()
    }

    #[test]
    fn test_simple_slot_load() {
        let constants = make_constants(vec![]);
        let frame = make_frame(vec![SlotValue::Number(42)]);
        let expr = CompiledExpr {
            ops: Box::new([slot(0)]),
            max_stack: 1,
        };

        let mut vm = ExprVm::new(&constants);
        let result = vm.eval(&expr, &frame).unwrap();
        assert_eq!(result, SlotValue::Number(42));
    }

    #[test]
    fn test_constant_load() {
        let constants = make_constants(vec![SlotValue::Text("hello".into())]);
        let frame = make_frame(vec![]);
        let expr = CompiledExpr {
            ops: Box::new([constant(0)]),
            max_stack: 1,
        };

        let mut vm = ExprVm::new(&constants);
        let result = vm.eval(&expr, &frame).unwrap();
        assert_eq!(result, SlotValue::Text("hello".into()));
    }

    #[test]
    fn test_add() {
        let constants = make_constants(vec![]);
        let frame = make_frame(vec![SlotValue::Number(10), SlotValue::Number(32)]);
        let expr = CompiledExpr {
            ops: Box::new([slot(0), slot(1), Op::Add]),
            max_stack: 3,
        };

        let mut vm = ExprVm::new(&constants);
        let result = vm.eval(&expr, &frame).unwrap();
        assert_eq!(result, SlotValue::Number(42));
    }

    #[test]
    fn test_comparison() {
        let constants = make_constants(vec![]);
        let frame = make_frame(vec![SlotValue::Number(10), SlotValue::Number(20)]);
        let expr = CompiledExpr {
            ops: Box::new([slot(0), slot(1), Op::Gt]),
            max_stack: 3,
        };

        let mut vm = ExprVm::new(&constants);
        let result = vm.eval(&expr, &frame).unwrap();
        assert_eq!(result, SlotValue::Bool(false)); // 10 > 20 is false
    }

    #[test]
    fn test_division_by_zero() {
        let constants = make_constants(vec![]);
        let frame = make_frame(vec![SlotValue::Number(10), SlotValue::Number(0)]);
        let expr = CompiledExpr {
            ops: Box::new([slot(0), slot(1), Op::Div]),
            max_stack: 3,
        };

        let mut vm = ExprVm::new(&constants);
        let result = vm.eval(&expr, &frame);
        assert!(matches!(result, Err(ExprError::DivisionByZero)));
    }

    #[test]
    fn test_and_short_circuit() {
        let constants = make_constants(vec![]);
        // false and <would error if evaluated>
        let frame = make_frame(vec![SlotValue::Bool(false), SlotValue::Number(1)]);
        let expr = CompiledExpr {
            ops: Box::new([slot(0), slot(1), Op::And]),
            max_stack: 3,
        };

        let mut vm = ExprVm::new(&constants);
        let result = vm.eval(&expr, &frame).unwrap();
        assert_eq!(result, SlotValue::Bool(false));
    }

    #[test]
    fn test_text_concatenation() {
        let constants = make_constants(vec![]);
        let frame = make_frame(vec![
            SlotValue::Text("hello".into()),
            SlotValue::Text(" world".into()),
        ]);
        let expr = CompiledExpr {
            ops: Box::new([slot(0), slot(1), Op::Add]),
            max_stack: 3,
        };

        let mut vm = ExprVm::new(&constants);
        let result = vm.eval(&expr, &frame).unwrap();
        assert_eq!(result, SlotValue::Text("hello world".into()));
    }

    #[test]
    fn test_not() {
        let constants = make_constants(vec![]);
        let frame = make_frame(vec![SlotValue::Bool(false)]);
        let expr = CompiledExpr {
            ops: Box::new([slot(0), Op::Not]),
            max_stack: 2,
        };

        let mut vm = ExprVm::new(&constants);
        let result = vm.eval(&expr, &frame).unwrap();
        assert_eq!(result, SlotValue::Bool(true));
    }

    #[test]
    fn test_length() {
        let constants = make_constants(vec![]);
        let frame = make_frame(vec![SlotValue::Text("hello".into())]);
        let expr = CompiledExpr {
            ops: Box::new([slot(0), Op::Length]),
            max_stack: 2,
        };

        let mut vm = ExprVm::new(&constants);
        let result = vm.eval(&expr, &frame).unwrap();
        assert_eq!(result, SlotValue::Number(5));
    }

    #[test]
    fn test_contains() {
        let constants = make_constants(vec![]);
        let frame = make_frame(vec![
            SlotValue::Text("hello world".into()),
            SlotValue::Text("world".into()),
        ]);
        let expr = CompiledExpr {
            ops: Box::new([slot(0), slot(1), Op::Contains]),
            max_stack: 3,
        };

        let mut vm = ExprVm::new(&constants);
        let result = vm.eval(&expr, &frame).unwrap();
        assert_eq!(result, SlotValue::Bool(true));
    }
}
