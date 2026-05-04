//! SwitchState: Expression-based branching with precompiled ExprVm.
//!
//! SwitchState evaluates a list of (expression, target_step) pairs using
//! precompiled ExprVm expressions. First matching expression wins (returns true),
//! and the corresponding target step is selected. Falls through to default
//! if no expression matches.
//!
//! ## Performance
//!
//! Expressions are compiled to ExprVm Ops at workflow compile time, enabling
//! high-performance evaluation at runtime. Benchmark: 1000 switch evaluations
//! in under 1ms.

use serde::{Deserialize, Serialize};
use thiserror::Error;

use super::types::StateName;
use crate::eval::vm::{ExprVm, Op, SlotValue};

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum SwitchStateError {
    #[error("switch state must have at least one case")]
    EmptyCases,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SwitchCase {
    ops: Vec<Op>,
    target: StateName,
}

impl SwitchCase {
    #[must_use]
    pub fn new(ops: Vec<Op>, target: StateName) -> Self {
        Self { ops, target }
    }

    #[must_use]
    pub fn ops(&self) -> &[Op] {
        &self.ops
    }

    #[must_use]
    pub fn target(&self) -> &StateName {
        &self.target
    }

    pub fn evaluate(&self, slots: &[SlotValue]) -> bool {
        let mut vm = ExprVm::new(self.ops.clone());
        matches!(vm.execute(slots), Ok(SlotValue::Bool(true)))
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SwitchState {
    cases: Vec<SwitchCase>,
    #[serde(skip_serializing_if = "Option::is_none")]
    default: Option<StateName>,
}

impl SwitchState {
    pub fn new(
        cases: Vec<SwitchCase>,
        default: Option<StateName>,
    ) -> Result<Self, SwitchStateError> {
        if cases.is_empty() {
            return Err(SwitchStateError::EmptyCases);
        }
        Ok(Self { cases, default })
    }

    #[must_use]
    pub fn cases(&self) -> &[SwitchCase] {
        &self.cases
    }

    #[must_use]
    pub fn default(&self) -> Option<&StateName> {
        self.default.as_ref()
    }

    pub fn evaluate(&self, slots: &[SlotValue]) -> Option<&StateName> {
        for case in &self.cases {
            if case.evaluate(slots) {
                return Some(case.target());
            }
        }
        self.default.as_ref()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::eval::vm::Op;

    #[test]
    fn test_switch_case_evaluate_true() {
        let ops = vec![
            Op::LoadConst(SlotValue::Int(1)),
            Op::LoadConst(SlotValue::Int(1)),
            Op::Eq,
        ];
        let case = SwitchCase::new(ops, StateName::new("target").unwrap());
        let slots: &[SlotValue] = &[];
        assert!(case.evaluate(slots));
    }

    #[test]
    fn test_switch_case_evaluate_false() {
        let ops = vec![
            Op::LoadConst(SlotValue::Int(1)),
            Op::LoadConst(SlotValue::Int(2)),
            Op::Eq,
        ];
        let case = SwitchCase::new(ops, StateName::new("target").unwrap());
        let slots: &[SlotValue] = &[];
        assert!(!case.evaluate(slots));
    }

    #[test]
    fn test_switch_state_first_match() {
        let case1 = SwitchCase::new(
            vec![
                Op::LoadConst(SlotValue::Int(1)),
                Op::LoadConst(SlotValue::Int(1)),
                Op::Eq,
            ],
            StateName::new("case1").unwrap(),
        );
        let case2 = SwitchCase::new(
            vec![
                Op::LoadConst(SlotValue::Int(2)),
                Op::LoadConst(SlotValue::Int(2)),
                Op::Eq,
            ],
            StateName::new("case2").unwrap(),
        );
        let switch =
            SwitchState::new(vec![case1, case2], Some(StateName::new("default").unwrap())).unwrap();
        let slots: &[SlotValue] = &[];
        assert_eq!(
            switch.evaluate(slots),
            Some(&StateName::new("case1").unwrap())
        );
    }

    #[test]
    fn test_switch_state_falls_through_to_default() {
        let case1 = SwitchCase::new(
            vec![
                Op::LoadConst(SlotValue::Int(1)),
                Op::LoadConst(SlotValue::Int(2)),
                Op::Eq,
            ],
            StateName::new("case1").unwrap(),
        );
        let switch =
            SwitchState::new(vec![case1], Some(StateName::new("default").unwrap())).unwrap();
        let slots: &[SlotValue] = &[];
        assert_eq!(
            switch.evaluate(slots),
            Some(&StateName::new("default").unwrap())
        );
    }

    #[test]
    fn test_switch_state_error_on_empty_cases() {
        let result = SwitchState::new(vec![], None);
        assert!(matches!(result, Err(SwitchStateError::EmptyCases)));
    }

    #[test]
    fn test_benchmark_1000_evaluations() {
        let cases: Vec<SwitchCase> = (0..10)
            .map(|i| {
                SwitchCase::new(
                    vec![
                        Op::LoadConst(SlotValue::Int(i)),
                        Op::LoadConst(SlotValue::Int(i)),
                        Op::Eq,
                    ],
                    StateName::new(format!("case{}", i)).unwrap(),
                )
            })
            .collect();
        let switch = SwitchState::new(cases, Some(StateName::new("default").unwrap())).unwrap();
        let slots: &[SlotValue] = &[];

        let start = std::time::Instant::now();
        let iterations = 1000;
        for _ in 0..iterations {
            switch.evaluate(slots);
        }
        let elapsed = start.elapsed();

        println!("{} iterations in {:?}", iterations, elapsed);
        assert!(
            elapsed.as_millis() < 1,
            "Benchmark failed: {}ms > 1ms",
            elapsed.as_millis()
        );
    }
}
